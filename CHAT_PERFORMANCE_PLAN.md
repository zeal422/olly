# Plan: Chat Scroll Performance Optimization

## Problem

When a session has many messages, scrolling becomes sluggish. Every 80ms tick, the entire chat is re-rendered from scratch — parsing markdown, building visual lines, word-wrapping, and converting to strings for ALL messages, even when only the scroll position changed.

## Root Cause Analysis

### Current Rendering Pipeline (Every 80ms Tick)

```
Event Loop (80ms)
  └─ terminal.draw()
       └─ render_chat_panel()
            ├─ build_chat_logical_lines()     ← O(n) messages
            │   └─ markdown_lines()           ← Parse inline markdown for EVERY message
            │       └─ inline_markdown_spans() ← Character-by-character parsing
            ├─ manual_reflow()                ← Word-wrap ALL lines to width
            ├─ line_to_string()               ← Allocate strings for ALL lines
            ├─ update_chat_area_metadata()    ← Store in app.chat_visual_lines (already cached but not reused)
            ├─ apply_selection_to_text()      ← Apply selection to ALL lines
            └─ Paragraph::new().scroll()      ← Render to terminal
```

**Key issues:**
1. **Cache not reused**: `app.chat_visual_lines` exists but is overwritten every tick, never read
2. **Redundant work**: Markdown parsed, lines wrapped, strings allocated on every frame
3. **No content change detection**: No way to know if messages actually changed
4. **Selection applied to all lines**: Even off-screen lines get selection styling

### Code Locations

| Function | File | Line | Cost |
|----------|------|------|------|
| `render_chat_panel()` | `src/app/tui/chat.rs` | 16 | Entry point |
| `build_chat_logical_lines()` | `src/app/tui/chat.rs` | 219 | O(n) messages |
| `markdown_lines()` | `src/app/tui/markdown.rs` | 18 | Per message |
| `inline_markdown_spans()` | `src/app/tui/markdown.rs` | 189 | Char-by-char parsing |
| `manual_reflow()` | `src/app/tui/mod.rs` | 123 | Word-wrap all lines |
| `update_chat_area_metadata()` | `src/app/chat_navigation.rs` | 53 | Stores cache but doesn't reuse |
| Event loop tick | `src/app/runtime/event_loop.rs` | 23 | 80ms fixed |

### Existing Cache (Not Reused)

`src/app/state.rs:220`:
```rust
pub chat_visual_lines: Vec<(String, Option<Alignment>)>,
```

`src/app/chat_navigation.rs:53`:
```rust
pub(crate) fn update_chat_area_metadata(
    &mut self,
    area: ratatui::layout::Rect,
    visual_lines: Vec<(String, Option<Alignment>)>,
    max_scroll: u16,
) {
    self.chat_area = Some(area);
    self.chat_visual_lines = visual_lines;  // Already cached!
    self.update_chat_max_scroll_hint(max_scroll);
}
```

The cache exists but is never read — it's only written to. The fix is to **read the cache** and skip rebuilding when unchanged.

## Solution: Two-Level Cache with Version Tracking

### Approach

Use two caches:
1. **Visual lines cache** (`Vec<Line<'static>>`) — For rendering with selection styling
2. **Plain lines cache** (`Vec<(String, Option<Alignment>)>`) — For scroll metadata

Track changes with `chat_content_version` (incremented when messages change) and `chat_cached_version` (version when cache was built). Cache is valid when these match.

### Step 1: Add Cache Fields to App

#### File: `src/app/state.rs`

**Add to `App` struct** (after line 220):
```rust
pub chat_visual_lines_styled: Vec<ratatui::text::Line<'static>>,
pub chat_cached_version: u64,
pub chat_content_version: u64,
```

**Add to `App::new()`** (after line 303):
```rust
chat_visual_lines_styled: Vec::new(),
chat_cached_version: 0,
chat_content_version: 0,
```

### Step 2: Track Message Changes

#### File: `src/app/messages_and_conversation.rs`

After each message push, increment the version counter.

**In `push_message()`** (after line 53, before closing brace):
```rust
self.chat_content_version += 1;
```

This covers all callers: `push_user()`, `push_system()`, `push_assistant()`, `push_tool()`.

### Step 3: Use Cache in Rendering

#### File: `src/app/tui/chat.rs`

**Replace lines 133-163** with cache-aware logic:

```rust
let viewport_height = split[0].height;
let chat_width = split[0].width as usize;

// Check if cache is valid (version matches and not empty)
let cache_valid = !app.chat_visual_lines_styled.is_empty()
    && app.chat_content_version == app.chat_cached_version
    && app.chat_last_render_width == split[0].width;

let (styled_lines, plain_lines, max_scroll) = if cache_valid {
    // Use cached visual lines — skip expensive markdown parsing and reflow
    let plain: Vec<(String, Option<Alignment>)> = app.chat_visual_lines_styled
        .iter()
        .map(|line| (line_to_string(line), line.alignment))
        .collect();
    (
        app.chat_visual_lines_styled.clone(),
        plain,
        app.chat_max_scroll_hint.get(),
    )
} else {
    // Rebuild from scratch
    let logical_lines = build_chat_logical_lines(app, palette);
    let visual_lines = super::manual_reflow(&logical_lines, chat_width);
    let styled_lines: Vec<Line<'static>> = visual_lines
        .into_iter()
        .map(|line| Line::from(line.spans.into_iter().map(|s| Span::styled(
            s.content.into_owned(),
            s.style,
        )).collect::<Vec<_>>()).alignment(line.alignment))
        .collect();
    let plain_lines: Vec<(String, Option<Alignment>)> = styled_lines
        .iter()
        .map(|line| (line_to_string(line), line.alignment))
        .collect();
    let max_scroll = styled_lines.len().saturating_sub(viewport_height as usize) as u16;

    // Update cache
    app.chat_visual_lines_styled = styled_lines.clone();
    app.chat_cached_version = app.chat_content_version;
    app.chat_last_render_width = split[0].width;
    app.chat_area = Some(split[0]);
    app.chat_visual_lines = plain_lines.clone();
    app.chat_max_scroll_hint.set(max_scroll);

    (styled_lines, plain_lines, max_scroll)
};

let scroll_y = super::calculate_scroll_position(app.chat_scroll, max_scroll);

let display_lines = if let (Some(anchor), Some(current)) =
    (app.chat_selection_anchor, app.chat_selection_current)
{
    super::apply_selection_to_text(&styled_lines, anchor, current)
} else {
    styled_lines
};
```

### Step 4: Add Width Tracking Field

#### File: `src/app/state.rs`

**Add to `App` struct** (after `chat_cached_version`):
```rust
pub chat_last_render_width: u16,
```

**Add to `App::new()`** (after `chat_cached_version`):
```rust
chat_last_render_width: 0,
```

### Step 5: Handle Streaming Updates

#### File: `src/app/chat_pipeline.rs`

During streaming, the assistant message content changes. We need to invalidate the cache.

**In the streaming callback** (around line 280, inside the `|chunk|` closure):
```rust
if !chunk.content.is_empty() {
    streamed_any = true;
    let message = &mut app.messages[assistant_index];
    message.content.push_str(&chunk.content);
    app.chat_content_version += 1; // Invalidate cache by incrementing version
}
```

This increments the version, making `chat_content_version != chat_cached_version`, so the cache is rebuilt on the next tick.

### Step 6: Handle Resize Events

#### File: `src/app/runtime/event_loop.rs`

**In the `Event::Resize` handler** (around line 30):
```rust
Event::Resize(_, _) => {
    terminal.autoresize()?;
    terminal.clear()?;
    crate::app::platform::hide_console_scrollbars();
    app.clear_chat_selection();
    app.chat_visual_lines_styled.clear(); // Invalidate cache on resize
}
```

### Step 7: Handle Theme Changes

Theme changes don't increment the version counter, so cached visual lines would show old colors until a message is pushed.

#### File: `src/app/state.rs`

**In `set_theme()` method** (around line 400, after `self.theme = theme;`):
```rust
pub fn set_theme(&mut self, theme: UiTheme) {
    self.theme = theme;
    self.chat_content_version += 1; // Invalidate cache — theme affects styled lines
}
```

This ensures the cache is rebuilt on the next tick when the theme changes, applying new colors immediately.

### Step 8: Handle Welcome/No-Session States

The existing code already clears `chat_visual_lines` in these cases (lines 95, 121 in `chat.rs`). Update to also clear the styled cache:

**After line 95:**
```rust
app.chat_visual_lines_styled.clear();
```

**After line 121:**
```rust
app.chat_visual_lines_styled.clear();
```

## Testing

### Unit Tests: `src/app/mod_tests/regressions.rs`

```rust
#[test]
fn chat_content_version_increments_on_push_user() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let before = app.chat_content_version;
    app.push_user("Hello".to_string(), 0, None);
    assert!(app.chat_content_version > before);
}

#[test]
fn chat_content_version_increments_on_push_system() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let before = app.chat_content_version;
    app.push_system("System message".to_string());
    assert!(app.chat_content_version > before);
}

#[test]
fn chat_content_version_increments_on_push_assistant() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let before = app.chat_content_version;
    app.push_assistant("Assistant response".to_string(), None);
    assert!(app.chat_content_version > before);
}

#[test]
fn chat_visual_lines_cache_used_when_unchanged() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.push_user("Hello".to_string(), 0, None);

    // Simulate cache being built
    app.chat_content_version = 1;
    app.chat_cached_version = 1;
    app.chat_visual_lines_styled = vec![Line::from("test")];

    // Cache should be valid
    assert!(!app.chat_visual_lines_styled.is_empty());
    assert_eq!(app.chat_content_version, app.chat_cached_version);
}
```

### Manual Testing

1. Create session with 50+ messages
2. Scroll up/down — should be smooth (no lag)
3. Resize terminal — visual lines should reflow correctly
4. Send new message — cache should invalidate and rebuild
5. Select text — selection should highlight correctly with markdown styling preserved
6. During streaming — cache should invalidate on each chunk

## Finish Criteria

The implementation is complete when ALL of the following pass:

### 1. Code Quality Checks
```bash
./scripts/check.ps1
```
Must pass:
- ✅ `cargo fmt --check` — No formatting violations
- ✅ `cargo clippy -D warnings` — No clippy warnings
- ✅ `cargo build` — Compiles without errors
- ✅ `cargo test` — All tests pass (current count: 322+)

### 2. New Tests Pass
- ✅ `chat_content_version_increments_on_push_user`
- ✅ `chat_content_version_increments_on_push_system`
- ✅ `chat_content_version_increments_on_push_assistant`
- ✅ `chat_visual_lines_cache_used_when_unchanged`

### 3. No Regressions
- ✅ All existing tests pass (no test count decrease)
- ✅ No new clippy warnings
- ✅ No dead code warnings for new fields

### 4. Performance Verification
- ✅ Scrolling 50+ messages feels responsive (no visible lag)
- ✅ Cache is actually being used (verify by checking `cache_valid` path is taken during scroll)
- ✅ Markdown styling preserved (bold, italic, code blocks, syntax highlighting)
- ✅ Selection highlighting works correctly with cached lines

## Files Modified

- `src/app/state.rs` — Add `chat_visual_lines_styled`, `chat_cached_version`, `chat_content_version`, `chat_last_render_width`
- `src/app/messages_and_conversation.rs` — Increment version on message push
- `src/app/tui/chat.rs` — Use cache when valid, skip rebuild
- `src/app/chat_pipeline.rs` — Invalidate cache during streaming
- `src/app/runtime/event_loop.rs` — Clear cache on resize
- `src/app/mod_tests/regressions.rs` — Add cache tests

## Expected Improvement

| Metric | Before | After |
|--------|--------|-------|
| Render time (50 messages, scroll) | ~15-30ms | ~1-2ms (cache hit) |
| Scroll responsiveness | Sluggish | Instant |
| CPU usage (idle, no streaming) | ~2-5% | ~0.5-1% |
| Memory overhead | Baseline | +~100KB for styled cache |

## Rollback Plan

If issues arise:
1. Remove new cache fields from `App` struct
2. Revert `render_chat_panel()` to original logic
3. Remove version increments from `push_message()`
4. Cache is purely additive — no data loss risk
