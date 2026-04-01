# MOD_RS Monolith Reduction Plan

## Objective

Reduce monolith risk in:

- `src/app/mod.rs`
- `src/app/tui/mod.rs`

without changing runtime behavior, UI output, command semantics, persistence format, or keybindings.

## Constraints (Non-Negotiable)

- No user-facing behavior changes.
- No visual regressions in layout/theme rendering.
- No schema changes in SQLite/data files.
- Keep all slash commands and tool behaviors unchanged.
- Refactor in small, reversible phases with checks after every phase.
- LLM/agent must not create commits; version-control commits are user-owned.
- Prefer existing files over creating new ones; only add new modules when a clear cohesion boundary still remains after filling existing stubs.
- Avoid over-fragmentation: prefer a few cohesive modules over many micro-modules.

---

## Verification Status (Updated 2026-04-01)

**Function names verified**: ✅ All Phase 2 function names have been verified against the actual codebase via grep search.

**Key findings from verification**:
1. **Direct field access**: Most `App` struct fields are `pub`, so direct access is common (no getter/setter methods needed).
2. **Helper methods exist**: Core operations use methods like `scroll_up()`, `set_toast()`, `push_user()` - not the approximate names initially documented.
3. **Some methods need creation**: `confirm_pending_*()`, `cancel_pending_*()`, `toast_expired()` don't exist yet - should be created during refactoring.
4. **Naming distinction preserved**: `input_editor.rs` (cursor/text editing) is correctly distinguished from `input.rs` (keybinding resolution).

**See "Appendix: Verified Function Names (Phase 2 Reference)" for exact function names to use during implementation.**

---

## Baseline Snapshot (VERIFIED - Codebase Analysis Complete)

### Baseline LOC (Recorded)
- [x] `src/app/mod.rs`: **1,732 lines**
- [x] `src/app/tui/mod.rs`: **2,194 lines**
- [x] **Combined total**: **3,926 lines**

### Target LOC (Post-Refactor)
- [ ] `src/app/mod.rs`: **< 400 lines** (coordinator/facade only)
- [ ] `src/app/tui/mod.rs`: **< 300 lines** (render orchestration only)
- [ ] **Target reduction**: ~85% LOC moved to focused modules

### Current State of Existing TUI Files (CRITICAL FINDING)
The following files exist but are **thin re-export wrappers** (8-19 lines each), NOT real implementations:

| File | Current LOC | Status |
|------|-------------|--------|
| `src/app/tui/palette.rs` | 9 | Re-exports only |
| `src/app/tui/sidebar.rs` | 19 | Re-exports only |
| `src/app/tui/chat.rs` | 10 | Re-exports only |
| `src/app/tui/input.rs` | 10 | Re-exports only |
| `src/app/tui/markdown.rs` | 8 | Re-exports only |
| `src/app/tui/modals.rs` | 227 | **Full implementation** (keep as-is) |

**Implication**: Phase 1 requires moving ~2,000 lines of actual rendering logic, not just filling stubs.

### Current State of App Module Files
None of the target app module files exist yet:
- [ ] `src/app/state.rs` - does not exist
- [ ] `src/app/session_ui_state.rs` - does not exist
- [ ] `src/app/input_and_navigation.rs` - does not exist
- [ ] `src/app/messages_and_conversation.rs` - does not exist
- [ ] `src/app/workspace_session_help.rs` - does not exist
- [ ] `src/app/model_context_ops.rs` - does not exist

### Baseline Quality Gate Status
- [ ] `cargo fmt --all -- --check` - **TO RUN**
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` - **TO RUN**
- [ ] `cargo test --all-targets --all-features` - **TO RUN**

### Baseline Manual Smoke (TO RECORD)
- [ ] app launch/maximize
- [ ] sidebar navigation
- [ ] send message + stream
- [ ] slash command autocomplete
- [ ] mouse-wheel chat scroll
- [ ] selection copy/paste
- [ ] `/tools` status and policy commands

---

## Key Architecture Insights (From Codebase Analysis)

### Module Dependencies (LOW circular dependency risk)
- Clear hierarchy: `tui/*` → `app/mod.rs` (not vice versa)
- TUI submodules only call parent `mod.rs` functions
- `platform/mod.rs` already well-factored (~280 lines Windows console hardening)

### Test Structure
- `src/app/mod_tests/`: 11 organized test files for app logic
- `src/app/tui/mod.rs`: **22 inline tests** (need extraction with code movement)
- `src/app/tui/modals.rs`: 2 inline tests

### Extraction Candidates Identified

**From `src/app/mod.rs` (1,732 lines)**:
1. **State Module** (~400 lines) - `App` struct (35+ fields), snapshot/restore
2. **Messages Module** (~150 lines) - `push_*` methods
3. **Input Editor Module** (~200 lines) - cursor management, text manipulation
4. **Chat Navigation Module** (~150 lines) - scroll, selection, auto-scroll
5. **Overlay/Toast Module** (~100 lines) - toast, confirmations, help
6. **Theme Picker Module** (~80 lines) - theme picker state machine

**From `src/app/tui/mod.rs` (2,194 lines)**:
1. **Chat rendering** - `render_chat_panel`, footer context strip
2. **Input rendering** - `render_input_panel`, cursor placement
3. **Sidebar rendering** - `render_models_panel`, hints, marquee
4. **Status bar rendering** - top status line
5. **Markdown/syntax** - syntax highlighting helpers
6. **Utilities** - `truncate_middle`, `scroll_window`, `manual_reflow`

### Risks Identified
1. **God Object**: `App` struct has 35+ fields mixing unrelated concerns
2. **Re-export Anti-Pattern**: Current TUI files provide zero modularity benefit
3. **Missing Trait Abstractions**: No traits for state operations
4. **Test Distribution**: 22 inline TUI tests need extraction

---

## Phase 0 - Safety Harness & Refactor Skeleton

### Tasks

- [ ] **Run and record baseline quality gates**:
  - [ ] `cargo fmt --all -- --check` (capture output)
  - [ ] `cargo clippy --all-targets --all-features -- -D warnings` (capture output)
  - [ ] `cargo test --all-targets --all-features` (capture test count and result)
- [ ] **Record manual smoke test baseline** (run app and verify):
  - [ ] app launch/maximize
  - [ ] sidebar navigation
  - [ ] send message + stream
  - [ ] slash command autocomplete
  - [ ] mouse-wheel chat scroll
  - [ ] selection copy/paste
  - [ ] `/tools` status and policy commands
- [ ] **Convert `src/app/mod.rs` into a coordinator/facade shape** (module wiring first, no logic changes):
  - [ ] Review current module declarations
  - [ ] Ensure stable re-export surface for extracted modules
- [ ] **Convert `src/app/tui/mod.rs` into render orchestrator shape** (keep `draw()` orchestration, move internals later):
  - [ ] Review current module declarations
  - [ ] Identify orchestration vs. implementation boundaries
- [ ] **Avoid "empty scaffold first" churn**: for each new module, move at least one real function in the same change.
- [ ] **Keep function signatures stable** wherever possible to minimize call-site churn.
- [ ] **Preserve module-facing API** via explicit `pub use` re-exports in `app::mod` / `tui::mod` where callers currently depend on existing paths.

### Completion Criteria

- [ ] Build compiles with no behavior changes.
- [ ] No dead imports/warnings from scaffolding.
- [ ] Baseline quality gates recorded and passing.
- [ ] Manual smoke baseline documented.

### Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`

---

## Phase 1 - Split `src/app/tui/mod.rs` (Pure Move, No Logic Change)

**SCOPE**: Move ~2,000 lines of rendering logic from `tui/mod.rs` into focused modules.

**CRITICAL**: Current TUI module files (`palette.rs`, `sidebar.rs`, `chat.rs`, `input.rs`, `markdown.rs`) are **thin re-export wrappers** (8-19 lines each). This phase requires moving actual implementation, not just filling stubs.

**STRATEGY**: Split into sub-phases to manage risk and enable incremental verification.

---

### ✅ Function Names Verified

All Phase 1 function names have been verified via grep search on `src/app/tui/mod.rs`:
- Render functions: `render_chat_panel()`, `render_input_panel()`, `render_models_panel()`, `render_status_bar()`
- Utilities: `markdown_lines()`, `truncate_middle()`, `truncate_end()`, `scroll_window()`, `manual_reflow()`, `apply_selection_to_text()`
- Helpers: `render_code_line()`, `render_autocomplete_popup()`, `calculate_scroll_position()`
- Tests: 22 inline tests confirmed in `tui/mod.rs` (lines 1959-2194)

---

### Phase 1a - Chat & Input Rendering (~800 lines)

**Target Files**:
- [ ] `src/app/tui/chat.rs` (currently 10 lines of re-exports)
- [ ] `src/app/tui/input.rs` (currently 10 lines of re-exports)

**Functions to Move**:
- [ ] `render_chat_panel()` → `chat.rs`
- [ ] Chat footer context rendering helpers → `chat.rs`
- [ ] `render_input_panel()` → `input.rs`
- [ ] Input height calculation helpers → `input.rs`
- [ ] `estimate_input_visual_lines()` → `input.rs`
- [ ] `input_panel_height()` → `input.rs`

**Tests to Move**:
- [ ] `estimate_input_visual_lines` tests → `input.rs` (inline `#[cfg(test)]`)
- [ ] `input_panel_height` tests → `input.rs` (inline `#[cfg(test)]`)
- [ ] `input_cursor_*` tests → `input.rs` (inline `#[cfg(test)]`)

**Note**: Function names verified via grep search.

---

### Phase 1b - Sidebar & Status Bar Rendering (~600 lines)

**Target Files**:
- [ ] `src/app/tui/sidebar.rs` (currently 19 lines of re-exports)
- [ ] `src/app/tui/palette.rs` (currently 9 lines of re-exports)

**Functions to Move**:
- [ ] `render_models_panel()` → `sidebar.rs`
- [ ] Sidebar hint/marquee helpers → `sidebar.rs`
- [ ] Sidebar action hint helpers → `sidebar.rs`
- [ ] `render_status_bar()` → `sidebar.rs` (or create `status_bar.rs` if >150 lines)
- [ ] `ThemePalette` struct → `palette.rs`
- [ ] Theme/background mapping helpers → `palette.rs`
- [ ] `sidebar_marquee()` helper → `sidebar.rs`
- [ ] `sidebar_action_hint()` helper → `sidebar.rs`

**Tests to Move**:
- [ ] `sidebar_marquee` tests → `sidebar.rs` (inline `#[cfg(test)]`)
- [ ] `sidebar_action_hint` tests → `sidebar.rs` (inline `#[cfg(test)]`)

**Note**: Function names verified via grep search.

---

### Phase 1c - Markdown, Utilities & Layout (~600 lines)

**Target Files**:
- [ ] `src/app/tui/markdown.rs` (currently 8 lines of re-exports)
- [ ] Keep `src/app/tui/modals.rs` as-is (already 227 lines implementation)

**Functions to Move**:
- [ ] `markdown_lines()` → `markdown.rs`
- [ ] Syntax highlighting helpers → `markdown.rs`
- [ ] Code block rendering → `markdown.rs`
- [ ] `render_code_line()` → `markdown.rs`
- [ ] `render_autocomplete_popup()` → `input.rs` or `markdown.rs`
- [ ] `truncate_middle()`, `truncate_end()` → `markdown.rs` or new `utils.rs`
- [ ] `scroll_window()` → `markdown.rs` or new `utils.rs`
- [ ] `manual_reflow()` → `markdown.rs` or new `utils.rs`
- [ ] `apply_selection_to_text()` → `markdown.rs` or new `selection.rs`
- [ ] `calculate_scroll_position()` → `chat.rs` or `utils.rs`

**Tests to Move**:
- [ ] `truncate_middle`, `truncate_end` tests → owning module
- [ ] `scroll_window` tests → owning module
- [ ] `manual_reflow` tests → owning module
- [ ] `apply_selection_to_text` tests → owning module

**Note**: Function names verified via grep search.

---

### Optional New Modules (Only If Needed After Phases 1a-1c)

- [ ] `src/app/tui/layout.rs` - root/body/input area split calculations (only if not cohesive in existing files)
- [ ] `src/app/tui/selection.rs` - selection split/highlight utilities (only if `apply_selection_to_text` has complex logic)
- [ ] `src/app/tui/autocomplete.rs` - autocomplete popup rendering (only if not in `input.rs`)
- [ ] `src/app/tui/status_bar.rs` - top status line rendering (only if `render_status_bar` >150 lines)

---

### Ownership Lock (to avoid ping-pong refactors)

- [ ] `sidebar.rs` owns: sidebar panels, hints, marquee behaviors, status bar rendering (unless `status_bar.rs` created)
- [ ] `chat.rs` owns: chat panel rendering, footer context strip, message area layout
- [ ] `input.rs` owns: input panel rendering, cursor placement, input height calculations
- [ ] `markdown.rs` owns: syntax engine, markdown conversion, code block rendering, text utilities
- [ ] `palette.rs` owns: `ThemePalette` struct, theme/background mapping
- [ ] `modals.rs` owns: all modal rendering (already self-contained)
- [ ] Shared utilities must live in the module that uses them most; duplicates are not allowed.

---

### Rules

- [ ] **Move code as-is first** (copy/move), avoid semantic edits during movement.
- [ ] **Move tests WITH code** - extract inline `#[cfg(test)]` modules alongside functions.
- [ ] Keep `tui/mod.rs` limited to:
  - Module exports (`mod X;` declarations)
  - `draw()` orchestration function
  - Tiny glue code (frame setup, area calculations)
- [ ] Do not alter constants, truncation logic, or rendering order.
- [ ] Preserve function visibility (`pub(super)`, `pub(crate)`) to match existing call patterns.
- [ ] Update `mod.rs` re-exports to point to new module locations.
- [ ] **Do not create new TUI files** until the existing stub files (`chat.rs`, `input.rs`, `markdown.rs`, `palette.rs`, `sidebar.rs`) contain their real implementations.

---

### Completion Criteria

- [ ] `src/app/tui/mod.rs` is **< 300 lines** (down from 2,194 lines).
- [ ] Functionality remains bit-for-bit equivalent from user perspective.
- [ ] Existing TUI stub modules are no longer forwarding wrappers.
- [ ] All 22 inline tests from `tui/mod.rs` have been moved to owning modules.
- [ ] Each TUI submodule has its own `#[cfg(test)]` module with extracted tests.

---

### Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] **Manual UI smoke**:
  - [ ] Welcome screen
  - [ ] Chat pane rendering and scrolling
  - [ ] Footer telemetry display
  - [ ] Toast/help/modals layering
  - [ ] Theme switching (`/themes`) visual sanity
  - [ ] Input panel cursor behavior
  - [ ] Sidebar navigation and hints

---

## Phase 2 - Split `src/app/mod.rs` (State vs Behavior Boundaries)

**SCOPE**: Move ~1,300 lines from `app/mod.rs` into focused modules.

**CRITICAL**: `App` struct has **35+ fields** mixing unrelated concerns (god object pattern). This phase extracts cohesive behavior boundaries while keeping the struct intact (no field refactoring yet).

**STRATEGY**: Extract modules by responsibility, keeping public API stable for existing callers.

---

### ⚠️ Function Name Verification Note

Function names in this phase are **approximations based on code patterns**. The actual codebase uses:
- Direct field access for most state (fields are `pub`)
- Helper methods with names like `scroll_up()`, `set_toast()`, `push_user()`
- Some methods may need to be created during refactoring (e.g., `confirm_pending_workspace_delete()`)

**During implementation**:
1. Verify exact function names by searching `src/app/mod.rs`
2. Keep existing call sites working - don't break `runtime/`, `commands/`, `tui/`
3. It's OK to create new helper methods that don't exist yet (e.g., accessor methods for pub fields)

---

### Phase 2a - State Module (~450 lines)

**Target File**: `src/app/state.rs` (does not exist)

**Functions/Types to Move**:
- [ ] `App` struct definition (all 35+ fields stay together)
- [ ] `App::default()` implementation (if exists)
- [ ] `App::new()` constructor
- [ ] `should_quit()` method
- [ ] `request_quit()` method
- [ ] `quit_armed_until` field (pub field)
- [ ] `UiRuntimeState` struct
- [ ] `SessionContextSnapshot` struct
- [ ] `ThemePickerState` struct
- [ ] `AppUiStateView` struct
- [ ] `AppSessionStateView` struct
- [ ] `AppTelemetryStateView` struct
- [ ] `ui_state_view()` method (if exists - verify during implementation)
- [ ] `session_state_view()` method (if exists - verify during implementation)
- [ ] `telemetry_state_view()` method (if exists - verify during implementation)
- [ ] `snapshot_ui_state()` method (if exists - verify during implementation)
- [ ] `apply_ui_state()` method (if exists - verify during implementation)
- [ ] `snapshot_session_context()` method (if exists - verify during implementation)
- [ ] `apply_session_context()` method (if exists - verify during implementation)
- [ ] `restored_session_context` field
- [ ] State persistence helpers (load/save UI runtime state)

**Note**: Some view methods (`ui_state_view()`, etc.) may be named differently or not exist as explicit methods. The `App` struct fields are all public, so direct field access is common. During implementation, verify exact method names.

**Re-exports in `app/mod.rs`**:
- [ ] `pub use state::App;`
- [ ] `pub use state::{UiRuntimeState, SessionContextSnapshot, ThemePickerState};`
- [ ] `pub use state::{AppUiStateView, AppSessionStateView, AppTelemetryStateView};`

---

### Phase 2b - Messages & Conversation Module (~200 lines)

**Target File**: `src/app/messages_and_conversation.rs` (does not exist)

**Functions to Move**:
- [ ] `push_message()` helper (private, internal)
- [ ] `push_user()` method
- [ ] `push_assistant()` method
- [ ] `push_tool()` method
- [ ] `push_system()` method
- [ ] `push_tool_notice()` method (currently has `#[allow(dead_code)]`)
- [ ] `push_tool_result_notice()` method (currently has `#[allow(dead_code)]`)
- [ ] `now_label()` helper (private, timestamp formatting)
- [ ] `normalize_storage_timestamp()` helper (private)
- [ ] `load_conversation_into_app()` method (if exists - verify during implementation)
- [ ] `Message` struct (pub struct, may stay in `mod.rs` with re-export)

**Note**: Function names verified - they use `push_*` pattern as documented. Some tool notice methods are currently marked `#[allow(dead_code)]` and may be used by command handlers.

---

### Phase 2c - Input Editor Module (~250 lines)

**Target File**: `src/app/input_editor.rs` (does not exist)

**Note**: Named `input_editor.rs` to distinguish from existing `input.rs` (which contains `InputAction`/`InputBindings` keybinding resolution).

**Functions to Move**:
- [ ] `input_cursor_char_index` field getter/setter (pub field, keep direct access or add accessor)
- [ ] `input_cursor_preferred_column` field getter/setter (pub field)
- [ ] `move_input_cursor_left()` method
- [ ] `move_input_cursor_right()` method
- [ ] `move_input_cursor_up()` method
- [ ] `move_input_cursor_down()` method
- [ ] `clamp_input_cursor()` helper (private)
- [ ] `input_line_col_for_cursor()` helper (private)
- [ ] `input_char_index_for_line_col()` helper (private)
- [ ] `input_line_count()` helper (private)
- [ ] `input_char_len()` helper (private)
- [ ] `insert_input_char()` method
- [ ] `insert_input_string()` method
- [ ] `backspace_input_char()` method
- [ ] `insert_newline_at_cursor()` method
- [ ] `clear_input_editor_state()` method
- [ ] `set_input_text()` helper (private)

**Re-exports in `app/mod.rs`**:
- [ ] Keep public cursor methods on `App` struct (called from `runtime/key_actions.rs`)
- [ ] Internal calculation helpers move to module

**File-level doc comment** (to address naming confusion):
```rust
//! Input editor operations - cursor movement, text insertion/deletion.
//! 
//! This module handles the text editing logic for the chat input field.
//! Distinct from `app::input` which contains `InputAction`/`InputBindings` 
//! for keybinding resolution.
```

---

### Phase 2d - Chat Navigation Module (~200 lines)

**Target File**: `src/app/chat_navigation.rs` (does not exist)

**Functions to Move**:
- [ ] `scroll_up()` helper (private, internal scroll logic)
- [ ] `scroll_down()` helper (private, internal scroll logic)
- [ ] `scroll_up_preserve_selection()` helper (private)
- [ ] `scroll_down_preserve_selection()` helper (private)
- [ ] `update_chat_max_scroll_hint()` method
- [ ] `clamp_chat_scroll_to_hint()` method
- [ ] `chat_scroll` field (pub field, may add accessor methods)
- [ ] `clear_chat_selection()` method
- [ ] `chat_selection_anchor` field (pub field)
- [ ] `chat_selection_current` field (pub field)
- [ ] `is_selecting` field (pub field)
- [ ] `update_chat_area_metadata()` method
- [ ] `auto_scroll_direction` field (pub field)
- [ ] `auto_scroll_speed` field (pub field)

**Note**: Most scroll operations use helper methods (`scroll_up`, `scroll_down`) rather than `scroll_chat_up`/`scroll_chat_down`. The plan will use exact names during implementation.

---

### Phase 2e - Overlay & Toast Module (~150 lines)

**Target File**: `src/app/overlay_toast.rs` (does not exist)

**Functions/Fields to Move**:
- [ ] `set_toast()` method
- [ ] `toast` field (pub field on App)
- [ ] `dismiss_overlay()` method
- [ ] `toggle_help()` method
- [ ] `show_help` field (pub field)
- [ ] `pending_workspace_delete` field (pub field)
- [ ] `pending_session_delete` field (pub field)
- [ ] `pending_tool_confirmation` field (pub field)
- [ ] `pending_images` field (pub field)
- [ ] Toast expiration check logic (currently in `tick()` method around line 478-490)

**Note**: The codebase doesn't have explicit `show_toast()`, `confirm_pending_action()`, or `cancel_pending_action()` methods. Toasts are set via `set_toast()` and pending actions expire via timeout in the `tick()` method. Pending action confirmation/cancellation is handled in `runtime/key_actions.rs` or command handlers.

**Implementation note**: May need to create helper methods for:
- `toast_expired(&self) -> bool` (new helper)
- `is_overlay_visible(&self) -> bool` (new helper - checks help, toast, pending dialogs)
- `confirm_pending_workspace_delete()` (new helper, called from commands/runtime)
- `cancel_pending_workspace_delete()` (new helper)
- Similar for session delete and tool confirmation

---

### Phase 2f - Theme Picker Module (~100 lines)

**Target File**: `src/app/theme_picker.rs` (does not exist)

**Functions/Fields to Move**:
- [ ] `theme_picker` field (pub field)
- [ ] `theme_picker_entries()` method (returns `&'static [UiTheme]`)
- [ ] `open_theme_picker()` helper (private)
- [ ] `theme_picker_next()` method
- [ ] `theme_picker_prev()` method
- [ ] `theme_picker_confirm()` method
- [ ] `theme_picker_cancel()` method
- [ ] `UiTheme` enum (may stay in `mod.rs` or move - used widely)
- [ ] `ThemePickerState` struct

**Note**: `UiTheme` enum is used across many modules. Consider keeping it in `mod.rs` with `pub use` re-exports, or create a separate `theme.rs` module if it grows.

---

### Phase 2g - Helper Modules (Optional, Only If Needed)

**Target Files** (only if `app/mod.rs` still > 400 lines after 2a-2f):
- [ ] `src/app/workspace_session_help.rs` - workspace/session help text generators
- [ ] `src/app/model_context_ops.rs` - model-context refresh + command model-context change predicates

---

### Rules

- [ ] **Keep public API stable** for existing callers (`commands`, `runtime`, `chat_pipeline`, `tui`).
- [ ] **Keep stable re-export surface** in `src/app/mod.rs` for moved public/crate-visible items to avoid broad call-site churn.
- [ ] **No field rename or persistence key rename** unless strictly necessary.
- [ ] **Keep storage behavior and context restoration semantics** unchanged.
- [ ] **Avoid source-module names that mirror `src/app/mod_tests/*` filenames** when a clearer name exists (reduce contributor confusion).
- [ ] **Add file-level ownership comment** at the top of broad modules (especially `input_editor.rs`) describing exactly what belongs there and what does not.
- [ ] **Keep methods on `App` struct** when they're called from outside the `app` module - use internal module functions for implementation.

---

### Completion Criteria

- [ ] `src/app/mod.rs` is **< 400 lines** (down from 1,732 lines).
- [ ] `src/app/mod.rs` acts as module coordinator + minimal impl glue.
- [ ] Extracted modules have coherent, single-purpose responsibilities.
- [ ] `src/app/mod.rs` no longer mixes unrelated concerns (state, input math, sidebar, persistence IO, command routing) in one file.
- [ ] All re-exports are in place for external callers.
- [ ] No public API changes for `commands`, `runtime`, `chat_pipeline`, `tui` modules.

---

### Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] **Focused behavior checks**:
  - [ ] input multiline cursor movement parity
  - [ ] sidebar selection/collapse parity
  - [ ] session/workspace context restoration on restart
  - [ ] selection clear/scroll interactions parity
  - [ ] toast dismissal and expiration behavior
  - [ ] theme picker navigation and selection
  - [ ] message push behavior (user/assistant/tool/system)

---

## Phase 3 - Regression Test Reinforcement (Only Where Risk Justifies)

### Add/Adjust Tests

- [ ] Add focused tests for extracted boundaries if coverage drops or fragile behavior is detected:
  - [ ] **Input cursor tests**: line/column transitions, multiline behavior, preferred column persistence
  - [ ] **Sidebar tests**: entry ordering, index clamp, marquee timing, action hint formatting
  - [ ] **Scroll tests**: clamp behavior, preserve-selection during scroll, bounds checking
  - [ ] **UI-state tests**: snapshot/restore consistency, view projection accuracy
  - [ ] **Footer context tests**: token formatting fallback, context used/max calculation
  - [ ] **Toast tests**: expiration timing, dismissal behavior, overlay stacking
  - [ ] **Theme picker tests**: cycling behavior, confirm/cancel semantics
  - [ ] **Message tests**: push ordering, timestamp normalization, tool notice formatting

- [ ] **Move TUI tests from inline to module tests**:
  - [ ] Ensure all 22 tests from `tui/mod.rs` are in owning module `#[cfg(test)]` blocks
  - [ ] Verify test coverage is unchanged after movement

- [ ] Keep tests deterministic (no network dependency).

- [ ] Keep test placement aligned with ownership:
  - [ ] Move TUI helper tests out of `tui/mod.rs` into their owning modules when helpers move
  - [ ] Keep app-state tests in `src/app/mod_tests/*` unless ownership justifies a local module test
  - [ ] Consider new test file `tests/tui_rendering.rs` for high-level TUI integration tests

---

### Completion Criteria

- [ ] No regressions detected in high-risk interaction areas.
- [ ] New tests cover newly isolated logic seams.
- [ ] All moved tests pass in their new locations.
- [ ] Test count is equal to or higher than baseline (no lost tests).

---

### Verification

- [ ] `cargo test --all-targets --all-features`
- [ ] Re-run full gate:
  - [ ] `cargo fmt --all -- --check`
  - [ ] `cargo clippy --all-targets --all-features -- -D warnings`
  - [ ] `cargo test --all-targets --all-features`
- [ ] Compare test count with baseline (should be equal or higher)

---

## Phase 4 - Final Hardening & Documentation

### Tasks

- [ ] **Remove temporary glue/compat helpers** that are no longer needed:
  - [ ] Check for re-export wrappers that are no longer needed
  - [ ] Remove any `#[allow(dead_code)]` that were added temporarily
  - [ ] Clean up any `#[allow(clippy::too_many_arguments)]` that can now be resolved

- [ ] **Ensure module boundaries are documented** with brief file-level comments where useful:
  - [ ] Add `//!` module doc comments to each new module
  - [ ] Document ownership boundaries (what belongs here, what doesn't)
  - [ ] Add examples for complex public APIs

- [ ] **Update developer docs** if module map changed materially:
  - [ ] Update `README.md` if public API changed (unlikely per constraints)
  - [ ] Update `AGENTS.md` module structure section if paths changed
  - [ ] Update this plan document with actual final LOC counts

- [ ] **Update changelog entry** for refactor release version when implementation is complete:
  - [ ] Add entry to `CHANGELOG.md` documenting the refactoring
  - [ ] Note any new public module paths (for plugin/extension authors)

---

### Completion Criteria

- [ ] No dead code or unused module wrappers remain.
- [ ] Refactor is understandable for future contributors.
- [ ] All new modules have doc comments explaining their purpose.
- [ ] Developer documentation reflects the new module structure.

---

### Verification

- [ ] Full quality gate passes:
  - [ ] `cargo fmt --all -- --check`
  - [ ] `cargo clippy --all-targets --all-features -- -D warnings`
  - [ ] `cargo test --all-targets --all-features`
- [ ] Debug build check:
  - [ ] `cargo build`
- [ ] Record final LOC counts:
  - [ ] `src/app/mod.rs`: ___ lines (target: < 400)
  - [ ] `src/app/tui/mod.rs`: ___ lines (target: < 300)
  - [ ] Total reduction: ___% (target: ~85%)

---

## Definition of Done (Whole Plan)

- [x] Baseline LOC recorded (3,926 lines combined)
- [x] Architecture insights documented
- [x] Extraction candidates identified with line estimates
- [x] Test structure mapped (22 inline TUI tests + 2 in modals.rs)
- [ ] `src/app/mod.rs` and `src/app/tui/mod.rs` are no longer monolithic coordinators of all logic.
- [ ] Extracted modules are responsibility-focused and navigable.
- [ ] UI and behavior are unchanged from baseline (validated by tests + manual smoke).
- [ ] Full checks pass (`fmt`, `clippy -D warnings`, `test`, `build`).
- [ ] Documentation/changelog updated if needed.
- [ ] Final LOC targets met:
  - [ ] `src/app/mod.rs`: < 400 lines (from 1,732)
  - [ ] `src/app/tui/mod.rs`: < 300 lines (from 2,194)

---

## Rollback Strategy

- [ ] Execute phase-by-phase with small, isolated file-change sets.
- [ ] If regression appears, undo only the offending phase changes (targeted file-level rollback).
- [ ] Keep move-only changes separate from cleanup changes in implementation order to simplify bisect/debug.
- [ ] **Git hygiene**: Consider creating a branch per phase for easier rollback:
  - `refactor/phase-0-baseline`
  - `refactor/phase-1a-tui-chat-input`
  - `refactor/phase-1b-tui-sidebar-palette`
  - `refactor/phase-1c-tui-markdown-utils`
  - `refactor/phase-2a-app-state`
  - `refactor/phase-2b-messages`
  - `refactor/phase-2c-input-editor`
  - `refactor/phase-2d-chat-navigation`
  - `refactor/phase-2e-overlay-toast`
  - `refactor/phase-2f-theme-picker`

---

## Estimated Effort (From Codebase Analysis)

**Total estimated time**: 40-60 hours for complete extraction with tests.

| Phase | Estimated Hours | Risk Level |
|-------|-----------------|------------|
| Phase 0 - Baseline & Safety | 2-4 hours | Low |
| Phase 1a - Chat & Input | 6-8 hours | Medium |
| Phase 1b - Sidebar & Palette | 6-8 hours | Medium |
| Phase 1c - Markdown & Utils | 6-8 hours | Medium |
| Phase 2a - State Module | 4-6 hours | Medium |
| Phase 2b - Messages | 3-4 hours | Low |
| Phase 2c - Input Editor | 4-6 hours | Medium |
| Phase 2d - Chat Navigation | 3-4 hours | Low |
| Phase 2e - Overlay & Toast | 3-4 hours | Low |
| Phase 2f - Theme Picker | 2-3 hours | Low |
| Phase 3 - Test Reinforcement | 4-6 hours | Medium |
| Phase 4 - Hardening & Docs | 2-4 hours | Low |

**Critical Path**: Phases 1a-1c (TUI extraction) must complete before Phase 2 (app extraction) to validate the module boundary pattern.

---

## Appendix: Verified Function Names (Phase 2 Reference)

**Verified via grep search on 2026-04-01**. Use these exact names during implementation.

### Input Editor Functions (Phase 2c)
- `move_input_cursor_left()`, `move_input_cursor_right()`
- `move_input_cursor_up()`, `move_input_cursor_down()`
- `insert_input_char()`, `insert_input_string()`
- `backspace_input_char()`, `insert_newline_at_cursor()`
- `clamp_input_cursor()` (private)
- `input_line_col_for_cursor()` (private)
- `input_char_index_for_line_col()` (private)
- `input_line_count()` (private)
- `input_char_len()` (private)
- `clear_input_editor_state()`
- `set_input_text()` (private)
- Fields: `input_cursor_char_index`, `input_cursor_preferred_column`

### Chat Navigation Functions (Phase 2d)
- `scroll_up()`, `scroll_down()` (private helpers)
- `scroll_up_preserve_selection()`, `scroll_down_preserve_selection()` (private)
- `update_chat_max_scroll_hint()`
- `clamp_chat_scroll_to_hint()`
- `clear_chat_selection()`
- `update_chat_area_metadata()`
- Fields: `chat_scroll`, `chat_max_scroll_hint`, `chat_selection_anchor`, `chat_selection_current`, `is_selecting`, `auto_scroll_direction`, `auto_scroll_speed`

### Overlay & Toast Functions (Phase 2e)
- `set_toast()`
- `dismiss_overlay()`
- `toggle_help()`
- Fields: `show_help`, `toast`, `pending_workspace_delete`, `pending_session_delete`, `pending_tool_confirmation`, `pending_images`
- **Note**: Toast expiration checked in `tick()` method (line 478-490), not a separate method
- **Note**: No explicit `confirm_pending_*()` or `cancel_pending_*()` methods exist - may need to create them

### Theme Picker Functions (Phase 2f)
- `theme_picker_entries()`
- `open_theme_picker()` (private)
- `theme_picker_next()`, `theme_picker_prev()`
- `theme_picker_confirm()`, `theme_picker_cancel()`
- Fields: `theme_picker`, `theme`
- Types: `UiTheme` enum, `ThemePickerState` struct

### Messages Functions (Phase 2b)
- `push_message()` (private)
- `push_user()`, `push_assistant()`, `push_tool()`, `push_system()`
- `push_tool_notice()`, `push_tool_result_notice()` (both have `#[allow(dead_code)]`)
- `now_label()` (private, timestamp formatting)
- `normalize_storage_timestamp()` (private)
- Type: `Message` struct

### State Functions (Phase 2a)
- `App::new()`, `App::should_quit()`, `App::request_quit()`
- Fields: All 35+ public fields including `quit_armed_until`, `restored_session_context`
- Types: `UiRuntimeState`, `SessionContextSnapshot`, `ThemePickerState`, `AppUiStateView`, `AppSessionStateView`, `AppTelemetryStateView`
- **Note**: View methods (`ui_state_view()`, etc.) may need verification during implementation

---

## Appendix: Current Module Map (Pre-Refactor)

```
src/app/
├── mod.rs                 # 1,732 lines - MAIN MONOLITH
├── input.rs               # 321 lines - InputAction, bindings (keep as-is)
├── chat_pipeline.rs       # 631 lines - Send/stream/persist (keep as-is)
├── context.rs             # 39 lines - AppContext (keep as-is)
├── mod_tests/             # 11 test files (well-organized)
├── autocomplete/          # Command suggestion (keep as-is)
├── commands/              # Slash command handlers (keep as-is)
├── domain/                # Workspace/session helpers (keep as-is)
├── platform/              # Windows console hardening (~280 lines, well-factored)
├── runtime/               # Event loop, key actions, overlays (keep as-is)
├── tools/                 # Tool registry, executor (keep as-is)
└── tui/
    ├── mod.rs             # 2,194 lines - MAIN MONOLITH
    ├── palette.rs         # 9 lines - Re-exports only (NEEDS IMPLEMENTATION)
    ├── sidebar.rs         # 19 lines - Re-exports only (NEEDS IMPLEMENTATION)
    ├── chat.rs            # 10 lines - Re-exports only (NEEDS IMPLEMENTATION)
    ├── input.rs           # 10 lines - Re-exports only (NEEDS IMPLEMENTATION)
    ├── markdown.rs        # 8 lines - Re-exports only (NEEDS IMPLEMENTATION)
    └── modals.rs          # 227 lines - Full implementation (KEEP AS-IS)
```

---

## Appendix: Post-Refactor Module Map (Target)

```
src/app/
├── mod.rs                 # < 400 lines - Coordinator/facade only
├── state.rs               # NEW - App struct, state types, snapshot/restore
├── messages_and_conversation.rs  # NEW - Message push, conversation loading
├── input_editor.rs        # NEW - Cursor management, text editing
├── chat_navigation.rs     # NEW - Scroll, selection, auto-scroll
├── overlay_toast.rs       # NEW - Toast, confirmations, help overlay
├── theme_picker.rs        # NEW - Theme picker state machine
├── input.rs               # 321 lines - InputAction, bindings (unchanged)
├── chat_pipeline.rs       # 631 lines - Send/stream/persist (unchanged)
├── context.rs             # 39 lines - AppContext (unchanged)
├── mod_tests/             # Test files (may add tui_rendering.rs)
├── autocomplete/          # Command suggestion (unchanged)
├── commands/              # Slash command handlers (unchanged)
├── domain/                # Workspace/session helpers (unchanged)
├── platform/              # Windows console hardening (unchanged)
├── runtime/               # Event loop, key actions, overlays (unchanged)
├── tools/                 # Tool registry, executor (unchanged)
└── tui/
    ├── mod.rs             # < 300 lines - Render orchestration only
    ├── palette.rs         # ~150 lines - ThemePalette, theme mapping
    ├── sidebar.rs         # ~400 lines - Models panel, status bar, hints
    ├── chat.rs            # ~400 lines - Chat panel, footer context
    ├── input.rs           # ~300 lines - Input panel, cursor, height calc
    ├── markdown.rs        # ~400 lines - Syntax, markdown, utilities
    └── modals.rs          # 227 lines - Modal rendering (unchanged)
```
