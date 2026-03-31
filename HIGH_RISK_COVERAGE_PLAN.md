# High-Risk Test Coverage Plan (Final)

## Goal
Add critical unit tests for high-risk areas with insufficient coverage.

---

## 1. Toast System

**Files:**
- `src/app/mod_tests/toast.rs` (new)
- `src/app/mod_tests/mod.rs` (add `mod toast`)

**Tests:**

| Test | Description |
|------|-------------|
| `toast_creation_preserves_level_and_message` | `set_toast(level, msg)` stores correctly |
| `toast_dismissal_via_dismiss_overlay` | `dismiss_overlay()` clears toast |
| `toast_expires_after_tick` | Set `expires_at` to past, call `tick()`, toast cleared |
| `toast_multiple_sets_replace` | Second `set_toast` overwrites first |
| `toast_render_without_panic` | Call `tui::render_app_ui` with toast, verify no panic |

**Notes:**
- Toast expires after 4 seconds (line 567 in mod.rs)
- `tick()` clears expired toasts (line 458-461)
- Render test: create mock frame, call `tui::render_app_ui`, capture output

---

## 2. Model Switching

**Files:**
- `src/app/mod_tests/model_switching.rs` (new)
- `src/app/mod_tests/mod.rs` (add `mod model_switching`)

**Tests:**

| Test | Description |
|------|-------------|
| `sidebar_model_selection_switches_active_model` | `activate_sidebar_selection(manager, SidebarItem::Model(idx))` updates `app.model` |
| `model_command_updates_model_from_argument` | `execute_slash_command(app, "/model llama3.2:latest")` sets model |
| `model_command_invalid_shows_error_toast` | `execute_slash_command(app, "/model unknown")` shows error |
| `command_may_change_model_context_true_for_model` | `command_may_change_model_context("/model")` returns true |
| `command_may_change_model_context_true_for_url` | `command_may_change_model_context("/url http://...")` returns true |
| `refresh_model_context_window_sets_telemetry` | Mock Ollama returns context window, verify `app.model_context_window` |

**Notes:**
- Use `MockOllamaServer` for async refresh test (integration-like but in unit test)
- `activate_sidebar_selection` is `pub(crate)` in mod.rs (line 1124)
- `execute_slash_command` is `pub(crate)` in commands/execute.rs

---

## 3. Input Editor Operations

**Files:**
- `src/app/mod_tests/input_editor.rs` (new)
- `src/app/mod_tests/mod.rs` (add `mod input_editor`)

**Tests:**

| Test | Description |
|------|-------------|
| `input_backspace_at_start_does_not_panic` | Empty buffer, `backspace_input_char()` safe |
| `input_insert_newline_resets_caret_to_line_start` | Insert '\n', cursor moves to line start |
| `input_cursor_left_at_line_start_wraps_to_prev_line_end` | "abc\ndef", cursor at 3 (index of 'd'), left → 2 |
| `input_cursor_right_at_line_end_moves_to_next_line_start` | "abc\ndef", cursor at 2, right → 4 |
| `input_vertical_movement_preserves_preferred_column` | Up/down on wrapped line uses `input_cursor_preferred_column` |
| `input_long_line_editing_maintains_cursor_position` | Insert/delete on long line (>viewport width) |

**Notes:**
- Methods: `insert_input_char`, `backspace_input_char`, `move_input_cursor_left/right/up/down`, `insert_newline_at_cursor`
- Example: `app.set_input_text("abc\ndef"); app.move_input_cursor_right();` etc.

---

## 4. Connection Status (Bootstrap Integration)

**Files:**
- `src/app/mod_tests/startup_connection.rs` (new)
- `src/app/mod_tests/mod.rs` (add `mod startup_connection`)

**Tests:**

| Test | Description |
|------|-------------|
| `bootstrap_ping_success_sets_connection_online` | `startup::bootstrap()` with mock 200 → `app.connection_status == Online` |
| `bootstrap_ping_failure_sets_connection_offline` | `startup::bootstrap()` with mock error → `app.connection_status == Offline` |
| `footer_renders_connection_status_without_panic` | Render footer with `Offline`, check output contains "Offline" |

**Notes:**
- `runtime::startup::bootstrap` performs the ping (line 85-94 in startup.rs)
- Use `MockOllamaServer::spawn_once(200, "{}")` for success
- For failure: spawn server that closes connection or returns 500
- Footer render: call `tui::render_footer` with mock frame and `AppFooterState`

---

## Acceptance Criteria

- [ ] All 4 modules added to `mod_tests/mod.rs`
- [ ] All ~20 tests implemented and passing
- [ ] `cargo test --lib` passes
- [ ] `cargo test --all-targets --all-features` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] No `#[ignore]` tests
- [ ] Total new tests: 20-25

---

## Implementation Order

1. Toast (simplest)
2. Input editor (no external deps)
3. Model switching (uses MockOllamaServer)
4. Startup connection (uses MockOllamaServer)

---

## Feasibility Verified

- All referenced methods exist and are `pub(crate)`
- Test access via `mod_tests` works (already used for other tests)
- `MockOllamaServer` available in `tests/common/` and usable in `mod_tests` (already imported in workspace_session tests)
- TUI render functions can be called with mock `Frame` (see existing TUI tests)

---

## Excluded (Lower Risk)

- Theme system: covered indirectly by UI state tests
- Workspace edges: already well-tested in workspace_session.rs
- Configuration: already tested in config_* tests
- Platform helpers: would need cfg(test) hacks, low value
