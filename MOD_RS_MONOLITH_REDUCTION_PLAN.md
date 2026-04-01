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

## Baseline Snapshot (Before Refactor)

- [ ] Record baseline LOC:
  - `src/app/mod.rs`
  - `src/app/tui/mod.rs`
- [ ] Record baseline quality gate status:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-targets --all-features`
- [ ] Record a short manual smoke baseline:
  - app launch/maximize
  - sidebar navigation
  - send message + stream
  - slash command autocomplete
  - mouse-wheel chat scroll
  - selection copy/paste
  - `/tools` status and policy commands

---

## Phase 0 - Safety Harness & Refactor Skeleton

### Tasks

- [ ] Convert `src/app/mod.rs` into a coordinator/facade shape (module wiring first, no logic changes).
- [ ] Convert `src/app/tui/mod.rs` into render orchestrator shape (keep `draw()` orchestration, move internals later).
- [ ] Avoid "empty scaffold first" churn: for each new module, move at least one real function in the same change.
- [ ] Keep function signatures stable wherever possible to minimize call-site churn.
- [ ] Preserve module-facing API via explicit `pub use` re-exports in `app::mod` / `tui::mod` where callers currently depend on existing paths.

### Completion Criteria

- [ ] Build compiles with no behavior changes.
- [ ] No dead imports/warnings from scaffolding.

### Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`

---

## Phase 1 - Split `src/app/tui/mod.rs` (Pure Move, No Logic Change)

### Target Modules

- [ ] `src/app/tui/palette.rs`
  - `ThemePalette`
  - theme/background mapping helpers
- [ ] `src/app/tui/layout.rs`
  - root/body/input area split calculations
- [ ] `src/app/tui/status_bar.rs`
  - top status line rendering
- [ ] `src/app/tui/sidebar.rs`
  - models/workspaces panels + hints
- [ ] `src/app/tui/chat.rs`
  - chat rendering + footer context strip
- [ ] `src/app/tui/input.rs`
  - input panel, cursor placement, input-height helpers
- [ ] `src/app/tui/autocomplete.rs`
  - autocomplete popup rendering
- [ ] `src/app/tui/markdown.rs`
  - syntax engine + markdown/code conversion helpers
- [ ] `src/app/tui/selection.rs`
  - selection split/highlight utilities

### Ownership Lock (to avoid ping-pong refactors)

- [ ] `status_bar.rs` owns only top-line status composition/render (`render_status_bar` and its direct formatting helpers).
- [ ] `sidebar.rs` owns only sidebar panels/hints/marquee behaviors (`render_models_panel`, sidebar hint/marquee helpers).
- [ ] Shared utilities must live in the module that uses them most; duplicates are not allowed.

### Rules

- [ ] Move code as-is first (copy/move), avoid semantic edits.
- [ ] Keep `tui/mod.rs` limited to module exports + `draw()` orchestration + tiny glue.
- [ ] Do not alter constants, truncation logic, or rendering order.
- [ ] Move colocated tests with moved helpers (do not leave `tui/mod.rs` tests bound to now-extracted private functions).

### Completion Criteria

- [ ] `src/app/tui/mod.rs` is substantially reduced and readable as a facade.
- [ ] Functionality remains bit-for-bit equivalent from user perspective.

### Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] Manual UI smoke:
  - [ ] Welcome screen
  - [ ] Chat pane rendering and scrolling
  - [ ] Footer telemetry display
  - [ ] Toast/help/modals layering
  - [ ] Theme switching (`/themes`) visual sanity

---

## Phase 2 - Split `src/app/mod.rs` (State vs Behavior Boundaries)

### Target Modules

- [ ] `src/app/state.rs`
  - `App` struct fields and constructor/default state
- [ ] `src/app/ui_runtime_state.rs`
  - snapshot/apply/restore runtime UI + context telemetry
- [ ] `src/app/input_editor.rs`
  - cursor math, line/col conversion, input text operations
- [ ] `src/app/chat_scroll_selection.rs`
  - scroll math, preserve-selection scrolling, auto-scroll tick behavior
- [ ] `src/app/sidebar_state.rs`
  - sidebar entry building, index clamping, collapse/expand helpers
- [ ] `src/app/message_state.rs`
  - message mapping/push helpers and timestamp normalization
- [ ] `src/app/autocomplete_state.rs`
  - suggestions and selection-cycle helpers
- [ ] `src/app/ui_state_io.rs`
  - ui-state path/load/save/apply-startup-precedence helpers
- [ ] `src/app/conversation_loader.rs`
  - conversation hydration (`load_conversation_into_app`) and message restoration glue
- [ ] `src/app/command_dispatch.rs`
  - `handle_command` routing integration and unknown-command fallback behavior
- [ ] `src/app/help_text.rs`
  - workspace/session help text generators
- [ ] `src/app/model_context.rs`
  - model-context refresh + command model-context change predicates

### Rules

- [ ] Keep public API stable for existing callers (`commands`, `runtime`, `chat_pipeline`, `tui`).
- [ ] Keep stable re-export surface in `src/app/mod.rs` for moved public/crate-visible items to avoid broad call-site churn.
- [ ] No field rename or persistence key rename unless strictly necessary.
- [ ] Keep storage behavior and context restoration semantics unchanged.

### Completion Criteria

- [ ] `src/app/mod.rs` acts as module coordinator + minimal impl glue.
- [ ] Extracted modules have coherent, single-purpose responsibilities.

### Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] Focused behavior checks:
  - [ ] input multiline cursor movement parity
  - [ ] sidebar selection/collapse parity
  - [ ] session/workspace context restoration on restart
  - [ ] selection clear/scroll interactions parity

---

## Phase 3 - Regression Test Reinforcement (Only Where Risk Justifies)

### Add/Adjust Tests

- [ ] Add focused tests for extracted boundaries if coverage drops or fragile behavior is detected:
  - [ ] input cursor line/column transitions
  - [ ] sidebar entry ordering and index clamp
  - [ ] scroll clamp and preserve-selection behavior
  - [ ] runtime UI-state snapshot/restore consistency
  - [ ] footer context token formatting fallback behavior
- [ ] Keep tests deterministic (no network dependency).
- [ ] Keep test placement aligned with ownership:
  - [ ] move TUI helper tests out of `tui/mod.rs` into their owning modules when helpers move
  - [ ] keep app-state tests in `src/app/mod_tests/*` unless ownership justifies a local module test

### Completion Criteria

- [ ] No regressions detected in high-risk interaction areas.
- [ ] New tests cover newly isolated logic seams.

### Verification

- [ ] `cargo test --all-targets --all-features`
- [ ] Re-run full gate:
  - [ ] `cargo fmt --all -- --check`
  - [ ] `cargo clippy --all-targets --all-features -- -D warnings`
  - [ ] `cargo test --all-targets --all-features`

---

## Phase 4 - Final Hardening & Documentation

### Tasks

- [ ] Remove temporary glue/compat helpers that are no longer needed.
- [ ] Ensure module boundaries are documented with brief file-level comments where useful.
- [ ] Update developer docs if module map changed materially (`README.md`, `AGENTS.md` only if needed).
- [ ] Update changelog entry for refactor release version when implementation is complete.

### Completion Criteria

- [ ] No dead code or unused module wrappers remain.
- [ ] Refactor is understandable for future contributors.

### Verification

- [ ] Full quality gate passes:
  - [ ] `cargo fmt --all -- --check`
  - [ ] `cargo clippy --all-targets --all-features -- -D warnings`
  - [ ] `cargo test --all-targets --all-features`
- [ ] Debug build check:
  - [ ] `cargo build`

---

## Definition of Done (Whole Plan)

- [ ] `src/app/mod.rs` and `src/app/tui/mod.rs` are no longer monolithic coordinators of all logic.
- [ ] Extracted modules are responsibility-focused and navigable.
- [ ] UI and behavior are unchanged from baseline (validated by tests + manual smoke).
- [ ] Full checks pass (`fmt`, `clippy -D warnings`, `test`, `build`).
- [ ] Documentation/changelog updated if needed.

---

## Rollback Strategy

- [ ] Execute phase-by-phase with small, isolated file-change sets.
- [ ] If regression appears, undo only the offending phase changes (targeted file-level rollback).
- [ ] Keep move-only changes separate from cleanup changes in implementation order to simplify bisect/debug.
