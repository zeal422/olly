# Text Selection Enhancement Implementation Plan

## Overview
Add drag-to-scroll selection and Shift+click range selection to the existing mouse drag text selection feature in Olly.

---

## Current State Analysis

### Existing Selection Infrastructure

**Input Resolution** (`src/app/input.rs`):
- `InputAction::StartSelection` - Left mouse down
- `InputAction::UpdateSelection` - `Drag(MouseButton::Left)` events
- `InputAction::EndSelection` - Left mouse up
- `InputAction::Copy` - Ctrl+C

**Selection State** (`src/app/mod.rs`):
```rust
chat_selection_anchor: Option<(usize, usize)>,  // (visual_line, col)
chat_selection_current: Option<(usize, usize)>,
is_selecting: bool,
chat_area: Option<Rect>,
chat_visual_lines: Vec<(String, Option<Alignment>)>,
chat_scroll: u16,  // Offset from bottom
chat_max_scroll_hint: Cell<u16>,
```

**Coordinate Mapping** (`src/app/runtime/key_actions.rs:23-70`):
- `mouse_to_chat_visual_position()` maps mouse coords to (visual_line, col)
- Handles alignment offsets (Left/Right/Center)
- Returns `None` if outside rendered text span

**Selection Handlers** (`src/app/runtime/key_actions.rs`):
- `handle_start_selection()` - Sets anchor and current
- `handle_update_selection()` - Updates current during drag
- `handle_end_selection()` - Sets `is_selecting = false`

**Event Loop** (`src/app/runtime/event_loop.rs:65-89`):
```rust
Event::Mouse(mouse) => {
    let action = resolve_mouse_event(mouse);
    if action != InputAction::Noop {
        // Overlay check (returns early if overlay consumes action)
        // ...
        handle_action(app, action, Some(mouse), ...)?;
    }
}
```

**Important Notes from Verification:**
- `MouseEvent` has `modifiers: KeyModifiers` field for Shift detection
- Overlays consume all mouse actions when active (correct behavior)
- `chat_visual_lines` contains ALL reflowed lines (not just visible)
- `scroll_up()` and `scroll_down()` both call `clear_chat_selection()` - must use new methods for auto-scroll
- Event loop only processes mouse events if `action != Noop`

**Coordinate Mapping** (`src/app/runtime/key_actions.rs:23-70`):
- `mouse_to_chat_visual_position()` maps mouse coords to (visual_line, col)
- Handles alignment offsets (Left/Right/Center)
- Returns `None` if outside rendered text span

**Selection Handlers** (`src/app/runtime/key_actions.rs`):
- `handle_start_selection()` - Sets anchor and current
- `handle_update_selection()` - Updates current during drag
- `handle_end_selection()` - Sets `is_selecting = false`

**Event Loop** (`src/app/runtime/event_loop.rs:65-89`):
```rust
Event::Mouse(mouse) => {
    let action = resolve_mouse_event(mouse);
    if action != InputAction::Noop {
        // Overlay check (returns early if overlay consumes action)
        // ...
        handle_action(app, action, Some(mouse), ...)?;
    }
}
```

**Important Notes from Verification:**
- `MouseEvent` has `modifiers: KeyModifiers` field for Shift detection
- Overlays consume all mouse actions when active (correct behavior)
- `chat_visual_lines` contains ALL reflowed lines (not just visible)
- `scroll_up()` and `scroll_down()` both call `clear_chat_selection()` - must use new methods for auto-scroll
- Event loop only processes mouse events if `action != Noop`

---

## Feature 1: Drag-to-Scroll Selection

### Problem
Users cannot select text beyond the visible viewport while dragging because there's no auto-scroll mechanism.

### Solution
When the mouse is near the top/bottom edge of the chat area during a drag, automatically scroll in that direction while maintaining the selection.

### Implementation Details

#### 1.1 Add Auto-Scroll State to App

**File**: `src/app/mod.rs`

Add fields:
```rust
auto_scroll_direction: Option<i16>,  // -1 for up, +1 for down, None for idle
auto_scroll_speed: u16,              // Lines to scroll per tick
```

Initialize in `App::new()`:
```rust
auto_scroll_direction: None,
auto_scroll_speed: 3,  // Default: 3 lines per tick
```

#### 1.2 Detect Edge Proximity During Drag

**File**: `src/app/runtime/key_actions.rs`

Modify `handle_update_selection()`:
```rust
fn handle_update_selection(app: &mut App, mouse: MouseEvent) {
    if app.is_selecting {
        if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
            app.chat_selection_current = Some(pos);
        }
        
        // Check if mouse is near chat area edges for auto-scroll
        if let Some(area) = app.chat_area {
            let mouse_row = mouse.row;
            let chat_top = area.y;
            let chat_bottom = area.y + area.height - 1;
            
            // Edge threshold: 3 rows from top/bottom
            const EDGE_THRESHOLD: u16 = 3;
            
            if mouse_row <= chat_top + EDGE_THRESHOLD && mouse_row > chat_top {
                // Near top edge - scroll up
                app.auto_scroll_direction = Some(-1);
            } else if mouse_row >= chat_bottom - EDGE_THRESHOLD && mouse_row < chat_bottom {
                // Near bottom edge - scroll down
                app.auto_scroll_direction = Some(1);
            } else {
                // Outside edge zone - stop auto-scroll
                app.auto_scroll_direction = None;
            }
        }
    }
}
```

**Note**: `handle_update_selection()` already receives `MouseEvent` as a parameter. The mouse event is passed through `handle_action()` in the event loop.
```

**Note**: `handle_update_selection()` already receives `MouseEvent` as a parameter. The mouse event is passed through `handle_action()` in the event loop.
```

#### 1.3 Apply Auto-Scroll During Tick

**File**: `src/app/mod.rs`

Add auto-scroll processing in the existing `tick()` method:

```rust
pub fn tick(&mut self) {
    self.spinner_frame = self.spinner_frame.wrapping_add(1);
    
    // ... existing toast/pending expiration logic ...
    
    // Apply auto-scroll if active during selection drag
    if let Some(direction) = self.auto_scroll_direction {
        if direction < 0 {
            self.scroll_up_preserve_selection(self.auto_scroll_speed);
        } else {
            self.scroll_down_preserve_selection(self.auto_scroll_speed);
        }
    }
}
```

**Tick Rate**: The event loop calls `tick()` every 80ms (12.5 Hz). With `auto_scroll_speed = 3`, this gives ~37.5 lines/second scroll speed during drag-to-scroll.

**Important**: `scroll_up()` and `scroll_down()` both call `clear_chat_selection()` at the start. We must create new methods that preserve selection.

**Option B (Recommended)**: Create new methods:
```rust
fn scroll_up_preserve_selection(&mut self, amount: u16) {
    let max_scroll = self.chat_max_scroll_hint.get();
    self.chat_scroll = self.chat_scroll
        .min(max_scroll)
        .saturating_add(amount)
        .min(max_scroll);
}

fn scroll_down_preserve_selection(&mut self, amount: u16) {
    self.chat_scroll = self.chat_scroll
        .min(self.chat_max_scroll_hint.get())
        .saturating_sub(amount);
}
```

#### 1.4 Stop Auto-Scroll on Mouse Up

**File**: `src/app/runtime/key_actions.rs`

Modify `handle_end_selection()`:
```rust
fn handle_end_selection(app: &mut App) {
    app.is_selecting = false;
    app.auto_scroll_direction = None;  // Stop auto-scroll
}
```

#### 1.5 Handle Drag Outside Chat Area

**Edge case**: User drags mouse outside the chat area (e.g., into sidebar or status bar). 

**Current behavior**: `mouse_to_chat_visual_position()` returns `None` when mouse is outside `chat_area`. Selection current position won't update.

**Auto-scroll behavior**: Should continue if mouse is near top/bottom edge of chat area, even if outside horizontally.

**Solution**: Modify edge detection to only check vertical position, not horizontal:
```rust
// Check vertical edge proximity regardless of horizontal position
if mouse_row <= chat_top + EDGE_THRESHOLD && mouse_row > chat_top {
    app.auto_scroll_direction = Some(-1);
} else if mouse_row >= chat_bottom - EDGE_THRESHOLD && mouse_row < chat_bottom {
    app.auto_scroll_direction = Some(1);
} else {
    app.auto_scroll_direction = None;
}
```

This allows users to drag to the side and still have auto-scroll work, which is standard terminal behavior.

#### 1.5 Handle Selection Across Scroll Boundaries

**Challenge**: When scrolling, the `chat_visual_lines` buffer changes. The selection anchor and current positions are indices into this buffer.

**Solution**: The selection indices remain valid because:
- `chat_visual_lines` is the full buffer of all visual lines
- Scrolling only changes which portion is visible
- The indices (visual_line, col) reference the buffer, not the viewport

**Verification needed**: Confirm that `chat_visual_lines` contains ALL lines, not just visible ones.

Looking at `render_chat_panel()`:
```rust
let visual_lines = manual_reflow(&logical_lines, chat_width);
let plain_lines: Vec<(String, Option<Alignment>)> = visual_lines
    .iter()
    .map(|line| (line_to_string(line), line.alignment))
    .collect();
app.update_chat_area_metadata(split[0], plain_lines, max_scroll);
```

✅ **Confirmed**: `plain_lines` contains ALL reflowed lines, not just visible ones. Selection indices remain valid across scrolls.

---

## Feature 2: Shift+Click Range Selection

### Problem
Dragging is imprecise for large selections. Users need a click-to-set-anchor, Shift+click-to-select-end workflow.

### Solution
Add Shift+LeftClick as a new input action that extends the selection from the current anchor to the click position.

### Implementation Details

#### 2.1 Add Shift+Click Detection

**File**: `src/app/input.rs`

Modify `resolve_mouse_event()`:
```rust
pub fn resolve_mouse_event(event: MouseEvent) -> InputAction {
    match event.kind {
        MouseEventKind::Down(MouseButton::Right) => InputAction::Paste,
        MouseEventKind::Down(MouseButton::Left) => {
            if event.modifiers.contains(KeyModifiers::SHIFT) {
                InputAction::ExtendSelection
            } else {
                InputAction::StartSelection
            }
        }
        MouseEventKind::Up(MouseButton::Left) => InputAction::EndSelection,
        MouseEventKind::Drag(MouseButton::Left) => InputAction::UpdateSelection,
        MouseEventKind::ScrollUp => InputAction::MouseWheel(1),
        MouseEventKind::ScrollDown => InputAction::MouseWheel(-1),
        _ => InputAction::Noop,
    }
}
```

#### 2.2 Add New InputAction Variant

**File**: `src/app/input.rs`

Add to `InputAction` enum:
```rust
pub enum InputAction {
    // ... existing variants
    ExtendSelection,  // Shift+LeftClick
    // ...
}
```

#### 2.3 Add ExtendSelection Handler

**File**: `src/app/runtime/key_actions.rs`

Add to `handle_action()` match:
```rust
crate::app::input::InputAction::ExtendSelection => {
    handle_extend_selection(app, mouse_event);
}
```

**Note**: `handle_action()` already accepts `mouse_event: Option<MouseEvent>` as a parameter. The event loop passes `Some(mouse)` for all mouse events.

Implement handler:
```rust
fn handle_extend_selection(app: &mut App, mouse: MouseEvent) {
    // If there's an existing anchor, extend selection to click position
    if app.chat_selection_anchor.is_some() {
        if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
            app.chat_selection_current = Some(pos);
            app.is_selecting = true;  // Keep selection active for potential further extension
        }
    } else {
        // No anchor exists - treat as regular click
        handle_start_selection(app, mouse);
    }
}
```

#### 2.4 Handle Shift+Click on Empty/No-Session Views

**Edge case**: If user Shift+clicks when `chat_area` is `None` (welcome/no-session screens), the handler should gracefully do nothing.

**Solution**: `mouse_to_chat_visual_position()` already returns `None` when `chat_area.is_none()`, so the handler will naturally handle this.

---

## Testing Strategy

### Unit Tests

#### Drag-to-Scroll Tests
1. `auto_scroll_activates_near_top_edge` - Verify direction set to -1 when mouse near top
2. `auto_scroll_activates_near_bottom_edge` - Verify direction set to +1 when mouse near bottom
3. `auto_scroll_deactivates_in_middle` - Verify direction cleared when mouse moves away from edges
4. `auto_scroll_preserves_selection_during_scroll` - Verify selection not cleared during auto-scroll
5. `auto_scroll_stops_on_mouse_up` - Verify direction cleared on EndSelection

#### Shift+Click Tests
6. `shift_click_extends_existing_selection` - Verify selection extended from anchor
7. `shift_click_without_anchor_starts_selection` - Verify acts as StartSelection when no anchor
8. `shift_click_outside_chat_area_does_nothing` - Verify graceful handling

#### Integration Tests
9. `drag_scroll_selection_across_viewport` - Full drag-to-scroll workflow
10. `shift_click_range_selection_large_span` - Full Shift+click workflow

### Manual Testing Checklist
- [ ] Drag from middle of paragraph to bottom edge
- [ ] Verify auto-scroll activates and selection extends
- [ ] Release mouse to end selection
- [ ] Verify Ctrl+C copies correct text
- [ ] Click to set anchor
- [ ] Shift+click elsewhere to select range
- [ ] Verify selection highlights correctly
- [ ] Test with right/center-aligned messages
- [ ] Test with wrapped lines
- [ ] Test with empty lines
- [ ] Test on welcome/no-session screens (should do nothing)

---

## File Touch List

| File | Changes |
|------|---------|
| `src/app/input.rs` | Add `ExtendSelection` variant, modify `resolve_mouse_event()` |
| `src/app/mod.rs` | Add `auto_scroll_direction`, `auto_scroll_speed` fields, `scroll_up_preserve_selection()`, `scroll_down_preserve_selection()`, update `tick()` |
| `src/app/runtime/key_actions.rs` | Modify `handle_update_selection()`, `handle_end_selection()`, add `handle_extend_selection()`, update `handle_action()` |
| `tests/input_keybindings.rs` | Add Shift+click test |
| `CHANGELOG.md` | Document new features |

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Auto-scroll too fast/slow | Poor UX | Make speed configurable, start with conservative default (3 lines/tick) |
| Selection indices become stale after scroll | Copy wrong text | Verify `chat_visual_lines` contains all lines (confirmed) |
| Shift+click conflicts with terminal | Feature doesn't work | Raw mode captures all key combos; verify on Windows terminal |
| Edge detection too sensitive | Accidental auto-scroll | Use 3-row threshold, test on various terminal sizes |
| Performance with large selections | Jank during drag | Auto-scroll only triggers on tick (60fps), minimal overhead |

---

## Implementation Order

1. **Feature 2 first** (Shift+Click): Simpler, fewer moving parts
   - Add `ExtendSelection` variant
   - Implement handler
   - Add tests
   - Verify

2. **Feature 1 second** (Drag-to-Scroll): More complex, depends on stable selection
   - Add auto-scroll state
   - Implement edge detection
   - Add scroll methods that preserve selection
   - Integrate with tick
   - Add tests
   - Verify

3. **Integration testing**: Test both features together

---

## Verified Corrections

After checking the plan against the current codebase, the following corrections were made:

### ✅ Confirmed Accurate
1. **Event loop structure**: Mouse events are properly routed through `resolve_mouse_event()` → overlay check → `handle_action()`
2. **Tick integration**: `tick()` is called every 80ms in event loop, correct place for auto-scroll
3. **chat_visual_lines**: Contains ALL reflowed lines, not just visible ones. Selection indices remain valid across scrolls
4. **MouseEvent modifiers**: Has `modifiers: KeyModifiers` field for Shift detection
5. **Overlay behavior**: Overlays consume all mouse actions when active (correct)
6. **handle_action signature**: Already accepts `Option<MouseEvent>` parameter
7. **handle_update_selection**: Already receives `MouseEvent` parameter

### ⚠️ Corrections Applied
1. **scroll_up/scroll_down clear selection**: Both methods call `clear_chat_selection()` at start. Must create new `scroll_up_preserve_selection()` and `scroll_down_preserve_selection()` methods
2. **Event loop Noop filtering**: Only processes mouse events if `action != Noop`. `ExtendSelection` must return non-Noop action (plan correctly identifies this)
3. **Tick rate**: 80ms (12.5 Hz), not 60fps. Auto-scroll speed calculation adjusted accordingly
4. **Drag outside chat area**: Edge detection should check vertical position only, allowing auto-scroll even when mouse is horizontally outside chat area

### 📋 Implementation Notes
- All file paths verified against current codebase structure
- No conflicts with existing App struct fields
- No changes needed to rendering pipeline (selection highlighting already works with indices)
- Tests should cover edge cases: empty chat, welcome screen, no-session screen, overlays active

---

## Success Criteria

- [ ] Users can drag-select text beyond viewport with auto-scroll
- [ ] Users can Shift+click to select ranges precisely
- [ ] Selection highlighting remains accurate during auto-scroll
- [ ] Copied text matches visual selection exactly
- [ ] All existing tests pass (no regressions)
- [ ] New tests cover edge cases (drag-to-scroll, Shift+click, margin clicks, overlays)
- [ ] **Full project checks pass**:
  - `cargo fmt --all --check` (formatting)
  - `cargo clippy --all-targets --all-features -- -D warnings` (linting)
  - `cargo test --all-targets --all-features` (all tests)
  - `./scripts/check.ps1` (full check script)
  - `cargo build` (debug build)
- [ ] Manual testing confirms smooth UX on Windows terminal
- [ ] CHANGELOG.md updated with new features
