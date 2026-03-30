# REFACTOR_PLAN.md

## Execution Status
- Completed: 2026-03-30
- Outcome: All planned refactor phases (0 -> 7) implemented and validated through full quality gates.
- Baseline evidence: `docs/REFACTOR_BASELINE.md`

## Objective
Refactor the codebase for maintainability and growth without changing app behavior, UX, visual design, command semantics, or persistence behavior.

This plan is implementation-ready and intentionally detailed so it can be executed phase-by-phase with low risk.

## Scope
- In scope:
  - Internal code structure, module boundaries, naming, function decomposition, and test organization.
  - Reducing complexity in `src/app/mod.rs` and `src/app/tui/mod.rs`.
  - Adding/refining tests to lock behavior before and after refactors.
- Out of scope:
  - New features.
  - UX/design/layout changes.
  - Command behavior changes.
  - DB schema changes unless required for strict compatibility (should be avoided).

## Hard Constraints
- [x] No user-visible behavior changes.
- [x] No visual/layout changes in TUI.
- [x] No command syntax/semantics changes.
- [x] No persistence format changes (except backward-compatible no-op metadata when unavoidable).
- [x] Backward compatibility for existing `ui_state.json` files must be preserved (missing/older fields deserialize safely).
- [x] Startup precedence must remain stable: CLI overrides > persisted runtime state > config defaults.
- [x] Keep all existing tests passing throughout.
- [x] Every refactor step must be covered by tests/checks before moving to next phase.

## Baseline Risk Areas (Current)
- `src/app/mod.rs` is very large (~5k lines), with high coupling between:
  - runtime loop,
  - command handling,
  - workspace/session transitions,
  - streaming/persistence path,
  - UI runtime state persistence.
- `src/app/tui/mod.rs` is large (~1.9k lines), with repeated modal rendering and mixed responsibilities.
- Large functions increase regression risk:
  - `handle_command` (~900 lines),
  - `run` (~500 lines),
  - `autocomplete_suggestions` (~300 lines).

## Refactor Strategy
Use "strangler" refactoring:
1. lock behavior with tests,
2. extract one concern at a time behind thin adapters,
3. keep call sites stable,
4. run full checks after each slice,
5. only then continue.

## Flake Prevention Rules
- [x] No feature work while refactor phases are active.
- [x] Do not mix unrelated refactors in the same commit.
- [x] Avoid changing more than one high-risk area per phase (runtime, commands, tui, persistence) at once.
- [x] Keep module extraction commits mechanically simple first, behavior-preserving cleanup second.
- [x] If a phase exceeds manageable diff size, split phase into sub-phases before continuing.

---

## Phase 0 - Baseline Lock and Refactor Harness
Goal: Create a safety net and baseline so later changes can be validated quickly.

### Tasks
- [x] Create a baseline checklist document section in PR notes (runtime behavior matrix).
- [x] Run and record current full gate results (`./scripts/check.ps1`).
- [x] Record current startup precedence behavior (CLI model override vs persisted model vs config default).
- [x] Identify and list critical user journeys to validate after each phase:
  - [x] start app and load workspace/session,
  - [x] send chat message and stream response,
  - [x] `/workspace` create/switch/rename/delete,
  - [x] `/session` create/switch/rename/delete with modal,
  - [x] `/models`, `/model`, `/url`, `/themes`, `/export`, `/checkpoint`, `/attach`.
- [x] Add any missing high-value regression tests before moving code (only where gaps exist).
  - [x] Add/confirm tests for runtime-state backward compatibility (`ui_state.json` older shape).
  - [x] Add/confirm tests for startup model precedence behavior.
  - [x] Add/confirm restart telemetry restore (`ctx:used/max`) behavior.

### Required Checks
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test --all-targets --all-features`
- [x] `cargo build`

### Exit Criteria
- [x] Baseline is green and documented.
- [x] High-risk behaviors have explicit tests.

---

## Phase 1 - Command System Decomposition
Goal: Split command parsing/dispatch/handlers out of `src/app/mod.rs`.

### Target Structure
- `src/app/commands/mod.rs`
- `src/app/commands/parser.rs`
- `src/app/commands/dispatch.rs`
- `src/app/commands/handlers/`:
  - `workspace.rs`
  - `session.rs`
  - `models.rs`
  - `utility.rs` (search/export/checkpoint/attach/url/model/themes/commands)

### Tasks
- [x] Introduce command AST/enums with strict parser unit tests.
- [x] Move parse logic out of `handle_command`.
- [x] Introduce route-based command dispatch (`CommandRoute`) and wire `handle_command` to dispatcher.
- [x] Extract utility command family to `commands/handlers/utility.rs` (`/commands`, `/models`, `/attach`, `/clear-attachments`, `/model`, `/url`).
- [x] Extract misc command family to `commands/handlers/misc.rs` (`/search`, `/export`, `/checkpoint`, `/workspaces` alias).
- [x] Move each command family into dedicated handler module.
- [x] Keep `handle_command` as thin adapter calling dispatcher.
- [x] Keep existing command strings/help/toasts unchanged.
- [x] Keep existing module/public function signatures stable where feasible to reduce integration churn.

### Tests
- [x] Existing command tests continue to pass unchanged.
- [x] Add parser-specific unit tests for edge cases:
  - [x] quoted args,
  - [x] `--to`, `--path`,
  - [x] unknown/empty command fallback.
- [x] Add snapshot-style assertions for command help output where brittle.

### Required Checks
- [x] full `./scripts/check.ps1`

### Exit Criteria
- [x] `handle_command` reduced to orchestration only.
- [x] command-specific logic lives in `commands/handlers/*`.

---

## Phase 2 - Runtime Loop Decomposition
Goal: Split the giant `run` function into focused runtime handlers without behavior drift.

### Target Structure
- `src/app/runtime/mod.rs`
- `src/app/runtime/event_loop.rs`
- `src/app/runtime/key_actions.rs`
- `src/app/runtime/overlays.rs`
- `src/app/runtime/startup.rs`
- `src/app/runtime/shutdown.rs`

### Tasks
- [x] Extract startup initialization sequence (config/session/model bootstrap).
- [x] Extract event polling + key event dispatch.
- [x] Extract overlay handling blocks:
  - [x] help modal,
  - [x] workspace delete modal,
  - [x] session delete modal,
  - [x] theme picker modal.
- [x] Extract chat-input action handler (`Insert`, `Backspace`, arrows, newline/send).
- [x] Preserve exact keybinding behavior and focus rules.
- [x] Preserve startup precedence behavior explicitly (CLI > persisted runtime state > config defaults).

### Tests
- [x] Existing keybinding and modal tests pass.
- [x] Add targeted tests for action routing precedence:
  - [x] overlay active vs normal input,
  - [x] sidebar-focused slash command behavior,
  - [x] command suggestion navigation precedence.

### Required Checks
- [x] full `./scripts/check.ps1`

### Exit Criteria
- [x] `run` becomes high-level orchestration only.
- [x] event/action behavior unchanged in tests.

---

## Phase 3 - Chat Send/Stream Pipeline Extraction
Goal: Isolate the send+stream+persistence pipeline from UI event handling.

### Target Structure
- `src/app/chat_pipeline.rs`
- (optional) `src/app/chat_pipeline/types.rs`

### Tasks
- [x] Extract prompt send flow:
  - [x] user message persistence,
  - [x] request preparation,
  - [x] streaming callbacks,
  - [x] assistant/tool persistence,
  - [x] telemetry updates (`prompt_eval_count`, `eval_count`),
  - [x] error path handling.
- [x] Keep rendering hooks/callback contracts stable.
- [x] Keep context-window refresh triggers exactly as now.
- [x] Keep persistence ordering unchanged (user message save -> assistant/tool save -> telemetry/state snapshot updates).

### Tests
- [x] Existing streaming/network tests pass unchanged.
- [x] Add focused unit tests for pipeline state transitions:
  - [x] success with content,
  - [x] success with empty content,
  - [x] stream failure,
  - [x] tool-call capture/persist.

### Required Checks
- [x] full `./scripts/check.ps1`

### Exit Criteria
- [x] Streaming logic no longer embedded in runtime event switch.

---

## Phase 4 - Session/Workspace Domain Operations Cleanup
Goal: Centralize workspace/session transition and delete-confirm behavior.

### Target Structure
- `src/app/domain/workspace_session.rs`
- `src/app/domain/context_state.rs`

### Tasks
- [x] Extract repeated workspace/session state sync blocks into reusable helpers.
- [x] Consolidate delete confirmation executors and selection resolution helpers.
- [x] Remove duplicated "refresh + sync + focus" call chains via common utility.
- [x] Preserve existing toast/status/system message texts.

### Tests
- [x] Existing workspace/session behavior tests pass.
- [x] Add tests for selection fallback matrix:
  - [x] selected session/workspace,
  - [x] active fallback,
  - [x] not-found messaging.

### Required Checks
- [x] full `./scripts/check.ps1`

### Exit Criteria
- [x] No duplicated transition code blocks in command handlers.

---

## Phase 5 - Autocomplete Engine Refactor
Goal: Replace monolithic autocomplete function with command-scoped providers.

### Target Structure
- `src/app/autocomplete/mod.rs`
- `src/app/autocomplete/core.rs`
- `src/app/autocomplete/providers/`:
  - `workspace.rs`
  - `session.rs`
  - `models.rs`
  - `misc.rs`

### Tasks
- [x] Create a provider interface for suggestions.
- [x] Move workspace/session/model logic into dedicated providers.
- [x] Keep ordering, filtering, casing, and exact outputs stable.
- [x] Keep `/session delete` single-command behavior intact.

### Tests
- [x] Existing autocomplete tests pass unchanged.
- [x] Add provider-level tests for complex branches.

### Required Checks
- [x] full `./scripts/check.ps1`

### Exit Criteria
- [x] `autocomplete_suggestions` becomes thin coordinator.

---

## Phase 6 - TUI Rendering Modularization (No Visual Change)
Goal: Split `src/app/tui/mod.rs` by rendering concern while preserving output.

### Target Structure
- `src/app/tui/mod.rs` (entry)
- `src/app/tui/palette.rs`
- `src/app/tui/sidebar.rs`
- `src/app/tui/chat.rs`
- `src/app/tui/input.rs`
- `src/app/tui/modals.rs`
- `src/app/tui/markdown.rs`

### Tasks
- [x] Extract modal rendering into reusable function (`render_confirm_delete_modal`).
- [x] Extract sidebar/chat/input/status rendering modules.
- [x] Keep all layout constants and calculations unchanged.
- [x] Keep color/theme semantics unchanged.

### Tests
- [x] Existing tui tests pass unchanged.
- [x] Add one or two extra deterministic tests for modal text/layout guards.

### Required Checks
- [x] full `./scripts/check.ps1`

### Exit Criteria
- [x] `tui/mod.rs` primarily composes submodule renders.

---

## Phase 7 - State Struct Cleanup and Naming Pass
Goal: Improve readability and ownership boundaries in app state.

### Tasks
- [x] Group fields in `App` by concern using nested structs (internal only), e.g.:
  - [x] `AppUiState`,
  - [x] `AppSessionState`,
  - [x] `AppTelemetryState`.
- [x] Rename ambiguous helpers to intent-revealing names.
- [x] Remove dead/duplicated helper functions discovered during previous phases.
- [x] Keep serde compatibility for persisted UI runtime state.
- [x] Keep migration-free compatibility with prior `ui_state.json` files from older releases.

### Tests
- [x] Existing tests pass.
- [x] Add/adjust serialization compatibility tests for `ui_state.json`.

### Required Checks
- [x] full `./scripts/check.ps1`

### Exit Criteria
- [x] `App` is easier to navigate and reason about.
- [x] No behavior changes detected.

---

## Cross-Phase Quality Gates (Must Run Every Phase)
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test --all-targets --all-features`
- [x] `cargo build`
- [x] Manual smoke pass of critical user journeys
- [x] `cargo build --release` at least once per major phase boundary (after Phases 2, 4, 6, 7)

## Manual Smoke Checklist (Every Phase)
- [x] Launch app, load existing workspace/session.
- [x] Launch with and without CLI model override and confirm active model precedence.
- [x] Send prompt, verify streaming + persistence.
- [x] Switch model, verify status/footer context stays coherent.
- [x] `/workspace delete` modal: Enter/Esc behavior.
- [x] `/session delete` modal: selected target and Enter/Esc behavior.
- [x] `/themes` modal live preview and cancel/confirm behavior.
- [x] Restart app, verify restored model/session/context telemetry.
- [x] On Windows, verify no startup/resize scrollbar gutter regression.

## Implementation Order Recommendation
- [x] Execute phases in strict order (0 -> 7).
- [x] One phase per commit (or small batch of atomic commits) for easy rollback.
- [x] Do not start a new phase until previous phase gate is fully green.

## Rollback Protocol
- [x] If any phase introduces non-trivial regressions, stop and revert only that phase scope.
- [x] Re-run full checks immediately after rollback.
- [x] Add missing regression test before re-attempting that phase.
- [x] If rollback occurs, capture root cause and patch plan with a preventive check before retry.

## Definition of Done (Whole Plan)
- [x] All phases completed.
- [x] All checks green.
- [x] No behavior/UI regressions in manual smoke checks.
- [x] `src/app/mod.rs` and `src/app/tui/mod.rs` reduced to coordinator-style modules.
- [x] Changelog updated with refactor summary (internal architecture, no user-facing behavior changes).
