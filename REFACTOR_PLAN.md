# REFACTOR_PLAN.md

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
- [ ] No user-visible behavior changes.
- [ ] No visual/layout changes in TUI.
- [ ] No command syntax/semantics changes.
- [ ] No persistence format changes (except backward-compatible no-op metadata when unavoidable).
- [ ] Backward compatibility for existing `ui_state.json` files must be preserved (missing/older fields deserialize safely).
- [ ] Startup precedence must remain stable: CLI overrides > persisted runtime state > config defaults.
- [ ] Keep all existing tests passing throughout.
- [ ] Every refactor step must be covered by tests/checks before moving to next phase.

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
- [ ] No feature work while refactor phases are active.
- [ ] Do not mix unrelated refactors in the same commit.
- [ ] Avoid changing more than one high-risk area per phase (runtime, commands, tui, persistence) at once.
- [ ] Keep module extraction commits mechanically simple first, behavior-preserving cleanup second.
- [ ] If a phase exceeds manageable diff size, split phase into sub-phases before continuing.

---

## Phase 0 - Baseline Lock and Refactor Harness
Goal: Create a safety net and baseline so later changes can be validated quickly.

### Tasks
- [ ] Create a baseline checklist document section in PR notes (runtime behavior matrix).
- [ ] Run and record current full gate results (`./scripts/check.ps1`).
- [ ] Record current startup precedence behavior (CLI model override vs persisted model vs config default).
- [ ] Identify and list critical user journeys to validate after each phase:
  - [ ] start app and load workspace/session,
  - [ ] send chat message and stream response,
  - [ ] `/workspace` create/switch/rename/delete,
  - [ ] `/session` create/switch/rename/delete with modal,
  - [ ] `/models`, `/model`, `/url`, `/themes`, `/export`, `/checkpoint`, `/attach`.
- [ ] Add any missing high-value regression tests before moving code (only where gaps exist).
  - [ ] Add/confirm tests for runtime-state backward compatibility (`ui_state.json` older shape).
  - [ ] Add/confirm tests for startup model precedence behavior.
  - [ ] Add/confirm restart telemetry restore (`ctx:used/max`) behavior.

### Required Checks
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] `cargo build`

### Exit Criteria
- [ ] Baseline is green and documented.
- [ ] High-risk behaviors have explicit tests.

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
- [ ] Introduce command AST/enums with strict parser unit tests.
- [ ] Move parse logic out of `handle_command`.
- [ ] Move each command family into dedicated handler module.
- [ ] Keep `handle_command` as thin adapter calling dispatcher.
- [ ] Keep existing command strings/help/toasts unchanged.
- [ ] Keep existing module/public function signatures stable where feasible to reduce integration churn.

### Tests
- [ ] Existing command tests continue to pass unchanged.
- [ ] Add parser-specific unit tests for edge cases:
  - [ ] quoted args,
  - [ ] `--to`, `--path`,
  - [ ] unknown/empty command fallback.
- [ ] Add snapshot-style assertions for command help output where brittle.

### Required Checks
- [ ] full `./scripts/check.ps1`

### Exit Criteria
- [ ] `handle_command` reduced to orchestration only.
- [ ] command-specific logic lives in `commands/handlers/*`.

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
- [ ] Extract startup initialization sequence (config/session/model bootstrap).
- [ ] Extract event polling + key event dispatch.
- [ ] Extract overlay handling blocks:
  - [ ] help modal,
  - [ ] workspace delete modal,
  - [ ] session delete modal,
  - [ ] theme picker modal.
- [ ] Extract chat-input action handler (`Insert`, `Backspace`, arrows, newline/send).
- [ ] Preserve exact keybinding behavior and focus rules.
- [ ] Preserve startup precedence behavior explicitly (CLI > persisted runtime state > config defaults).

### Tests
- [ ] Existing keybinding and modal tests pass.
- [ ] Add targeted tests for action routing precedence:
  - [ ] overlay active vs normal input,
  - [ ] sidebar-focused slash command behavior,
  - [ ] command suggestion navigation precedence.

### Required Checks
- [ ] full `./scripts/check.ps1`

### Exit Criteria
- [ ] `run` becomes high-level orchestration only.
- [ ] event/action behavior unchanged in tests.

---

## Phase 3 - Chat Send/Stream Pipeline Extraction
Goal: Isolate the send+stream+persistence pipeline from UI event handling.

### Target Structure
- `src/app/chat_pipeline.rs`
- (optional) `src/app/chat_pipeline/types.rs`

### Tasks
- [ ] Extract prompt send flow:
  - [ ] user message persistence,
  - [ ] request preparation,
  - [ ] streaming callbacks,
  - [ ] assistant/tool persistence,
  - [ ] telemetry updates (`prompt_eval_count`, `eval_count`),
  - [ ] error path handling.
- [ ] Keep rendering hooks/callback contracts stable.
- [ ] Keep context-window refresh triggers exactly as now.
- [ ] Keep persistence ordering unchanged (user message save -> assistant/tool save -> telemetry/state snapshot updates).

### Tests
- [ ] Existing streaming/network tests pass unchanged.
- [ ] Add focused unit tests for pipeline state transitions:
  - [ ] success with content,
  - [ ] success with empty content,
  - [ ] stream failure,
  - [ ] tool-call capture/persist.

### Required Checks
- [ ] full `./scripts/check.ps1`

### Exit Criteria
- [ ] Streaming logic no longer embedded in runtime event switch.

---

## Phase 4 - Session/Workspace Domain Operations Cleanup
Goal: Centralize workspace/session transition and delete-confirm behavior.

### Target Structure
- `src/app/domain/workspace_session.rs`
- `src/app/domain/context_state.rs`

### Tasks
- [ ] Extract repeated workspace/session state sync blocks into reusable helpers.
- [ ] Consolidate delete confirmation executors and selection resolution helpers.
- [ ] Remove duplicated "refresh + sync + focus" call chains via common utility.
- [ ] Preserve existing toast/status/system message texts.

### Tests
- [ ] Existing workspace/session behavior tests pass.
- [ ] Add tests for selection fallback matrix:
  - [ ] selected session/workspace,
  - [ ] active fallback,
  - [ ] not-found messaging.

### Required Checks
- [ ] full `./scripts/check.ps1`

### Exit Criteria
- [ ] No duplicated transition code blocks in command handlers.

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
- [ ] Create a provider interface for suggestions.
- [ ] Move workspace/session/model logic into dedicated providers.
- [ ] Keep ordering, filtering, casing, and exact outputs stable.
- [ ] Keep `/session delete` single-command behavior intact.

### Tests
- [ ] Existing autocomplete tests pass unchanged.
- [ ] Add provider-level tests for complex branches.

### Required Checks
- [ ] full `./scripts/check.ps1`

### Exit Criteria
- [ ] `autocomplete_suggestions` becomes thin coordinator.

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
- [ ] Extract modal rendering into reusable function (`render_confirm_delete_modal`).
- [ ] Extract sidebar/chat/input/status rendering modules.
- [ ] Keep all layout constants and calculations unchanged.
- [ ] Keep color/theme semantics unchanged.

### Tests
- [ ] Existing tui tests pass unchanged.
- [ ] Add one or two extra deterministic tests for modal text/layout guards.

### Required Checks
- [ ] full `./scripts/check.ps1`

### Exit Criteria
- [ ] `tui/mod.rs` primarily composes submodule renders.

---

## Phase 7 - State Struct Cleanup and Naming Pass
Goal: Improve readability and ownership boundaries in app state.

### Tasks
- [ ] Group fields in `App` by concern using nested structs (internal only), e.g.:
  - [ ] `AppUiState`,
  - [ ] `AppSessionState`,
  - [ ] `AppTelemetryState`.
- [ ] Rename ambiguous helpers to intent-revealing names.
- [ ] Remove dead/duplicated helper functions discovered during previous phases.
- [ ] Keep serde compatibility for persisted UI runtime state.
- [ ] Keep migration-free compatibility with prior `ui_state.json` files from older releases.

### Tests
- [ ] Existing tests pass.
- [ ] Add/adjust serialization compatibility tests for `ui_state.json`.

### Required Checks
- [ ] full `./scripts/check.ps1`

### Exit Criteria
- [ ] `App` is easier to navigate and reason about.
- [ ] No behavior changes detected.

---

## Cross-Phase Quality Gates (Must Run Every Phase)
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] `cargo build`
- [ ] Manual smoke pass of critical user journeys
- [ ] `cargo build --release` at least once per major phase boundary (after Phases 2, 4, 6, 7)

## Manual Smoke Checklist (Every Phase)
- [ ] Launch app, load existing workspace/session.
- [ ] Launch with and without CLI model override and confirm active model precedence.
- [ ] Send prompt, verify streaming + persistence.
- [ ] Switch model, verify status/footer context stays coherent.
- [ ] `/workspace delete` modal: Enter/Esc behavior.
- [ ] `/session delete` modal: selected target and Enter/Esc behavior.
- [ ] `/themes` modal live preview and cancel/confirm behavior.
- [ ] Restart app, verify restored model/session/context telemetry.
- [ ] On Windows, verify no startup/resize scrollbar gutter regression.

## Implementation Order Recommendation
- [ ] Execute phases in strict order (0 -> 7).
- [ ] One phase per commit (or small batch of atomic commits) for easy rollback.
- [ ] Do not start a new phase until previous phase gate is fully green.

## Rollback Protocol
- [ ] If any phase introduces non-trivial regressions, stop and revert only that phase scope.
- [ ] Re-run full checks immediately after rollback.
- [ ] Add missing regression test before re-attempting that phase.
- [ ] If rollback occurs, capture root cause and patch plan with a preventive check before retry.

## Definition of Done (Whole Plan)
- [ ] All phases completed.
- [ ] All checks green.
- [ ] No behavior/UI regressions in manual smoke checks.
- [ ] `src/app/mod.rs` and `src/app/tui/mod.rs` reduced to coordinator-style modules.
- [ ] Changelog updated with refactor summary (internal architecture, no user-facing behavior changes).
