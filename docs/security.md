# Security

Olly's security model, tool execution safety, and data protection.

## Tool Execution Security

### Default Policy

Tools are **disabled by default**. Users must explicitly enable them with `/tools enable` or `/tools on`. This prevents unexpected autonomous behavior.

### Policy Modes

| Mode | Behavior |
|------|----------|
| `off` | Tools disabled (default) |
| `auto` | Tools execute immediately when allowlisted |
| `confirm` | Each tool execution requires user confirmation via modal |

### Workspace Sandboxing

All file operations are restricted to the active workspace root path:

- **Inside boundary**: Operations proceed normally
- **Outside boundary**: Denied with explicit error: `Path '{path}' is outside workspace root '{root}'. Tool call rejected.`
- **No active workspace**: All file tools fail with clear error message

This prevents accidental file system traversal and protects user data outside the workspace.

### Command Execution Safety

**Allowlist Model**: Only explicitly allowed commands can execute.

**Allowed roots (default)**:
- `cargo`
- `git`
- `rg` (ripgrep)
- `python`
- `node`

**Denied patterns**:
- `rm`, `del` (file deletion)
- `format` (disk formatting)
- `fdisk`, `dd` (disk operations)
- Destructive git operations

**Path Validation**:
- Path-like executable tokens are rejected (`/`, `\`, or leading `.`)
- Only `.exe`, `.cmd`, and `.bat` wrapper suffixes are normalized
- Dotted suffix forms like `git.evil` are rejected
- This prevents path injection attacks like `./malicious_script` or `/usr/bin/evil`

**Execution Limits**:
- Timeout: 60 seconds default
- Output cap: 10KB stdout/stderr combined (truncated with notice)
- CWD validation: Must be within workspace root

### Tool Allowlist/Denylist

Fine-grained control over individual tools:

```bash
/tools allow read_file    # Allow specific tool
/tools deny execute_command  # Deny specific tool
/tools reset              # Clear all allowlist/denylist
/tools list               # Show available tools
```

## Data Protection

### Local Storage

All data is stored locally:

| Data Type | Location | Encryption |
|-----------|----------|------------|
| Config | Platform config dir | None (plaintext TOML) |
| Workspaces DB | Platform data dir | None (SQLite) |
| UI State | Platform data dir | None (plaintext JSON) |
| Chat History | SQLite within workspace | None |

**Note**: Olly does not encrypt local data. Protect your machine and data directories with appropriate OS-level security.

### No Network Exfiltration

Olly only communicates with:
- **Ollama server** (local or configured URL) — for chat requests
- **No telemetry, analytics, or external services**

All chat data stays on your machine or your configured Ollama server.

### Ollama Connection Security

- Default URL: `http://localhost:11434` (local only)
- Custom URL: Configurable via `--url` flag or config file
- TLS: Uses `rustls` for HTTPS connections when connecting to remote servers
- No credential storage — Ollama API doesn't require authentication by default

## Model Capability Fallback

If a tool-enabled request fails:
1. Olly detects the failure (HTTP 400/422 or specific error)
2. Retries once without the `tools` array
3. Surfaces notice: "Model may not support tools - falling back to standard chat"
4. Tool metadata is still stored even if model returns empty tool_calls

This prevents information leakage to models that don't support tools.

## Input Safety

### Slash Command Validation

- Commands are parsed and routed through a type-safe dispatcher
- Unknown commands return "Unknown command. Use /commands"
- Command arguments are validated before execution

### Clipboard Integration

- Paste operations only work in chat input and slash command mode
- Clipboard access errors are handled silently (no crash)
- No clipboard data is stored or transmitted

### Text Selection

- Selection state is cleared on scroll, new messages, focus changes, and resize
- Copied text is sent to system clipboard only — not stored in app state
- Selection indices reference the visual lines buffer, preventing stale data access

## Threat Model

### What Olly Protects Against

- **Accidental file deletion**: Workspace sandboxing prevents operations outside workspace root
- **Runaway tool loops**: Max 4 tool calls per message
- **Unauthorized command execution**: Allowlist-only model with denylist patterns
- **Path injection attacks**: Path-like executable tokens rejected
- **UI state corruption**: SQLite transactions with rollback on failure
- **Terminal escape injection**: Raw mode and proper input sanitization

### What Olly Does NOT Protect Against

- **Malicious Ollama server**: If you connect to an untrusted Ollama server, it could return harmful responses
- **Compromised local machine**: Olly doesn't encrypt local data
- **Social engineering**: Tool confirmation modal helps but doesn't prevent all user errors
- **Resource exhaustion**: Very large models or conversations can consume significant memory

## Security Best Practices

1. **Keep tools disabled** unless you need them
2. **Use `confirm` policy** for untrusted tool usage
3. **Review tool calls** before confirming in confirm mode
4. **Set workspace paths** carefully — tools can access anything within the workspace root
5. **Don't connect to untrusted Ollama servers** — they control the model responses
6. **Keep Olly updated** — security fixes are released with new versions

## Reporting Security Issues

If you discover a security vulnerability:

1. **Do not** post public exploit details
2. Open a private report with maintainers
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

## Audit Trail

Tool execution results are stored in chat history with:
- Tool name
- Arguments (truncated in display, full in storage)
- Success/error status
- Output (truncated in display, full in storage)

This provides an audit trail of all tool operations performed during a session.
