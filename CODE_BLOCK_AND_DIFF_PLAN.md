# Plan: Code Block Copy + Tool Output Diff View

## Feature 1: Code Block Copy (Mouse Click)

### User Experience
- User sees code blocks in chat with a subtle "[copy]" indicator on the first line
- Clicking anywhere on a code block copies the raw code to clipboard
- Toast appears: "Copied rust code block (12 lines)"
- Clicking outside code blocks behaves normally (text selection)

### Implementation Order

#### Step 1: Add Code Block Tracking to App State

**File:** `src/app/state.rs`

Add struct and field to `App`:
```rust
#[derive(Debug, Clone)]
pub struct CodeBlockRange {
    pub start_visual_line: usize,
    pub end_visual_line: usize,
    pub raw_content: String,
    pub language: String,
}
```

Add to `App` struct (after `chat_visual_lines_styled`):
```rust
pub code_blocks: Vec<CodeBlockRange>,
```

Add to `App::new()` initialization:
```rust
code_blocks: Vec::new(),
```

#### Step 2: Build Logical Lines with Per-Message Counts

**File:** `src/app/tui/chat.rs`

Create `build_chat_logical_lines_with_counts()` that mirrors the existing `build_chat_logical_lines()` exactly but also returns a `Vec<usize>` of per-message logical line counts. **Copy the existing function verbatim** and add counting:

```rust
/// Build logical lines from messages for display.
/// Returns (logical_lines, msg_logical_counts) where msg_logical_counts[i] is the
/// number of logical lines produced by message i.
fn build_chat_logical_lines_with_counts(
    app: &App,
    palette: super::ThemePalette,
) -> (Vec<Line<'static>>, Vec<usize>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut msg_line_counts: Vec<usize> = Vec::new();

    for message in &app.messages {
        let (avatar, align, base_style, header_style) = match message.role.as_str() {
            "You" => (
                "YOU",
                Alignment::Right,
                Style::default().fg(palette.you_fg).bg(palette.you_bg),
                Style::default()
                    .fg(palette.you_header)
                    .add_modifier(Modifier::BOLD),
            ),
            "Assistant" => (
                "AI",
                Alignment::Left,
                Style::default()
                    .fg(palette.assistant_fg)
                    .bg(palette.assistant_bg),
                Style::default()
                    .fg(palette.assistant_header)
                    .add_modifier(Modifier::BOLD),
            ),
            "Tool" => (
                "TOOL",
                Alignment::Left,
                Style::default().fg(palette.tool_fg).bg(palette.tool_bg),
                Style::default()
                    .fg(palette.tool_header)
                    .add_modifier(Modifier::BOLD),
            ),
            _ => (
                "SYS",
                Alignment::Center,
                Style::default().fg(palette.system_fg).bg(palette.system_bg),
                Style::default()
                    .fg(palette.system_header)
                    .add_modifier(Modifier::ITALIC),
            ),
        };

        let mut count = 0;

        // 1. Header line (with optional tool_calls_preview — matches production exactly)
        lines.push(
            Line::from(vec![Span::styled(
                match &message.tool_calls_preview {
                    Some(preview) => format!("{avatar} {} | {preview}", message.timestamp),
                    None => format!("{avatar} {}", message.timestamp),
                },
                header_style,
            )])
            .alignment(align),
        );
        count += 1;

        // 2. Optional image badge line
        if message.image_count > 0 {
            lines.push(
                Line::from(vec![Span::styled(
                    format!(" [attached images: {}] ", message.image_count),
                    base_style
                        .fg(palette.image_badge_fg)
                        .bg(palette.image_badge_bg)
                        .add_modifier(Modifier::BOLD),
                )])
                .alignment(align),
            );
            count += 1;
        }

        // 3. Markdown content lines
        let message_lines = super::markdown_lines(
            &message.content,
            base_style,
            app.syntax_highlighting,
            palette,
        );
        for spans in message_lines {
            lines.push(Line::from(spans).alignment(align));
            count += 1;
        }

        // 4. Empty separator line
        lines.push(Line::default());
        count += 1;

        msg_line_counts.push(count);
    }

    (lines, msg_line_counts)
}
```

Keep the original as a thin wrapper:
```rust
fn build_chat_logical_lines(app: &App, palette: super::ThemePalette) -> Vec<Line<'static>> {
    build_chat_logical_lines_with_counts(app, palette).0
}
```

**Critical:** `msg_line_counts` must count exactly the same lines pushed for each message (header, optional image, markdown lines, separator). This ensures logical indices align with the visual reflow.

#### Step 3: Parse Code Blocks from Raw Content (Logical Phase)

**File:** `src/app/tui/chat.rs`

Add a struct and helper that parses code blocks from raw `message.content` and returns them as **logical** line ranges (before reflow):

```rust
/// A code block identified from raw message content, expressed in logical line indices.
#[derive(Debug, Clone)]
pub struct CodeBlockLogical {
    /// Index within the message's logical lines where the block starts (0-based).
    pub logical_start_in_msg: usize,
    /// Number of code content lines (before reflow; fences excluded).
    pub code_line_count: usize,
    pub raw_content: String,
    pub language: String,
}

/// Parse code blocks from all messages, returning logical-level block info.
fn parse_code_blocks_logical(messages: &[Message]) -> Vec<(usize, CodeBlockLogical)> {
    let mut result = Vec::new();

    for (msg_idx, msg) in messages.iter().enumerate() {
        // Count logical lines before content (header + optional image badge)
        let pre_content_lines = 1 + if msg.image_count > 0 { 1 } else { 0 };

        let mut in_code_block = false;
        let mut code_start_logical = 0;
        let mut code_lines = Vec::new();
        let mut code_lang = String::new();
        let mut logical_line_idx = 0; // tracks position within markdown output lines

        for raw_line in msg.content.lines() {
            let trimmed = raw_line.trim();

            if trimmed.starts_with("```") {
                if in_code_block {
                    if !code_lines.is_empty() {
                        result.push((msg_idx, CodeBlockLogical {
                            logical_start_in_msg: pre_content_lines + code_start_logical,
                            code_line_count: code_lines.len(),
                            raw_content: code_lines.join("\n"),
                            language: code_lang.clone(),
                        }));
                    }
                    in_code_block = false;
                    code_lines.clear();
                } else {
                    in_code_block = true;
                    code_start_logical = logical_line_idx;
                    code_lang = trimmed
                        .trim_start_matches("```")
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_string();
                }
            } else if in_code_block {
                code_lines.push(raw_line.to_string());
            }

            // Count non-fence lines (markdown_lines skips fences)
            if !trimmed.starts_with("```") {
                logical_line_idx += 1;
            }
        }

        // Flush unclosed block
        if in_code_block && !code_lines.is_empty() {
            result.push((msg_idx, CodeBlockLogical {
                logical_start_in_msg: pre_content_lines + code_start_logical,
                code_line_count: code_lines.len(),
                raw_content: code_lines.join("\n"),
                language: code_lang.clone(),
            }));
        }
    }

    result
}
```

**Why logical first:** We parse from raw content where fences exist. Logical indices are stable (don't depend on terminal width). The reflow phase converts them to visual ranges.

#### Step 4: Map Logical Code Blocks to Post-Reflow Visual Ranges

**File:** `src/app/tui/chat.rs` and `src/app/tui/mod.rs`

Add a shared helper in `src/app/tui/mod.rs` next to `manual_reflow`:

```rust
/// Visual rows count for a single logical line at `width`.
/// Uses `manual_reflow` on a single-line slice so the result matches production exactly.
pub(super) fn reflow_row_count_for_line(line: &Line<'_>, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    manual_reflow(std::slice::from_ref(line), width).len()
}
```

In `chat.rs`, add the mapping function:

```rust
/// Convert logical-level code blocks to post-reflow visual line ranges.
/// `logical_lines` = output of `build_chat_logical_lines` (one Line per logical line).
/// `msg_logical_counts[i]` = number of logical lines message `i` produced.
/// `blocks_logical` = output of `parse_code_blocks_logical`.
/// `chat_width` = the inner chat panel width (same value passed to `manual_reflow`).
fn map_code_blocks_to_visual(
    logical_lines: &[Line<'_>],
    msg_logical_counts: &[usize],
    blocks_logical: &[(usize, CodeBlockLogical)],
    chat_width: usize,
) -> Vec<CodeBlockRange> {
    // Build mapping: for each logical line index, what is the first visual line index it produced?
    let mut logical_to_visual_start = vec![0usize; logical_lines.len().saturating_add(1)];

    let mut visual_idx = 0;
    for (li, logical) in logical_lines.iter().enumerate() {
        logical_to_visual_start[li] = visual_idx;
        visual_idx += super::reflow_row_count_for_line(logical, chat_width);
    }
    logical_to_visual_start[logical_lines.len()] = visual_idx;

    // Convert each logical block to visual range
    blocks_logical
        .iter()
        .filter(|(_, block)| block.code_line_count > 0) // skip empty blocks
        .filter_map(|(msg_idx, block)| {
            let global_offset = compute_global_logical_offset(*msg_idx, msg_logical_counts);
            let start_logical = global_offset + block.logical_start_in_msg;
            let end_logical = (start_logical + block.code_line_count)
                .min(logical_lines.len());

            if start_logical >= logical_to_visual_start.len() {
                return None;
            }

            let visual_start = logical_to_visual_start[start_logical];
            let visual_end_exclusive = logical_to_visual_start[end_logical];

            if visual_end_exclusive <= visual_start {
                return None;
            }

            Some(CodeBlockRange {
                start_visual_line: visual_start,
                end_visual_line: visual_end_exclusive.saturating_sub(1),
                raw_content: block.raw_content.clone(),
                language: block.language.clone(),
            })
        })
        .collect()
}

/// Compute the global logical line offset for a message index.
/// Used internally by `map_code_blocks_to_visual`.
fn compute_global_logical_offset(msg_idx: usize, msg_logical_counts: &[usize]) -> usize {
    msg_logical_counts.iter().take(msg_idx).sum()
}
```

**Why this works:** We use the same `manual_reflow` function that production uses to count visual rows. This guarantees exact mapping regardless of wrapping complexity (span-aware greedy wrapping, mid-span splits, etc.).

**Cache-hit branch** (around line 147-152): Add after computing `max_scroll`:
```rust
// Cache is valid → messages haven't changed → code_blocks from last miss is still valid.
// No recomputation needed.
```

**Cache-miss branch** (around line 154-186): After building `styled_lines`, before updating cache:
```rust
// Phase 1: Build logical lines with per-message counts
let (logical_lines, msg_logical_counts) = build_chat_logical_lines_with_counts(app, palette);

// Phase 2: Parse code blocks from raw content (logical level)
let blocks_logical = parse_code_blocks_logical(&app.messages);

// Phase 3: Reflow to get visual lines (same as existing render_chat_panel logic)
let chat_width = split[0].width as usize;
let visual_lines = super::manual_reflow(&logical_lines, chat_width);
let styled_lines: Vec<Line<'static>> = visual_lines
    .into_iter()
    .map(|line| {
        let mut l = Line::from(
            line.spans
                .into_iter()
                .map(|s| Span::styled(s.content.into_owned(), s.style))
                .collect::<Vec<_>>(),
        );
        if let Some(align) = line.alignment {
            l = l.alignment(align);
        }
        l
    })
    .collect();

// Phase 4: Map logical block indices to post-reflow visual ranges
app.code_blocks = map_code_blocks_to_visual(
    &logical_lines,
    &msg_logical_counts,
    &blocks_logical,
    chat_width,
);

// Then: append [copy] spans to styled_lines, update cache, etc.
```

**Order matters:** `parse_code_blocks_logical` and `map_code_blocks_to_visual` must run after `logical_lines` is built but before `[copy]` spans are appended (so block indices are accurate).

#### Step 5: Render Copy Indicator on Code Block First Lines

**File:** `src/app/tui/chat.rs`

After building `styled_lines` and `code_blocks` (in cache-miss branch), add "[copy]" indicator to first line of each code block:

```rust
// Add copy indicators to code block first lines
for block in &app.code_blocks {
    if block.start_visual_line < styled_lines.len() {
        let line = &mut styled_lines[block.start_visual_line];
        line.spans.push(Span::styled(
            " [copy]",
            Style::default().fg(palette.body_muted).add_modifier(Modifier::DIM),
        ));
    }
}
```

**Why "[copy]" not emoji:** Emoji may render as 2 cells wide in some terminals. Plain text is safe.

**Cache behavior:** On cache hit, `chat_visual_lines_styled` already contains `[copy]` spans from the last cache miss (they were stored when the cache was built). So the indicator persists correctly during scrolling.

#### Step 6: Handle Code Block Copy in key_actions

**File:** `src/app/runtime/key_actions.rs`

Add helper function:
```rust
fn find_code_block_at_line(blocks: &[CodeBlockRange], visual_line: usize) -> Option<&CodeBlockRange> {
    blocks.iter().find(|b| visual_line >= b.start_visual_line && visual_line <= b.end_visual_line)
}
```

Modify `handle_start_selection()` to check for code block click first:

```rust
fn handle_start_selection(app: &mut App, mouse: MouseEvent) {
    // First, check if click is on a code block
    if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
        if let Some(block) = find_code_block_at_line(&app.code_blocks, pos.0) {
            // Copy code block to clipboard
            match ClipboardContext::new() {
                Ok(mut ctx) => {
                    let lang_label = if block.language.is_empty() {
                        "code".to_string()
                    } else {
                        block.language.clone()
                    };
                    let line_count = block.raw_content.lines().count();
                    if let Err(e) = ctx.set_contents(block.raw_content.clone()) {
                        app.set_toast(ToastLevel::Error, format!("Failed to copy: {e}"));
                    } else {
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Copied {} code block ({} lines)", lang_label, line_count),
                        );
                    }
                }
                Err(_) => {
                    app.set_toast(ToastLevel::Error, "Failed to copy to clipboard");
                }
            }
            return; // Don't start selection
        }
    }

    // Normal text selection behavior
    if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
        app.chat_selection_anchor = Some(pos);
        app.chat_selection_current = Some(pos);
        app.is_selecting = true;
    }
}
```

**Why here:** `handle_start_selection()` is called on left-click. By checking code block first, we intercept clicks on code blocks before starting text selection.

### Files Modified (Feature 1)
1. `src/app/state.rs` — Add `CodeBlockRange` struct, `code_blocks` field
2. `src/app/tui/chat.rs` — `build_chat_logical_lines_with_counts`, `parse_code_blocks_logical`, `map_code_blocks_to_visual`, `[copy]` rendering
3. `src/app/tui/mod.rs` — Add `reflow_row_count_for_line` helper next to `manual_reflow`
4. `src/app/runtime/key_actions.rs` — Modify `handle_start_selection()` to check code blocks, add helper

### Finish Criteria — Feature 1

#### Code Quality
- ✅ `cargo fmt --check` passes
- ✅ `cargo clippy -D warnings` passes
- ✅ `cargo build` succeeds

#### Tests Pass
- ✅ `parse_code_blocks_logical_tracks_single_block` — parses one code block from raw content
- ✅ `parse_code_blocks_logical_tracks_multiple_blocks` — parses multiple blocks
- ✅ `map_code_blocks_to_visual_handles_wrapping` — `map_code_blocks_to_visual` produces visual ranges that match `manual_reflow` output for wrapped lines
- ✅ `find_code_block_at_line_returns_correct_block` — hit-test helper for visual line indices
- ✅ `code_block_copy_sets_correct_toast` — asserts toast message, not clipboard I/O

#### No Regressions
- ✅ All existing tests pass (no test count decrease)
- ✅ No new clippy warnings
- ✅ No dead code warnings for new fields/functions

#### Manual Verification
- ✅ `[copy]` indicator appears on code block first lines
- ✅ Clicking code block copies correct content to clipboard
- ✅ Toast shows "Copied rust code block (N lines)"
- ✅ Clicking outside code block starts normal text selection
- ✅ Code blocks tracked on both cache hit and cache miss
- ✅ Unclosed code blocks (no closing ```) are still tracked

---

## Feature 2: Tool Output Diff View

### User Experience
- After a tool produces output, user can view it via `/diff` command
- Output is shown as a formatted code block in the chat
- No keyboard shortcut (to avoid conflicts with typing)

### Implementation Order

#### Step 1: Add Tool Output Storage to App State

**File:** `src/app/state.rs`

Add to `App` struct (after `code_blocks`):
```rust
pub last_tool_output: Option<String>,
```

Add to `App::new()` initialization:
```rust
last_tool_output: None,
```

**Note:** Named `last_tool_output` (not `last_tool_diff`) to accurately reflect that it stores the raw tool result, not a file diff.

#### Step 2: Capture Tool Result After Execution

**File:** `src/app/chat_pipeline.rs`

In the tool execution loop (inside `for tool_call in &current_tool_calls`), after the `result` is computed but before it's pushed as a message (around line 470-510), append:

```rust
// Store tool result for /diff view
if result.status == crate::app::tools::ToolResultStatus::Success && !result.output.is_empty() {
    app.last_tool_output = Some(result.output.clone());
}
```

**Placement:** Insert this line after the `result` variable is assigned (whether via confirmation or direct execution), and before `app.push_tool(...)`.

**Note:** Uses `result.output` (the correct field on `ToolResult`), not `result.content`. Gates on `ToolResultStatus::Success` to avoid storing error messages.

#### Step 3: Add /diff Command Route

**File:** `src/app/commands/dispatch.rs`

Add to `CommandRoute` enum:
```rust
Diff,
```

Add to `route_command()` match:
```rust
"/diff" => CommandRoute::Diff,
```

#### Step 4: Add /diff to Known Commands (Autocomplete)

**File:** `src/app/mod.rs`

Add to the `COMMAND_SUGGESTIONS` array (around line 57):
```rust
"/diff",
```

#### Step 5: Handle /diff Command

**File:** `src/app/commands/handlers/misc.rs`

Add handler in the `execute_slash_command()` match:
```rust
CommandRoute::Diff => {
    if let Some(ref output) = app.last_tool_output {
        app.push_system(format!("```\n{output}\n```"));
    } else {
        app.push_system(
            "No tool output available. Run a tool first, then use /diff to view its output.".to_string()
        );
    }
}
```

### Files Modified (Feature 2)
1. `src/app/state.rs` — Add `last_tool_output` field
2. `src/app/chat_pipeline.rs` — Capture tool result after execution
3. `src/app/commands/dispatch.rs` — Add `Diff` route
4. `src/app/mod.rs` — Add `/diff` to known commands
5. `src/app/commands/handlers/misc.rs` — Handle `/diff` command

### Finish Criteria — Feature 2

#### Code Quality
- ✅ `cargo fmt --check` passes
- ✅ `cargo clippy -D warnings` passes
- ✅ `cargo build` succeeds

#### Tests Pass
- ✅ `diff_command_shows_last_tool_output` — sets output, verifies system message
- ✅ `diff_command_shows_no_output_when_empty` — verifies "No tool output available"

#### No Regressions
- ✅ All existing tests pass (no test count decrease)
- ✅ No new clippy warnings
- ✅ No dead code warnings for new fields/functions

#### Manual Verification
- ✅ `/diff` shows last tool output as formatted code block
- ✅ "No tool output available" shown when no tool ran
- ✅ `/diff` appears in command autocomplete suggestions

---

## Combined Finish Criteria (Both Features)

Run after all implementation is complete:

```bash
./scripts/check.ps1
```

Must pass:
- ✅ `cargo fmt --check` — No formatting violations
- ✅ `cargo clippy -D warnings` — No clippy warnings
- ✅ `cargo build` — Compiles without errors
- ✅ `cargo test` — All tests pass (current count: `<baseline>` + 7 new tests)
  - **Note:** Run `cargo test --all-targets --all-features` before implementation to record the actual baseline count. Replace `<baseline>` with the real number (e.g., if baseline is 329, expect 336 total).

All per-feature finish criteria above must be satisfied before this combined check is run.

---

## Testing Plan

### Code Block Copy Tests (`src/app/mod_tests/regressions.rs`)

```rust
#[test]
fn parse_code_blocks_logical_tracks_single_block() {
    // Create messages with one code block in raw content
    // Call parse_code_blocks_logical()
    // Verify one CodeBlockLogical with correct raw_content, language, logical_start_in_msg
}

#[test]
fn parse_code_blocks_logical_tracks_multiple_blocks() {
    // Create messages with multiple code blocks
    // Call parse_code_blocks_logical()
    // Verify all blocks tracked with correct logical boundaries
}

#[test]
fn map_code_blocks_to_visual_handles_wrapping() {
    // Create logical lines where prose wraps but code doesn't
    // Call map_code_blocks_to_visual with exact msg_logical_counts
    // Verify visual ranges align with reflow output
}

#[test]
fn find_code_block_at_line_returns_correct_block() {
    // Test helper with visual line indices inside/outside blocks
    // Inside → returns Some
    // Outside → returns None
}

#[test]
fn code_block_copy_sets_correct_toast() {
    // Set up app with code_blocks populated
    // Call handle_start_selection() with click on code block line
    // Assert app.toast contains success message with language and line count
    // Do NOT assert on actual clipboard contents (avoids CI flakiness)
}
```

### Diff View Tests (`src/app/mod_tests/regressions.rs`)

```rust
#[test]
fn diff_command_shows_last_tool_output() {
    // Set app.last_tool_output = Some("tool output")
    // Call /diff handler
    // Verify system message contains the tool output
}

#[test]
fn diff_command_shows_no_output_when_empty() {
    // app.last_tool_output is None
    // Call /diff handler
    // Verify system message says "No tool output available"
}
```

### Manual Testing

1. **Code Block Copy:**
   - Send a message that produces a code block response
   - See `[copy]` indicator on first line of code block
   - Click on code block — clipboard should contain raw code
   - Toast should show: "Copied rust code block (N lines)"
   - Click outside code block — normal text selection behavior
   - Test with unclosed code block (no closing ```) — should still work

2. **Tool Output View:**
   - Run a tool that produces output (e.g., `/tools enable`, then ask model to use a tool)
   - Type `/diff` — should show the tool output
   - Without running a tool first — "No tool output available"

---

## Files Modified Summary

| File | Change |
|------|--------|
| `src/app/state.rs` | Add `CodeBlockRange`, `code_blocks`, `last_tool_output` |
| `src/app/tui/chat.rs` | `build_chat_logical_lines_with_counts`, `parse_code_blocks_logical`, `map_code_blocks_to_visual`, `[copy]` rendering |
| `src/app/tui/mod.rs` | Add `reflow_row_count_for_line` helper |
| `src/app/runtime/key_actions.rs` | Modify `handle_start_selection()` to check code blocks, add helper |
| `src/app/chat_pipeline.rs` | Capture tool result for `/diff` |
| `src/app/commands/dispatch.rs` | Add `Diff` route |
| `src/app/mod.rs` | Add `/diff` to `COMMAND_SUGGESTIONS` |
| `src/app/commands/handlers/misc.rs` | Handle `/diff` command |
| `src/app/mod_tests/regressions.rs` | Add 7 new tests (5 code block + 2 diff) |

**Total:** 9 files modified, no new dependencies, ~350-400 lines of code

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Code block `[copy]` indicator adds width to line | Low | Only on first line, dim styling makes it unobtrusive |
| Click detection overlaps with text selection | Low | Code block check happens first; if on code block, copy and return; otherwise proceed to selection |
| Tool output is too large for `/diff` display | Medium | Tool output is already truncated at `MAX_OUTPUT_SIZE` (10KB) |
| Reflow row count mismatch | Low | `reflow_row_count_for_line` uses `manual_reflow` on single-line slice; matches production exactly |
| Cache hit reuses stale `code_blocks` | Low | Cache invalidates on message change (`chat_content_version` bumps on push); blocks are recomputed on miss |
| Clipboard tests flaky in CI | Low | Tests assert on toast state only, not actual clipboard I/O |

## Rollback Plan

If issues arise:
1. Remove new fields from `App` struct
2. Remove code block detection from `handle_start_selection()`
3. Remove `/diff` command routing and handler
4. All changes are additive — no data loss risk
