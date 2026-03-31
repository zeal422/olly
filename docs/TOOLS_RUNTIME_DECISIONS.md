# Tools Runtime Decisions (ADR)

## Status
Design Locked - Phase 0 Complete

## Context
Implementing agent-style tool execution for Olly TUI client. This document locks critical design decisions before implementation begins.

## Decisions

### 1. Tool Policy Default
- **Decision**: Tools are `off` by default
- **Policy modes**: `auto` (execute immediately) | `confirm` (user confirmation required)
- **Scope**: Runtime toggleable, persisted in UI state
- **Rationale**: Existing users must not be surprised by new autonomous behavior

### 2. Workspace Boundary Enforcement
- **Decision**: File operations are sandboxed to active workspace root path
- **Behavior outside boundary**: Deny with explicit error: "Path '{path}' is outside workspace root '{root}'. Tool call rejected."
- **No active workspace/root**: All file tools fail with clear error message
- **Rationale**: Prevents accidental file system traversal and maintains user trust

### 3. Max Tool-Loop Steps
- **Decision**: Default limit of `4` tool calls per user message
- **Configurable**: Will be runtime-configurable (Phase 8), but MVP uses hardcoded default
- **Rationale**: Prevents runaway loops while allowing multi-step reasoning

### 4. Async Runtime Approach
- **Command tool**: Uses `tokio::process::Command` (async, non-blocking)
- **Filesystem work**: Uses `tokio::task::spawn_blocking` for I/O-heavy operations
- **Cancellation**: `tokio_util::sync::CancellationToken` passed through tool execution context
- **UI updates**: Status bar shows "Tool: step X/Y" during execution; spinner continues
- **Rationale**: TUI must remain responsive during long `cargo test` or `rg` searches

### 5. First Tool Set (MVP)
| Tool | Risk Level | Description |
|------|------------|-------------|
| `read_file` | Read-only | Read file contents with optional offset/limit |
| `list_dir` | Read-only | List directory entries |
| `grep_search` | Read-only | Pattern search (using regex) |
| `find_by_name` | Read-only | Find files by glob pattern |
| `execute_command` | Shell | Restricted command execution (allowlist only) |

### 6. Command Execution Safety Model
- **Allowlist roots only**: `cargo`, `git`, `rg`, `python`, `node`
- **Denylist patterns**: `rm`, `del`, `format`, `fdisk`, `dd`, destructive git operations
- **Timeout**: 60 seconds default
- **Output cap**: 10KB stdout/stderr combined (truncated with notice)
- **CWD validation**: Must be within workspace root

### 7. Model Capability Fallback
- **Detection**: If tool-enabled request fails with HTTP 400/422 or specific error message
- **Fallback behavior**: Retry once without `tools` array, surface system notice: "Model may not support tools - falling back to standard chat"
- **Storage**: Tool metadata is still stored if model returns tool_calls, even if empty

### 8. Tool Result Format
- **Success**: `{"status": "success", "output": "...", "truncated": false}`
- **Error**: `{"status": "error", "error": "...", "type": "path_denied|not_found|timeout|..."}`
- **Rendering**: Compact single-line in chat (truncated), full in persistence

## Consequences
- Tool execution cannot be used for system administration outside workspace
- Users must explicitly enable tools and understand the trust model
- Long-running commands are interruptible via normal quit flow
- Models without tool support degrade gracefully

## Implementation Order
1. Phase 1: Request wiring + fallback
2. Phase 2: Tool registry
3. Phase 3: Read/search executor
4. Phase 4: Command executor
5. Phase 5: Chat pipeline integration
6. Phase 6: /tools commands
7. Phase 7: Confirmation modal + UI
8. Phase 8: Persistence + docs

## References
- Ollama Tool Calling: https://docs.ollama.com/capabilities/tool-calling
- TOOLS_IMPLEMENTATION_PLAN.md (parent document)
