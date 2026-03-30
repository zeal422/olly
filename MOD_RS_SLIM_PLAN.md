# MOD_RS_SLIM_PLAN.md

## Goal
Slim `src/app/mod.rs` into a coordinator-focused module so future edits are faster, safer, and easier to reason about, with zero behavior/UI drift.

## Why This Plan Is Needed
`src/app/mod.rs` is still large and mixes responsibilities (state, orchestration, platform helpers, command/runtime glue, and a large in-file test block). This increases change risk and review overhead.

## Non-Negotiables
- [ ] No user-visible behavior changes.
- [ ] No command syntax/semantics changes.
- [ ] No persistence format changes.
- [ ] Startup precedence remains: CLI override > saved runtime state > config defaults.
- [ ] Keep all existing tests green.

## Measurable Success Criteria
- [ ] `src/app/mod.rs` line count reduced by at least 25% from baseline.
- [ ] `src/app/mod.rs` contains orchestration/state + small glue, not large feature logic blocks.
- [ ] No non-test function in `src/app/mod.rs` exceeds 120 lines.
- [ ] Full quality gates pass after each phase.

## Target Module Ownership
- `src/app/mod.rs`: `App` state + thin orchestration wrappers only.
- `src/app/platform/mod.rs`: Windows/console and terminal-sizing helpers.
- `src/app/commands/`: command execution orchestration + command routing.
- `src/app/runtime/`: event-loop-specific flows and confirm-delete executors.
- `src/app/mod_tests/*.rs` (wired via `#[cfg(test)]`): split unit test coverage currently embedded in `mod.rs`.

## Phase 1 - Baseline Lock

### Tasks
- [ ] Record baseline metrics:
  - [ ] current `src/app/mod.rs` line count,
  - [ ] top 10 largest functions in `src/app/mod.rs`.
- [ ] Confirm baseline quality gates:
  - [ ] `cargo fmt --all -- --check`
  - [ ] `cargo clippy --all-targets --all-features -- -D warnings`
  - [ ] `cargo test --all-targets --all-features`
  - [ ] `cargo build`
- [ ] Record baseline manual smoke expectations (same checklist already used in prior refactor cycle).

### Exit Criteria
- [ ] Baseline metrics and gate results captured.
- [ ] No existing regressions before refactor starts.

## Phase 2 - Platform Helper Extraction

### Tasks
- [ ] Create `src/app/platform/mod.rs`.
- [ ] Move from `src/app/mod.rs` into `platform`:
  - [ ] maximize/snap/resize console helpers,
  - [ ] scrollbar hide helpers,
  - [ ] buffer/window sync helpers.
- [ ] Keep call signatures stable for existing startup/runtime modules.
- [ ] Keep platform-specific behavior unchanged (especially Windows startup/resize behavior).

### Validation
- [ ] `./scripts/check.ps1`
- [ ] Manual smoke: Windows startup/resize scrollbar behavior.

### Exit Criteria
- [ ] `mod.rs` no longer owns low-level platform/console code.

## Phase 3 - Command Orchestration Slim-Down

### Tasks
- [ ] Move `execute_slash_command` orchestration out of `mod.rs` into `src/app/commands/execute.rs`.
- [ ] Keep current command handlers (`commands/handlers/*`) behavior unchanged.
- [ ] Preserve exact help/toast/system message strings.
- [ ] Keep model-context refresh triggers unchanged for `/model`, `/models`, `/url`.

### Validation
- [ ] `./scripts/check.ps1`
- [ ] Run focused command-related tests (existing suites).

### Exit Criteria
- [ ] `mod.rs` command path is a thin wrapper.

## Phase 4 - Confirm-Delete Flow Relocation

### Tasks
- [ ] Move:
  - [ ] `confirm_pending_workspace_delete`
  - [ ] `confirm_pending_session_delete`
  into `src/app/runtime/confirm_delete.rs`.
- [ ] Update `runtime/overlays.rs` to call new confirm-delete module.
- [ ] Keep Enter/Esc modal semantics identical.
- [ ] Preserve all not-found/selection-fallback behaviors and messages.

### Validation
- [ ] `./scripts/check.ps1`
- [ ] Focused tests:
  - [ ] selected target delete,
  - [ ] active fallback delete,
  - [ ] not-found handling.

### Exit Criteria
- [ ] `mod.rs` no longer contains detailed confirm-delete execution logic.

## Phase 5 - Split In-File `mod.rs` Tests

### Tasks
- [ ] Replace monolithic in-file `mod tests` block with `#[cfg(test)] mod mod_tests;` in `mod.rs`.
- [ ] Create `src/app/mod_tests/mod.rs` and split by domain:
  - [ ] `autocomplete.rs`
  - [ ] `workspace_session.rs`
  - [ ] `ui_runtime_state.rs`
  - [ ] `regressions.rs`
- [ ] Keep unit-test visibility model stable (`use super::*` where needed).
- [ ] Keep assertion behavior equivalent or stronger.

### Validation
- [ ] `./scripts/check.ps1`
- [ ] Confirm no test coverage drops for current mod.rs behaviors.

### Exit Criteria
- [ ] `mod.rs` test section removed or reduced to minimal test module wiring.

## Phase 6 - Final Consolidation

### Tasks
- [ ] Re-scan `mod.rs` for remaining oversized non-state helpers and extract if needed.
- [ ] Re-run function-size and file-size metrics vs baseline.
- [ ] Update architecture docs if ownership map changed:
  - [ ] `README.md` internal architecture section
  - [ ] `AGENTS.md` structure summary
- [ ] Add changelog entry for this internal refactor (next version).

### Final Validation
- [ ] `./scripts/check.ps1`
- [ ] `cargo build --release`
- [ ] Manual smoke pass:
  - [ ] startup precedence,
  - [ ] send/stream/persist,
  - [ ] workspace/session delete modal flows,
  - [ ] context telemetry restore,
  - [ ] sidebar + command suggestion navigation behavior.

### Exit Criteria
- [ ] All success criteria met.
- [ ] No regressions introduced.

## Execution Order
1. Phase 1
2. Phase 2
3. Phase 3
4. Phase 4
5. Phase 5
6. Phase 6

## Anti-Flake Rules
- [ ] One primary concern per phase (do not mix unrelated extractions).
- [ ] No broad renaming-only sweeps during this plan.
- [ ] If a phase regresses behavior, revert that phase scope, add a regression test, and retry.
