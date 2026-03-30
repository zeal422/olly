# TOOLS_IMPLEMENTATION_PLAN.md

## Goal
Implement a safe, agent-style tool execution loop for Olly (file/search/shell tools), integrated into the existing chat UI and slash-command UX, with no regressions to current chat/workspace/session behavior.

## Scope
- Add runtime tool support to Ollama chat requests (`tools` schema + tool-result loop).
- Add built-in coding tools with safety controls.
- Add `/tools` command family for control and visibility.
- Integrate tool execution status into existing UI (chat + footer), without redesigning the interface.

## Out of Scope (MVP)
- [ ] Rich/collapsible tool-result widgets in chat (MVP uses compact + truncated result rendering).
- [ ] Remote tool execution providers.
- [ ] Non-Ollama backends.

## Assumptions & Constraints
- [ ] Tool execution remains local-only (no remote execution surface added).
- [ ] Behavior is cross-platform (Windows/Linux/macOS) without hard dependency on unavailable binaries.
- [ ] Default mode stays safe for existing users (tools disabled unless explicitly enabled).
- [ ] Tool-loop execution must not block TUI responsiveness.

## Non-Negotiables
- [ ] No breaking change to existing commands unrelated to tools.
- [ ] No regression in workspace/session persistence.
- [ ] No unrestricted command execution (must be policy-driven and sandboxed to workspace rules).
- [ ] Full quality gates remain green.

## Definition of Done (Whole Plan)
- [ ] Tools can be enabled/disabled per app runtime and persisted in UI/runtime state.
- [ ] Model can call tools, Olly executes them safely, sends tool results back, and completes response loop.
- [ ] `/tools` commands are discoverable via autocomplete/help and behave consistently.
- [ ] Confirmation flow works when policy is `confirm`.
- [ ] Footer and chat clearly indicate tool activity and outcomes.
- [ ] Tests cover happy paths + safety/error paths for each built-in tool.
- [ ] `README.md`, `AGENTS.md`, and `CHANGELOG.md` updated.
- [ ] `./scripts/check.ps1` + `cargo build --release` pass.

---

## Phase 0 - Baseline & Design Lock

### Tasks
- [ ] Capture baseline behavior for current chat pipeline and slash-command UX.
- [ ] Lock 3 critical decisions (must be finalized before coding):
  - [ ] Tool policy default: tools `off` by default, policy supports `auto|confirm`.
  - [ ] Workspace boundary: deny outside active root path with explicit user-facing error.
  - [ ] Max tool-loop steps default: `4` (configurable later).
- [ ] Lock async runtime approach:
  - [ ] command tool via async process execution (non-blocking),
  - [ ] filesystem-heavy work offloaded to `spawn_blocking`,
  - [ ] cancellation token/channel path for in-flight tool loops.
- [ ] Lock first tool set (MVP): `read_file`, `list_dir`, `grep_search`, `find_by_name`, `execute_command` (restricted).
- [ ] Record design lock in `docs/TOOLS_RUNTIME_DECISIONS.md` (short ADR-style note).

### Verification
- [ ] `./scripts/check.ps1`
- [ ] Manual smoke: normal chat still works exactly as before.

### Exit Criteria
- [ ] Critical decisions + async approach documented and approved in `docs/TOOLS_RUNTIME_DECISIONS.md`.

---

## Phase 1 - Core Tool Types & Request Wiring

### Tasks
- [ ] Extend Ollama request payload in `src/ollama/chat.rs` to include optional `tools`.
- [ ] Extend message model for tool-result compatibility (tool message metadata as needed by Ollama format).
- [ ] Add optional `think` request flag plumbing (configurable, default off for parity).
- [ ] Keep backward compatibility when tools are disabled (payload should remain minimal/compatible).
- [ ] Parse and preserve tool/thinking fields defensively (ignore unknown fields without failing stream parse).
- [ ] Implement model capability fallback: if a tool-enabled request fails for compatibility reasons, retry once without `tools` and surface a clear system notice.

### Verification
- [ ] Unit tests for JSON serialization/deserialization of:
  - [ ] chat request with/without tools
  - [ ] chat message with tool metadata
  - [ ] thinking field parse behavior (if returned)
  - [ ] backward-compatible parse with missing/unknown optional fields
- [ ] Integration test: non-tool-capable model path degrades gracefully (no crash, clear message, normal chat continues).
- [ ] `cargo test --lib ollama::chat -- --nocapture`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`

### Exit Criteria
- [ ] Chat client supports tool-aware payloads without breaking existing chat flow.

---

## Phase 2 - Tool Registry Module

### Tasks
- [ ] Create `src/app/tools/mod.rs`.
- [ ] Create `src/app/tools/registry.rs` for tool schemas exposed to model.
- [ ] Define typed tool descriptors:
  - [ ] name
  - [ ] description
  - [ ] JSON-schema parameters
  - [ ] risk level (read-only vs mutating vs shell)
- [ ] Add function to produce active tools list based on policy.

### Verification
- [ ] Snapshot tests for exported tool schema JSON.
- [ ] Tests for policy filtering (allowed/denied tools).
- [ ] `./scripts/check.ps1`

### Exit Criteria
- [ ] Registry provides deterministic, policy-filtered tool definitions.

---

## Phase 3 - Tool Executor (Read/Search)

### Tasks
- [ ] Create `src/app/tools/executor.rs` with dispatcher.
- [ ] Implement read/search tools:
  - [ ] `read_file(path, offset?, limit?)`
  - [ ] `list_dir(path)`
  - [ ] `grep_search(query, path?, case_sensitive?)`
  - [ ] `find_by_name(pattern, path?)`
- [ ] Enforce workspace-root boundary for file operations.
- [ ] Define explicit behavior when no active workspace/root path exists (deny with clear error).
- [ ] Add structured tool result format (success/error + compact output).
- [ ] Normalize path handling consistently across Windows/Linux/macOS.
- [ ] Define binary/non-UTF8 file behavior (safe summary instead of panic/garbled output).

### Verification
- [ ] Unit tests for each tool success path.
- [ ] Unit tests for blocked paths/traversal attempts.
- [ ] Unit tests for large-output truncation behavior.
- [ ] Unit tests for non-UTF8 content and permission-denied paths.
- [ ] Unit tests for no-active-workspace/no-root-path behavior.
- [ ] Integration tests with temp directories for read/search tools (avoid filesystem mocks for core behavior).
- [ ] `./scripts/check.ps1`

### Exit Criteria
- [ ] Read/search tools execute safely and return deterministic results.

---

## Phase 4 - Command Execution Tool (Restricted)

### Tasks
- [ ] Implement `execute_command(command, cwd?)` with strict safety:
  - [ ] fixed root allowlist only: `cargo`, `git`, `rg`, `python`, `node`
  - [ ] deny all other command roots explicitly (clear error message)
  - [ ] denylist destructive patterns (`rm -rf`, `del /s`, hard reset, etc.)
  - [ ] timeout + output cap
  - [ ] workspace-bound cwd validation
- [ ] Return structured stderr/stdout/status summaries.
- [ ] Ensure command execution is async/non-blocking for UI (spinner/status continues to update).

### Verification
- [ ] Tests for allowed commands.
- [ ] Tests for blocked commands.
- [ ] Tests for timeout enforcement.
- [ ] Tests for cwd boundary checks.
- [ ] Integration tests for command execution behavior in temp project directory.
- [ ] `./scripts/check.ps1`

### Exit Criteria
- [ ] Shell tool cannot execute disallowed/destructive commands.

---

## Phase 5 - Chat Pipeline Tool Loop

### Tasks
- [ ] Integrate tools into `src/app/chat_pipeline.rs`:
  - [ ] include `tools` in outgoing request when enabled
  - [ ] capture model tool calls
  - [ ] execute tools via executor
  - [ ] append `role=tool` messages
  - [ ] re-call model until completion or `max_tool_steps`
- [ ] Preserve existing persistence model (store assistant + tool messages and metadata).
- [ ] Add graceful error handling for failed tool calls (surface to model + user).
- [ ] Add cumulative output/token budget guard for tool responses to prevent runaway context growth.
- [ ] Add cancellation guard so a new send cannot interleave with a previous in-flight tool loop.
- [ ] Keep chat responsive during long-running tools (progress status/toast updates while work continues).

### Verification
- [ ] Integration tests for single-tool roundtrip.
- [ ] Integration tests for multi-step tool loop.
- [ ] Integration tests for max-step cutoff behavior.
- [ ] Integration tests for tool failure continuation.
- [ ] Integration tests for cancellation / re-send during tool execution.
- [ ] `./scripts/check.ps1`

### Exit Criteria
- [ ] End-to-end tool-call loop works reliably without hanging or runaway behavior.

---

## Phase 6 - `/tools` Command UX + Autocomplete

### Tasks
- [ ] Add `/tools` command group:
  - [ ] `/tools` (status + active policy)
  - [ ] `/tools on|off`
  - [ ] `/tools policy auto|confirm`
  - [ ] `/tools allow <tool>`
  - [ ] `/tools deny <tool>`
  - [ ] `/tools list`
- [ ] Integrate suggestions into autocomplete providers.
- [ ] Update help snapshots and command docs text.
- [ ] Ensure command errors are explicit and actionable (unknown tool/policy/value).

### Verification
- [ ] Command parser/dispatch tests.
- [ ] Autocomplete tests for `/tools` branches.
- [ ] Help-text snapshot tests.
- [ ] `./scripts/check.ps1`

### Exit Criteria
- [ ] Tool controls are discoverable and consistent with existing slash-command UX.

---

## Phase 7 - Confirmation Modal + UI Telemetry

### Tasks
- [ ] Add confirmation modal flow for risky tool actions under `policy=confirm`.
- [ ] Reuse overlay architecture (Enter confirm, Esc cancel).
- [ ] Add footer telemetry fields:
  - [ ] `tools:on/off`
  - [ ] `policy:auto/confirm`
  - [ ] `tool-step:x/y` during loop
- [ ] Add compact chat notices for tool execution start/end.
- [ ] Prevent overlay/input/sidebar key conflicts while tool confirmation modal is open.
- [ ] Keep tool result rendering compact by default (truncate + summary); defer collapsible rich view to post-MVP.

### Verification
- [ ] Overlay routing tests for tool confirmation.
- [ ] UI rendering tests for footer indicator/truncation.
- [ ] Manual smoke in small terminal sizes.
- [ ] `./scripts/check.ps1`

### Exit Criteria
- [ ] Users can see and control tool behavior clearly without UI clutter/regression.

---

## Phase 8 - Persistence, Docs, and Hardening

### Tasks
- [ ] Persist tool settings in runtime UI state/config (backward compatible defaults).
- [ ] Add migration-safe handling for missing fields in old state files.
- [ ] Update docs:
  - [ ] `README.md` (tooling usage and safety model)
  - [ ] `AGENTS.md` (architecture + commands)
  - [ ] `CHANGELOG.md` (new version entry)
- [ ] Add troubleshooting notes for common tool failures.
- [ ] Document security posture and limitations (what tools can and cannot do).

### Verification
- [ ] Restart persistence tests for tool state.
- [ ] Backward compatibility tests for old UI state JSON.
- [ ] `./scripts/check.ps1`
- [ ] `cargo build --release`

### Exit Criteria
- [ ] Tool settings survive restart and docs fully reflect behavior.

---

## Final Validation Checklist
- [ ] Full command smoke (`/model`, `/models`, `/workspace`, `/session`, `/tools`) passes.
- [ ] Normal chat (tools off) is unchanged.
- [ ] Tool-enabled chat works with at least one read tool and one shell tool.
- [ ] Behavior is correct after app restart (tool policy + defaults restored as designed).
- [ ] No clippy warnings.
- [ ] No failing tests.
- [ ] Installer still builds (`./scripts/build-installer.ps1`) after version alignment.

## Anti-Flake Rules
- [ ] One phase at a time; do not mix unrelated refactors.
- [ ] Add/adjust tests in same phase as behavior change.
- [ ] If regression appears, revert phase-local scope, add reproducer test, re-implement.
- [ ] Keep tool outputs bounded and deterministic to prevent flaky snapshots.

## Suggested Implementation Order
1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 3
5. Phase 4
6. Phase 5
7. Phase 6
8. Phase 7
9. Phase 8
