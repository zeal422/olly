# Mouse Wheel Scroll Plan

## Goal
Enable mouse wheel scrolling in the chat window area for natural reading/navigation.

## Scope
- Enable terminal mouse tracking.
- Capture mouse wheel events.
- Route wheel scrolls to chat panel scroll offset.
- No focus changes required (wheel always scrolls chat).

## Out of Scope
- Mouse click-to-focus (separate feature).
- Horizontal scrolling.
- Scrollbar UI decoration.

## Implementation Phases

### Phase 1: Mouse Capture Lifecycle
**Files**: `src/app/runtime/startup.rs`, `src/app/runtime/shutdown.rs`
- Add `EnableMouseCapture` to terminal init flow using `execute!` (same style as alternate-screen setup).
- Add `DisableMouseCapture` to terminal teardown flow (before/with leaving alternate screen).
- Keep raw-mode and cursor behavior unchanged.

Exit criteria:
- Mouse events are delivered while app is running.
- Mouse capture is always released on shutdown.

### Phase 2: Mouse Event to InputAction Mapping
**Files**: `src/app/input.rs`, `src/app/runtime/event_loop.rs`
- Add a dedicated mouse-wheel action variant in `InputAction` (for example `MouseWheel(i16)`).
- Add a mouse-event resolver helper in `input.rs` (do not overload `resolve_key_event_with_bindings`, which is key-only).
- In event loop, handle `Event::Mouse(mouse)` and map:
  - `WheelUp` -> wheel-up action.
  - `WheelDown` -> wheel-down action.
  - other mouse events -> `Noop`.
- In `event_loop.rs`, replace the current catch-all-only mouse handling path by adding an explicit `Event::Mouse(mouse)` match arm, then keep `_ => {}` for other unhandled events.
- Keep existing key-event path unchanged.

Exit criteria:
- Wheel events produce app actions through the same overlay/key-action pipeline used by keyboard actions.

### Phase 3: Route Wheel to Chat Scroll (Focus-Independent)
**File**: `src/app/runtime/key_actions.rs`
- Add handling for wheel action that always calls chat scroll methods:
  - up -> `app.scroll_up(3)` (3 lines per wheel notch)
  - down -> `app.scroll_down(3)` (3 lines per wheel notch)
- Do not use `resolve_scroll_route` for wheel (keyboard arrows/page keys keep current behavior).

Exit criteria:
- Wheel scroll always affects chat panel regardless of sidebar/chat focus.

### Phase 4: Clamp/Behavior Validation
**File**: `src/app/mod.rs` (validation only unless needed)
- Confirm existing `scroll_up`/`scroll_down` clamping already prevents underflow/overflow.
- Keep `AUTO_SCROLL_TARGET` unchanged unless a concrete issue appears.

Exit criteria:
- No scroll bound regressions.

## Testing Checklist
- [ ] Unit tests for mouse-event mapping (`WheelUp`/`WheelDown`/non-wheel -> expected action).
- [ ] Unit test in key-action routing: wheel action scrolls chat and bypasses sidebar/input-caret route logic.
- [ ] Mouse wheel scrolls chat up/down in small increments.
- [ ] Wheel works when chat has focus.
- [ ] Wheel works when sidebar has focus (still scrolls chat).
- [ ] Wheel stops at top/bottom (no underflow/overflow).
- [ ] Overlay behavior remains safe (active modals still intercept/consume actions as today).
- [ ] `cargo fmt --all` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --all-targets --all-features` passes.

## Rollout
- Single commit: `feat: add mouse wheel scrolling in chat panel`.
- No config needed (enabled by default).
- No migration needed.

## Risks
- Low: event plumbing touches startup/shutdown and central event loop.
- Potential: future click handling can conflict if mouse mapping expands without clear action separation.
- Mitigation: keep only wheel kinds active now; map other mouse events to `Noop`.

## Done Criteria
- Mouse wheel reliably scrolls chat messages in both directions.
- Keyboard scrolling behavior is unchanged.
- Terminal mouse capture is cleaned up on exit.
- All existing tests pass.
- No clippy warnings.
