# Troubleshooting

Common issues and solutions for Olly.

## Startup Issues

### App Won't Launch

**Symptom**: Running `olly` or `cargo run` produces no output or exits immediately.

**Causes**:
- Ollama server not running
- Config file has invalid TOML
- Terminal doesn't support required features

**Fix**:
1. Verify Ollama is running: `curl http://localhost:11434/api/tags`
2. Check config file syntax: `~/.config/olly/config.toml` (Linux), `%APPDATA%\olly\config.toml` (Windows)
3. Try with explicit URL: `olly --url http://localhost:11434`

### Console Display Issues (Windows)

**Symptom**: Layout artifacts, native scrollbar visible, or garbled output.

**Fix**:
- Olly includes hardened Windows console startup behavior. If issues persist:
  - Use Windows Terminal instead of legacy `conhost.exe`
  - Ensure terminal supports UTF-8 and mouse events
  - Try resizing the window after launch

### Model Not Found

**Symptom**: "Model not found" or empty model list on startup.

**Fix**:
1. Check Ollama has models: `ollama list`
2. Refresh model list in Olly: `/models refresh`
3. Pull a model: `ollama pull llama3.2:latest`
4. Verify URL: `/url http://localhost:11434`

## Chat Issues

### No Response from Model

**Symptom**: Message sent but no response appears.

**Causes**:
- Ollama server unreachable
- Model not loaded
- Timeout exceeded

**Fix**:
1. Check connection: Look for connection status in status bar
2. Verify model: `/model <name>` to switch
3. Increase timeout in config: `timeout_secs = 600`
4. Check Ollama logs for errors

### Streaming Stops Mid-Response

**Symptom**: Response starts but stops generating tokens.

**Causes**:
- Network interruption
- Ollama server error
- Model context window exceeded

**Fix**:
1. Check Ollama server logs
2. Reduce message length or start a new session
3. Try a different model with larger context window

### Slow Response Generation

**Symptom**: Model responds but tokens appear very slowly.

**Causes**:
- Hardware limitations (CPU vs GPU)
- Large model size
- High system load

**Fix**:
1. Use a smaller model (e.g., `llama3.2:1b` instead of `llama3.2:7b`)
2. Close other resource-intensive applications
3. Check GPU utilization: `nvidia-smi` (Linux/Windows with NVIDIA)

## Selection & Copy Issues

### Selection Doesn't Highlight

**Symptom**: Dragging mouse doesn't show visual selection.

**Causes**:
- Mouse outside chat area
- Theme doesn't support REVERSED modifier well
- Selection cleared by another action

**Fix**:
1. Ensure mouse is within chat panel boundaries
2. Try a different theme: `/themes`
3. Avoid scrolling during selection (use drag-to-scroll instead)

### Copied Text Doesn't Match Selection

**Symptom**: Pasted text differs from what was highlighted.

**Causes**:
- Mouse clicked in margin area of aligned text
- Selection indices became stale after resize

**Fix**:
1. Click directly on text, not in empty margins
2. Re-select after resizing the terminal
3. Selection is cleared on resize — this is intentional

### Auto-Scroll Doesn't Activate

**Symptom**: Dragging to edge of chat area doesn't scroll.

**Fix**:
1. Ensure you're within 3 rows of the top/bottom edge
2. Check that selection is active (you started with a drag)
3. Auto-scroll only works during active drag, not after release

## Tool Execution Issues

### Tools Not Working

**Symptom**: Tool calls fail or don't execute.

**Fix**:
1. Enable tools: `/tools enable` or `/tools on`
2. Check policy: `/tools policy auto` for automatic execution
3. Verify tool is allowlisted: `/tools allow <tool_name>`
4. Check workspace is active (tools require workspace root)

### Command Execution Rejected

**Symptom**: "Path rejected" or "Command not allowed" error.

**Causes**:
- Command not in allowlist
- Path outside workspace root
- Path contains executable token

**Fix**:
1. Allowlist the command: `/tools allow <command>`
2. Ensure CWD is within workspace root
3. Don't use path-like tokens (`./script.sh`, `/usr/bin/python`)

### Tool Loop Exceeded

**Symptom**: "Max tool steps exceeded" error.

**Fix**:
- Default limit is 4 tool calls per message
- This prevents runaway loops
- Send a new message to continue

## Workspace & Session Issues

### Workspace Not Found

**Symptom**: "Workspace not found" when switching.

**Fix**:
1. List workspaces: `/workspace list`
2. Create if missing: `/workspace create <name> --path <folder>`
3. Check workspace path exists on disk

### Session Lost After Restart

**Symptom**: Previous session not restored on startup.

**Fix**:
1. Sessions are persisted in SQLite — check data directory exists
2. UI state restores last active workspace/session automatically
3. If model changed via CLI flag, session context won't restore (by design)

### Can't Delete Last Workspace

**Symptom**: Delete confirmation modal appears but after confirming you see the welcome screen.

**Fix**:
- This is expected behavior — deleting the last workspace is allowed
- The app transitions to a zero-workspace state showing the welcome screen
- Create a new workspace to continue: `/workspace create --path <folder>`

## Theme Issues

### Theme Looks Wrong After Switch

**Symptom**: Colors appear incorrect or unreadable after switching themes.

**Fix**:
1. Switch back: `/themes` and select previous theme
2. Some themes are light (Paper Mint, Daybreak Canvas) — ensure terminal background isn't white
3. Restart app if rendering artifacts persist

## Performance Issues

### High Memory Usage

**Symptom**: Olly uses increasing memory over time.

**Causes**:
- Long conversation history
- Many tool call results stored
- Large model context window

**Fix**:
1. Start a new session: `/session new <title>`
2. Export and clear old conversations
3. Use a model with smaller context window

### UI Lag During Streaming

**Symptom**: Interface becomes unresponsive while model is generating.

**Fix**:
1. This shouldn't happen — Olly uses async streaming
2. Check if system is under heavy load
3. Try a smaller model for faster token generation

## Configuration Issues

### Config File Not Loading

**Symptom**: Settings don't persist between sessions.

**Fix**:
1. Check config file location:
   - Linux: `~/.config/olly/config.toml`
   - macOS: `~/Library/Application Support/olly/config.toml`
   - Windows: `%APPDATA%\olly\config.toml`
2. Validate TOML syntax
3. Check file permissions (must be readable)

### Database Path Invalid

**Symptom**: SQLite errors on startup.

**Fix**:
1. Check `db_path` in config points to valid directory
2. Ensure directory is writable
3. Default path: `<data_dir>/workspaces.db`

## Getting Help

If the issue isn't covered here:

1. Check the [GitHub issues](https://github.com/anomalyco/opencode/issues) for known problems
2. Run with verbose output if available
3. Include Olly version, OS, and terminal emulator when reporting
