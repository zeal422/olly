# TO_FIX Real Bug Implementation Plan

## Goal
Fix the **real and actionable** bugs from `TO_FIX.md` with regression tests and full quality checks, while avoiding churn on items that are not currently reproducible as real defects.

## Scope
In scope:
- BUG #1 (UTF-8 truncate panic risk)
- BUG #2 (assistant `tool_calls` dropped on reload)
- BUG #3 (intermediate assistant `tool_calls` not persisted)
- BUG #4 (misleading `(no response content)` fallback when tool-only turn)
- BUG #5 (modifier matching uses subset, not exact)
- BUG #6 (startup uses default model instead of CLI override for workspace session open)
- BUG #7 (first imported conversation title not applied)
- BUG #8 (path normalization drops excess `..`)
- BUG #9 (path normalization root handling incomplete on Windows forms)

Out of scope (for now):
- BUG #10 as currently written (FTS trigger/column-order claim does not match current schema/trigger references).

## Phase 0 - Baseline + Safety Rails
- [ ] Capture baseline state (`cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo build`, `cargo test`).
- [ ] Add/confirm issue labels in comments/tests so each bug has a direct reproducer test reference.
- [ ] Create a short mapping table (bug id -> file(s) -> test name(s)) at the top of the implementation PR/notes.

Exit criteria:
- Baseline is green before code changes.
- Each bug has a planned regression test location.

## Phase 1 - Output/Path Safety Fixes (BUG #1, #8, #9)

### 1A) UTF-8-safe truncation utility (BUG #1)
- [ ] Introduce a shared helper in `src/app/tools/executor.rs`:
  - Input: `&mut String`, `max_bytes: usize`
  - Behavior: truncate only at valid UTF-8 boundary
  - Return: whether truncation occurred
- [ ] Replace all direct `truncate(MAX_OUTPUT_SIZE)` call sites with helper usage.

### 1B) Robust `clean_path` normalization (BUG #8, #9)
- [ ] Rework `clean_path` to:
  - Preserve unresolved/excess `..` where appropriate instead of silently dropping.
  - Handle `RootDir` explicitly in reconstruction logic.
  - Preserve Windows prefix/root semantics consistently.
- [ ] Ensure behavior remains stable for existing happy-path usage and workspace boundary checks.

### 1C) Tests for Phase 1
- [ ] Add unit tests for UTF-8 truncation helper with multi-byte text (emoji/CJK/accented content).
- [ ] Add/extend path-normalization tests for:
  - Excess `..` handling
  - Rooted Windows-style path semantics
  - No regressions for normal in-workspace paths

Exit criteria:
- No direct unsafe `truncate(MAX_OUTPUT_SIZE)` remaining.
- New normalization tests pass on current platform.
- Existing tool/path security tests remain green.

## Phase 2 - Tool-Call Context Integrity (BUG #2, #3, #4)

### 2A) Persist structured tool-call data in app state (BUG #2)
- [ ] Extend `Message` with structured assistant tool-call storage (not only preview text).
- [ ] Populate structured tool-calls in `from_storage_role()`.
- [ ] Use structured tool-calls in `to_ollama_messages()` for assistant replay.

### 2B) Persist intermediate assistant tool-call turns (BUG #3)
- [ ] In `chat_pipeline`, persist assistant tool-call message(s) before each tool result persistence.
- [ ] Keep ordering consistent with Ollama tool protocol:
  - user -> assistant(tool_calls) -> tool -> ... -> assistant(final)

### 2C) Correct no-content fallback semantics (BUG #4)
- [ ] Change fallback to `(no response content)` only when there is neither streamed text nor captured tool-calls.

### 2D) Tests for Phase 2
- [ ] Add/extend tests to verify replay preserves assistant `tool_calls` across persistence/reload.
- [ ] Add test for tool-only assistant turn not being replaced by fallback content.
- [ ] Add ordering test for persisted message sequence in multi-step tool loops.

Exit criteria:
- Structured tool-call chain survives reload and replay.
- Tool-loop persistence order matches protocol expectations.
- Tool-only turns do not get misleading fallback text.

## Phase 3 - Input/Startup/Import Correctness (BUG #5, #6, #7)

### 3A) Key modifier exact match (BUG #5)
- [ ] Update keybinding matcher to exact modifier equality for configured bindings.
- [ ] Verify defaults still behave as intended (`Ctrl+Enter` send, `Enter` newline).

### 3B) Startup model override flow (BUG #6)
- [ ] Use `startup_model` when opening workspace session during bootstrap.
- [ ] Confirm CLI model override governs initial session preset creation path.

### 3C) Import first conversation title application (BUG #7)
- [ ] During workspace import, apply imported title to first (pre-created) conversation.

### 3D) Tests for Phase 3
- [ ] Add/extend input keybinding test: unmodified binding must not match modified event.
- [ ] Add startup/bootstrap test that validates override model is used in session open path.
- [ ] Add import/export test asserting first imported conversation title is preserved.

Exit criteria:
- Input binding resolution is deterministic and exact.
- CLI model override is used for initial session open.
- First imported conversation title is preserved.

## Phase 4 - Full Verification + Documentation
- [ ] Run full quality gates:
  - `cargo fmt --all`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo build`
  - `cargo test`
- [ ] Verify `TO_FIX.md` statuses reflect actual outcomes (fixed / reworded / not real).
- [ ] Add changelog entry (next version) summarizing bug fixes and new regression tests.

Exit criteria:
- All checks green.
- All in-scope bugs resolved with tests.
- Changelog updated.

## Definition of Done
- [ ] Bugs #1-#9 (except #10 by scope decision) are fixed or intentionally reclassified with evidence.
- [ ] Every fixed bug has at least one regression test.
- [ ] No clippy warnings, no test failures, no build failures.
- [ ] Behavior is preserved outside the targeted bug fixes.

## Execution Order Rationale
1. Fix safety hazards first (truncate/path normalization) to avoid runtime panic/security drift.
2. Fix tool-call persistence chain second (highest correctness impact on chat/tool protocol).
3. Fix input/startup/import correctness third (localized, lower coupling).
4. Run full checks only after each phase and once again at the end.
