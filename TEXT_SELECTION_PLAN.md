# Chat Text Selection + Copy Implementation Plan

## Overview
Implement mouse drag text selection and Ctrl+C copy for chat messages, respecting terminal UX conventions and existing architecture.

---

## Phase 1: Input Action Extension & Mouse Resolution

### ✅ Completed Items
- Mouse events already captured by `resolve_mouse_event`
- Clipboard integration via `copypasta` exists for paste

### 📝 Tasks
1. **Extend `InputAction` enum** (`src/app/input.rs`)
   - Add: `StartSelection`, `UpdateSelection`, `EndSelection`, `Copy`
   - Update `resolve_mouse_event()`:
     - `Down(Left)` → `StartSelection`
     - `Up(Left)` → `EndSelection`
     - `Moved` → `UpdateSelection`
   - Update `resolve_key_event_with_bindings()`:
     - Return `InputAction::Copy` unconditionally for `Ctrl+C` (or check copy binding but unconditionally)
     - The focus check will happen in handler
   - Optionally add `copy: InputBinding` to `InputBindings` struct with default `Ctrl+C` (or make configurable via config.toml)

2. **Add Copy binding to `InputBindings::default()`** and `InputBindings::from_config()` so users can remap if desired (optional but recommended)

**Done Criteria:**
- New InputAction variants compile
- Right-drag generates Start/Update/End sequence
- Ctrl+C generates Copy action in Chat focus (and slash command mode)
- Existing bindings/tests unaffected

---

## Phase 2: Selection State & Clearing Logic

### 📝 Tasks
1. **Add selection fields to `App` struct** (`src/app/mod.rs`)
   ```rust
   chat_selection_anchor: Option<(usize, usize)>, // (visual_line, col)
   chat_selection_current: Option<(usize, usize)>,
   is_selecting: bool,
   chat_area: Option<Rect>,           // last rendered chat pane area
   chat_visual_lines: Vec<String>,    // plain text of each visual line
   ```

2. **Implement `clear_chat_selection(&mut self)`** method
   - Clears all selection-related fields

3. **Integrate clearing into existing methods** (call `clear_chat_selection()` at start or appropriate point):
   - `push_user()`, `push_assistant()`, `push_tool()`, `push_system()`
   - `scroll_up()`, `scroll_down()`
   - `insert_input_char()`, `insert_input_string()`, `backspace_input_char()`, `insert_newline_at_cursor()`
   - `switch_focus()`
   - `activate_sidebar_selection()` (when switching context)
   - In `event_loop.rs` on `Event::Resize`: call `app.clear_chat_selection()`

4. **Additionally**, in `update_chat_area_metadata()`: if `chat_area` changed (different Rect) or `chat_visual_lines` length changed significantly, consider clearing selection to avoid stale indexes. Simpler: rely on the above clear points.

**Done Criteria:**
- All push/scroll/edit/clear methods invoke `clear_chat_selection()`
- `update_chat_area_metadata` exists and is called from `tui/mod.rs`
- Selection state is reset appropriately

---

## Phase 3: Manual Text Reflow & Rendering

### 📝 Tasks
1. **Create `manual_reflow()` helper** (`src/app/tui/mod.rs` or new module)
   - Input: `logical_lines: Vec<Line<Span>>` (each Line already stores its alignment)
   - Output: `Vec<Line<Span>>` (visual lines, each line's total width ≤ given width)
   - Algorithm:
     - For each input line:
       - If line width ≤ width, keep as-is.
       - Else, wrap:
         - Break at word boundaries: spaces, tabs, punctuation edges if possible.
         - If a single word exceeds width, break mid-word at character boundary.
         - When breaking, split the style span at the break point, clone style for both parts.
     - Return flattened `Vec` of visual lines; each retains the original line's alignment (the Line's alignment field is preserved).
     - Width measured as sum of `span.content.chars().count()` for spans in line (assumes single-width characters).

2. **Modify `render_chat_panel()`**:
   - Build base `logical_lines: Vec<Line<Span>>` from `app.messages` (same logic as `render_chat_text`, which already sets alignment via `.alignment(align)`)
   - Compute available width: `area.width as usize` (the chat pane width)
   - Call `visual_lines = manual_reflow(logical_lines, area.width as usize)`
   - Extract plain text: `app.chat_visual_lines = visual_lines.iter().map(|line| line.to_string()).collect();`
   - Compute `max_scroll = visual_lines.len().saturating_sub(viewport_height)`
   - Call `app.update_chat_area_metadata(split[0], app.chat_visual_lines.clone(), max_scroll)`
   - Apply selection: if selection active, call `apply_selection_to_text(visual_lines, selection_range)` → highlighted `Vec<Line<Span>>`
   - Render: `frame.render_widget(Paragraph::new(Text::from(highlighted_lines)).wrap(Wrap { trim: false }), split[0])` (lines already fit but wrap is harmless)
   - Note: keep `.scroll((scroll_y, 0))` as before

3. **Remove/Replace `render_chat_text()`**: This function is now obsolete; replace its usage with `manual_reflow` pipeline.

4. **Adjust height calculation**: Previously `line_count` was from `app.messages.iter().map(|m| m.content.lines().count()).sum()` (approx). Now use `visual_lines.len()` for accurate `max_scroll`.

**Done Criteria:**
- `manual_reflow` passes basic tests: short lines stay intact, long lines split at spaces, no line exceeds width
- Chat visual appearance identical to before
- `chat_visual_lines` accurately reflects on-screen text (including wrapped lines)
- Selection highlighting visible during drag
- Scroll bounds correctly computed from visual line count

### Notes on manual reflow implementation
- Use a simple greedy line wrap: accumulate spans/words until adding next would exceed width, then break.
- To split a span, create two new `Span`s with same style but different text slices.
- Alignment does not affect wrapping width; it's only for final rendering.
- Wide characters (e.g., double-width) likely not present in chat; if present, they may wrap incorrectly—acceptable edge case for now.

---

## Phase 4: Coordinate Mapping

### 📝 Tasks
1. **Implement `mouse_to_chat_visual_position(mouse: MouseEvent) -> Option<(usize, usize)>`** (`src/app/runtime/key_actions.rs`)
   - Use `app.chat_area` (set by `update_chat_area_metadata`) to derive chat pane rect
   - Guard: if `chat_area.is_none()` or mouse outside `chat_area` → `None`
   - `area = chat_area.unwrap()`
   - `relative_row = mouse.row.saturating_sub(area.y)`
   - Guard: `relative_row >= area.height` → `None` (mouse below pane)
   - `visual_line = app.chat_scroll + relative_row as usize`
   - `col = mouse.col.saturating_sub(area.x).min(area.width as usize - 1)`
   - Clamp `visual_line` to `app.chat_visual_lines.len() - 1` (if empty → `None`)
   - Return `Some((visual_line, col))`

2. **Edge handling**: Empty line at that visual_line returns `col=0` (selection still valid at start of line)

**Done Criteria:**
- Unit tests: given known area (x=1,y=0,w=80,h=20), scroll=15, mouse at (x+5, y+3) → visual_line=18, col=5
- Coordinates clamped to valid line range

---

## Phase 5: Selection Handlers

### 📝 Tasks
1. **`handle_start_selection(app, mouse)`**:
   - `pos = mouse_to_chat_visual_position(mouse.row, mouse.col)`
   - If `Some((line, col))`:
     - `app.chat_selection_anchor = Some((line, col))`
     - `app.chat_selection_current = Some((line, col))`
     - `app.is_selecting = true`
   - Else ignore

2. **`handle_update_selection(app, mouse)`**:
   - If `app.is_selecting`:
     - Compute current pos as above
     - Set `app.chat_selection_current`
   - Else ignore

3. **`handle_end_selection(app)`**:
   - `app.is_selecting = false`
   - (keep `selection_*` so copy can use them)

**Done Criteria:**
- Dragging left mouse sets anchor on down, updates current on move
- Releasing mouse ends selection
- Selection persists until cleared by other actions

---

## Phase 6: Copy Action

### 📝 Tasks
1. **`handle_copy(app)`** (`src/app/runtime/key_actions.rs`)
   - Guard: `app.focus != FocusPane::Chat && !app.is_slash_command_input()` → return
   - Extract selection:
     - Load `anchor` and `current` (if `None`, nothing to copy)
     - Normalize: `start <= end` (swap if needed)
      - Gather lines from `app.chat_visual_lines`:
        - Clamp `start_col` and `end_col` to each line's length to avoid panics
        - If `start_line == end_line`: take slice `start_clamped..=end_clamped` (inclusive)
        - Else: first line `start_clamped..`, middle lines full, last line `..=end_clamped`
      - Join with `\n`
   - Copy: `copypasta::ClipboardContext::new().and_then(|mut c| c.set_contents(text))`
   - Toast: `"Copied N characters to clipboard"` or `"Selection copied"`
   - `app.clear_chat_selection()`
   - Handle clipboard errors silently (as paste does)

2. **Keybinding**: Ensure `resolve_key_event_with_bindings` maps `Ctrl+C` to `InputAction::Copy` when focus allows (same conditions as paste)

**Done Criteria:**
- Ctrl+C copies selected text to system clipboard
- Selection cleared after copy
- Toasted confirmation
- Clipboard errors silent

---

## Phase 7: Selection Highlighting

### 📝 Tasks
1. **Implement `apply_selection_to_text(visual_lines: Vec<Line<Span>>, range: ((usize, usize), (usize, usize))) -> Vec<Line<Span>>`**
    - Normalize range so `start_line <= end_line` (swap if needed; inclusive)
    - For each line with index `i`:
      - If `i < start_line` or `i > end_line`: keep unchanged
      - Else if `i == start_line && i == end_line`: partial line → apply selection style to segment `start_col..=end_col`
      - Else if `i == start_line`: first of multi-line → style from `start_col` to end of line
      - Else if `i == end_line`: last of multi-line → style from start to `end_col` (inclusive)
      - Else: whole line selected → style all spans
    - **Applying style to a segment**: Given a `Line<Span>` and column range, split spans at boundaries:
      - Iterate spans, track cumulative char offset.
      - For each span, compute `span_start_char` and `span_end_char` (exclusive) offset within the line.
      - Find intersection with selection range.
      - If intersection exists, produce:
        - Spans before intersection (unchanged)
        - Intersection span with selection style added (merge with existing style via `.style_modifier(REVERSED)`)
        - Spans after intersection (unchanged)
      - Concatenate resulting spans into new `Line`.
    - Selection style: `Style::default().add_modifier(Modifier::REVERSED)`
    - Return new `Vec<Line<Span>>`; alignment of each line is preserved (Line's alignment field passed through)

2. **Integration in `render_chat_panel()`**: After manual reflow, if any selection exists, transform visual lines using `apply_selection_to_text()` before rendering.

3. **Line-only optimization**: If `start_col == 0 && end_col >= line_width-1` (or `end_col` beyond line length), treat as whole-line selection to skip span splitting.

2. **Integration in `render_chat_panel()`**: After manual reflow, if any selection exists, transform visual lines using `apply_selection_to_text()` before rendering.

3. **Line-only optimization**: If `start_col == 0 && end_col == line_width-1` for a line, treat as whole-line selection to skip span splitting.

**Done Criteria:**
- Dragging highlights correct text region (character-accurate)
- Works across wrapped lines
- Alignment preserved for each visual line
- Selection style visible regardless of theme

---

## Phase 8: Testing & Polish

### 📝 Tasks
1. **Add unit tests**:
   - `manual_reflow`: short line, long line with spaces, long word without spaces, multiple spans
   - `mouse_to_chat_visual_position` coordinate mapping
   - Selection range normalization and extraction from `chat_visual_lines`
   - `apply_selection_to_text` covers partial/full line cases, span splitting
   - `Copy` handler with mocked clipboard
   - Clearing on push/scroll/edit

2. **Integration test**:
   - Drag-select, Ctrl+C, paste somewhere (input?) verifies clipboard

3. **Manual verification tests**:
   - Render small chat with known messages, ensure reflow output matches expected line breaks
   - Visually confirm selection highlight matches mouse drag
   - Scroll while selected → selection cleared
   - Copy and paste into external editor preserves text

4. **Edge cases**:
   - Selection beyond visible area (scrolled off)
   - Multi-line message wrapping
   - Empty lines
   - Right-to-left selection (drag backwards)
   - Rapid scrolling during selection (should clear)

5. **Cross-platform verification**: Windows terminal, Linux, macOS

6. **Documentation**: Update `CHANGELOG.md` with "Chat text selection and Ctrl+C copy" entry

**Done Criteria:**
- All tests pass
- No regressions in existing tests (`cargo test --all-targets`)
- Manual test confirms: drag selects, release keeps highlight, Ctrl+C copies, paste works
- Clippy passes (`cargo clippy`)

---

## Success Definition

- Users can click and drag in chat pane to select text (character-accurate)
- Selected region highlighted with inverted colors (or theme selection color if added later)
- Ctrl+C copies selection to system clipboard with toast confirmation
- Selection cleared on subsequent user actions (scroll, new message, edit, focus change)
- No performance degradation during rendering
- All existing functionality preserved

---

## Implementation Order

Phases 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8

Dependencies:
- Phase 2 depends on Phase 1 (new fields)
- Phase 3 depends on Phase 2 (state updates)
- Phase 4 depends on Phase 3 (metadata availability)
- Phase 5 depends on Phase 4 (coordinate mapping)
- Phase 6 depends on Phase 5 (selection state)
- Phase 7 depends on Phase 3 (rendering)
- Phase 8 depends on all

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Manual reflow diverges from ratatui's Wrap | Selection misalignment | Implement thorough `manual_reflow` tests; compare line counts against known cases; adjust algorithm to match `Wrap { trim: false }` semantics exactly |
| Span splitting bugs | Incorrect highlight boundaries | Write edge-case tests for partial-word selections; ensure style merging is correct; add whole-line fast path |
| Performance with large chats | Jank during scroll | Cache `chat_visual_lines`; only recompute on new messages or resize; avoid reallocations by reusing buffers |
| Multi-byte/grapheme clusters | Invalid char offsets | Use `char_indices()` consistently; assume terminal uses single-width chars (most common). Future: integrate `unicode-width` if needed |
| Clipboard errors | Copy fails silently | Toast on error (optional); keep silent for now to match paste behavior |

---

## File Touch List

- `src/app/input.rs`
- `src/app/mod.rs`
- `src/app/runtime/key_actions.rs`
- `src/app/tui/mod.rs`
- `tests/...` (new test files)
- `CHANGELOG.md`

---

**Total Estimated Complexity: High** (due to manual reflow + span splitting + selection highlighting).