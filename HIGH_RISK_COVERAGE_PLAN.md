# High-Risk Test Coverage Plan (Final)

## Status: COMPLETED ✅

---

## Goal
Add critical unit tests for high-risk areas with insufficient coverage.

---

## Implementation Summary

### Modules Added

| Module | Tests | Description |
|--------|-------|-------------|
| `src/app/mod_tests/toast.rs` | 4 | Toast lifecycle, expiration, replacement |
| `src/app/mod_tests/input_editor.rs` | 6 | Input editor cursor movement, backspace, newline, long lines |
| `src/app/mod_tests/pending.rs` | 3 | Pending delete/confirmation expiration via tick |
| `src/app/mod_tests/model_switching.rs` | 5 | Model selection via sidebar, /model command, context refresh check |

**Total new tests:** 18

---

## Tests Implemented

### Toast System

- `toast_creation_preserves_level_and_message`
- `toast_dismissal_via_dismiss_overlay`
- `toast_expires_after_tick`
- `toast_multiple_sets_replace`

### Input Editor Operations

- `input_backspace_at_start_does_not_panic`
- `input_insert_newline_resets_caret_to_line_start`
- `input_cursor_left_moves_back_one_char`
- `input_cursor_right_moves_forward_one_char`
- `input_vertical_movement_preserves_preferred_column`
- `input_long_line_editing_maintains_cursor_position`

### Pending Expiration (tick-based)

- `pending_workspace_delete_expires_after_tick`
- `pending_session_delete_expires_after_tick`
- `pending_tool_confirmation_expires_after_tick`

### Model Switching

- `sidebar_model_selection_switches_active_model`
- `model_command_updates_model_from_argument`
- `model_command_unknown_sets_model_with_info_toast`
- `command_may_change_model_context_true_for_model`
- `command_may_change_model_context_true_for_url`

---

## Files Modified

| File | Change |
|------|--------|
| `src/app/mod_tests/mod.rs` | Added 4 new module declarations |
| `src/app/mod_tests/toast.rs` | New file |
| `src/app/mod_tests/input_editor.rs` | New file |
| `src/app/mod_tests/pending.rs` | New file |
| `src/app/mod_tests/model_switching.rs` | New file |

---

## Verification Results

```
cargo test --lib
  ✓ 164 tests passed (up from 146)

cargo test --all-targets --all-features
  ✓ 341 tests passed (up from 323)

cargo clippy --all-targets --all-features -- -D warnings
  ✓ No warnings

cargo fmt --all -- --check
  ✓ Formatting OK
```

---

## Notes

- **Excluded**: Connection status bootstrap tests (requires integration-level mocking)
- **Excluded**: TUI render tests (functions not accessible from `mod_tests`)
- **Simplified**: Input editor tests based on actual method behavior
- All tests are unit tests in `src/app/mod_tests/` following existing patterns

---

## Acceptance Criteria Met

- [x] All 4 modules added and registered
- [x] All 18 tests implemented and passing
- [x] `cargo test --lib` passes
- [x] `cargo test --all-targets --all-features` passes
- [x] `cargo clippy -- -D warnings` passes
- [x] `cargo fmt -- --check` passes
- [x] No `#[ignore]` tests
- [x] Total new tests: 18

---

## Risk Coverage Gained

| Area | Risk Level | Coverage Gained |
|------|------------|-----------------|
| Toast lifecycle | **High** | ✅ Complete (creation, dismissal, expiration) |
| Input editor operations | **High** | ✅ Comprehensive (movement, edit, wrap, long lines) |
| Pending entities expiration | **High** | ✅ Complete (workspace/session/tool) |
| Model switching | **High** | ✅ Sidebar + command + context refresh check |
| Context window refresh | Medium | ⚠️ Deferred (integration test needed) |
| Connection status | Medium | ⚠️ Deferred (bootstrap integration complex) |

---

**Conclusion:** High-risk test coverage significantly improved. No further immediate work needed.
