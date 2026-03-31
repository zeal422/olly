# Test Implementation Plan

## Overview

This plan outlines unit tests to be implemented for the Olly TUI codebase, prioritized by risk and value. Coverage gaps were identified by analyzing the uncommitted IMP branch changes for mouse-wheel scrolling support.

**Corrections from initial draft:**
- Priority 7 (parser) tests removed - tests already exist with correct behavior
- Priority 2 (event loop timing) marked as requires refactoring - cannot mock `Instant` without changes
- Priority 4 (TUI scroll rendering) marked as requires refactoring - stateless render function not directly testable
- Priority 3 (mouse capture) moved to integration test category
- Priority 5 overlay tests corrected based on actual overlay handling flow

---

## Priority 1: Scroll Semantics Tests

**Location**: `src/app/runtime/key_actions.rs` (add to existing `#[cfg(test)] mod tests`)

**Rationale**: The from-bottom scroll semantics are a core behavioral change. Bugs here affect the primary user interaction (chat scrolling).

### Tests to Add

1. **`scroll_up_from_zero_does_nothing`**
   - Set `chat_scroll = 0`, `max_scroll_hint = 500`
   - Call `app.scroll_up(30)`
   - Verify `chat_scroll == 0`

2. **`scroll_up_increases_correctly`**
   - Set `chat_scroll = 100`, `max_scroll_hint = 500`
   - Call `app.scroll_up(30)`
   - Verify `chat_scroll == 130`

3. **`scroll_up_caps_at_max_scroll`**
   - Set `chat_scroll = 490`, `max_scroll_hint = 500`
   - Call `app.scroll_up(50)`
   - Verify `chat_scroll == 500` (capped, not 540)

4. **`scroll_down_from_max_does_nothing`**
   - Set `chat_scroll = 500`, `max_scroll_hint = 500`
   - Call `app.scroll_down(30)`
   - Verify `chat_scroll == 500`

5. **`scroll_down_decreases_correctly`**
   - Set `chat_scroll = 100`, `max_scroll_hint = 500`
   - Call `app.scroll_down(30)`
   - Verify `chat_scroll == 70`

6. **`scroll_down_caps_at_zero`**
   - Set `chat_scroll = 10`, `max_scroll_hint = 500`
   - Call `app.scroll_down(50)`
   - Verify `chat_scroll == 0`

7. **`clamp_chat_scroll_to_hint_reduces_when_above_max`**
   - Set `chat_scroll = 600`, `max_scroll_hint = 500`
   - Call `app.clamp_chat_scroll_to_hint()`
   - Verify `chat_scroll == 500`

8. **`clamp_chat_scroll_to_hint_keeps_valid_value`**
   - Set `chat_scroll = 100`, `max_scroll_hint = 500`
   - Call `app.clamp_chat_scroll_to_hint()`
   - Verify `chat_scroll == 100`

9. **`update_chat_max_scroll_hint_updates_cell`**
   - Call `app.update_chat_max_scroll_hint(250)`
   - Subsequent scroll operations should use 250 as max

10. **`apply_mouse_wheel_scroll_scrolls_chat`**
    - Set `chat_scroll = 100`, `max_scroll_hint = 500`
    - Call `apply_mouse_wheel_scroll(app, 1)` (scroll up)
    - Verify `chat_scroll == 103` (1 * MOUSE_WHEEL_SCROLL_STEP where step=3)

11. **`apply_mouse_wheel_scroll_down_caps_at_zero`**
    - Set `chat_scroll = 0`, `max_scroll_hint = 500`
    - Call `apply_mouse_wheel_scroll(app, -1)` (scroll down)
    - Verify `chat_scroll == 0`

---

## Priority 2: Mouse Wheel Routing Tests

**Location**: `src/app/runtime/key_actions.rs`

**Rationale**: Mouse wheel routing determines whether wheel events scroll chat, sidebar, or command suggestions.

### Tests to Add

1. **`mouse_wheel_routes_to_chat_when_sidebar_focused`**
   - Set `app.focus = FocusPane::Workspaces`
   - Call `apply_mouse_wheel_scroll(app, 1)`
   - Verify chat_scroll changes (not sidebar selection)

2. **`mouse_wheel_scroll_step_applies_multiplier`**
   - Set `chat_scroll = 0`, `max_scroll_hint = 500`
   - Call `apply_mouse_wheel_scroll(app, 2)` (two notch scroll up)
   - Verify `chat_scroll == 6` (2 * 3 step)

3. **`rapid_mouse_wheel_events_accumulate`**
   - Set `chat_scroll = 0`, `max_scroll_hint = 500`
   - Call `apply_mouse_wheel_scroll(app, 1)` three times rapidly
   - Verify `chat_scroll == 9` (3 * 3 step)

4. **`mouse_wheel_ignored_when_command_suggestion_mode`**
   - Enable command suggestion mode (set `app.input` to start with `/`)
   - Call `apply_mouse_wheel_scroll(app, 1)`
   - Verify chat_scroll UNCHANGED (route goes to command suggestions instead)

**Note on overlays**: Mouse events flow through `handle_overlay_action` first. If an overlay consumes the event (returns true), the wheel scroll does NOT happen. Test that overlay consumption properly blocks scrolling by verifying the overlay handling logic.

---

## Priority 3: Input Action Resolution Tests

**Location**: `src/app/input.rs`

**Rationale**: Input resolution converts crossterm events to app actions. New MouseWheel action needs coverage.

### Tests to Add

1. **`resolve_mouse_event_scroll_up_returns_positive`**
   - Create `MouseEvent { kind: ScrollUp, ... }`
   - Verify `resolve_mouse_event()` returns `InputAction::MouseWheel(1)`

2. **`resolve_mouse_event_scroll_down_returns_negative`**
   - Create `MouseEvent { kind: ScrollDown, ... }`
   - Verify `resolve_mouse_event()` returns `InputAction::MouseWheel(-1)`

3. **`resolve_mouse_event_other_ignored`**
   - Create `MouseEvent { kind: Moved, ... }`
   - Verify `resolve_mouse_event()` returns `InputAction::Noop`

4. **`resolve_mouse_event_drag_ignored`**
   - Create `MouseEvent { kind: Drag, ... }`
   - Verify returns `Noop`

---

## Priority 4: App Scroll State Tests

**Location**: `src/app/mod.rs` (add to existing `mod_tests` section)

**Rationale**: App manages scroll state transitions during message flow.

### Tests to Add

1. **`push_assistant_resets_scroll_to_bottom`**
   - Set `app.chat_scroll = 500`
   - Call `app.push_assistant(...)`
   - Verify `app.chat_scroll == 0`

2. **`push_user_resets_scroll_to_bottom`**
   - Set `app.chat_scroll = 500`
   - Call `app.push_user(...)`
   - Verify `app.chat_scroll == 0`

3. **`scroll_up_on_empty_chat_does_not_panic`**
   - Set `max_scroll_hint = 0` (empty chat)
   - Call `app.scroll_up(30)`
   - Verify no panic, scroll stays at 0

4. **`scroll_down_on_empty_chat_caps_at_zero`**
   - Set `max_scroll_hint = 0`
   - Call `app.scroll_down(30)`
   - Verify no panic, scroll stays at 0

---

## Priority 5: TUI Scroll Calculation

**Location**: `src/app/tui/mod.rs`

**Note**: The scroll calculation in `render_chat_panel` is a stateless function that cannot be directly unit tested. Two options:

### Option A: Extract Testable Function (Recommended)

Extract scroll calculation into a pure function:
```rust
pub fn calculate_scroll_position(chat_scroll: u16, max_scroll: u16) -> u16 {
    let from_bottom = chat_scroll.min(max_scroll);
    max_scroll.saturating_sub(from_bottom)
}
```

Add tests for boundary conditions:
- `max_scroll = 0` returns 0
- `chat_scroll = 0` returns `max_scroll`
- `chat_scroll = max_scroll` returns 0
- `chat_scroll > max_scroll` (stale) returns 0

### Option B: Integration Test via Render

Write a render test that creates a terminal, sets app state, renders, and verifies output. More expensive but validates the full pipeline.

---

## Priority 6: Chat Pipeline Reset Tests

**Location**: `src/app/chat_pipeline.rs`

**Rationale**: After streaming completes, scroll should reset to bottom.

### Tests to Add

1. **`streaming_response_callback_resets_scroll_to_zero`**
   - Set `app.chat_scroll = 500`
   - Simulate streaming response callback execution
   - Verify `app.chat_scroll == 0`

2. **`error_callback_also_resets_scroll`**
   - Same as above but via error path callback
   - Verify scroll reset on failure too

---

## Priority 7: Overlay Mouse Event Handling (Requires Verification First)

**Location**: `src/app/runtime/overlays.rs`

**Note**: Before writing these tests, verify whether overlays actually consume mouse wheel events or let them pass through to scroll.

### Tests to Add (If Overlay Handles Wheel)

1. **`help_overlay_consumes_mouse_wheel`**
   - Show help overlay
   - Send `MouseWheel` action
   - Verify overlay returns `true` (consumed)

2. **`tool_confirmation_overlay_consumes_mouse_wheel`**
   - Show tool confirmation modal
   - Send `MouseWheel` action
   - Verify overlay returns `true` (consumed)

### If Overlay Does NOT Handle Wheel

Then mouse wheel WILL scroll chat even with overlay active - which is actually fine behavior. In that case, no tests needed for blocking.

---

## Priority 8: Integration Tests (New File)

**Location**: `tests/scroll_integration.rs` (new file)

**Rationale**: Some behaviors require real terminal/event loop and can't be unit tested.

### Tests to Add

1. **`mouse_capture_enabled_on_startup`**
   - Verify startup sequence calls `EnableMouseCapture`
   - Use command spy or integration test

2. **`mouse_capture_disabled_on_shutdown`**
   - Verify shutdown sequence calls `DisableMouseCapture`
   - Use command spy or integration test

3. **`event_loop_tick_runs_at_fixed_interval`**
   - Integration test that measures time between ticks
   - Verify ~80ms intervals
   - Mark as `#[ignore]` by default (slow/flaky)

---

## Implementation Notes

### Test File Structure

- Unit tests in same file as code (preferred for Rust)
- Integration tests in `tests/` directory
- Use `#[cfg(test)]` modules for unit tests
- Follow existing test naming conventions: `feature_behavior_expected`

### Mocking Strategy

- For scroll tests: Use `Cell<u16>` via existing `chat_max_scroll_hint`
- For time-based tests: Consider extracting time to a trait if truly needed
- Avoid heavy mocking; test actual behavior where possible

### Test Data Fixtures

- Use deterministic data (no random generation)
- Reuse existing fixtures in `tests/common/`
- Keep test messages short to avoid bloating test execution time

---

## Files to Modify

| File | Tests to Add |
|------|--------------|
| `src/app/mod.rs` | App scroll state tests (Priority 4) |
| `src/app/runtime/key_actions.rs` | Scroll semantics, mouse wheel routing (Priority 1, 2) |
| `src/app/input.rs` | Input resolution (Priority 3) |
| `src/app/tui/mod.rs` | Scroll calculation (Priority 5) - requires extraction |
| `src/app/chat_pipeline.rs` | Scroll reset on streaming (Priority 6) |
| `src/app/runtime/overlays.rs` | Overlay mouse handling (Priority 7) - verify first |
| `tests/scroll_integration.rs` | Integration tests (Priority 8) - new file |

---

## Acceptance Criteria

- [ ] All Priority 1-3 tests implemented and passing
- [ ] All Priority 4 tests implemented and passing
- [ ] Priority 5 implemented via function extraction or integration test
- [ ] Priority 6-8 implemented if applicable
- [ ] No new `#[ignore]` tests
- [ ] All tests pass: `cargo test --all-targets --all-features`
- [ ] No clippy warnings in test code
- [ ] Run quality checks: `./scripts/check.ps1`
