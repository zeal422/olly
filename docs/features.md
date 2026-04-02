# Features

Complete reference for Olly's user-facing features.

## Chat

### Streaming Responses
- Real-time token rendering as the model generates text
- Live status indicator during generation
- Token count telemetry in footer (`ctx:used/max`)

### Message Types
- **You** (user messages) - right-aligned
- **Assistant** (model responses) - left-aligned
- **System** (notices, errors) - center-aligned
- **Tool** (tool execution results) - labeled with tool name

### Text Selection & Copy
- **Mouse drag**: Click and drag to select text in chat history
- **Shift+click**: Click to set anchor, then Shift+click to select range
- **Drag-to-scroll**: Auto-scroll activates when dragging near chat area edges (3-row threshold)
- **Ctrl+C**: Copy selected text to clipboard with toast confirmation
- Selection is cleared on: scroll, new messages, focus changes, resize

### File Context
- `/attach <image_path>` - Queue image attachment for next message
- `/clear-attachments` - Clear queued attachments

## Keyboard Shortcuts

### Chat Operations
| Key | Action |
|-----|--------|
| `Ctrl+Enter` | Send message |
| `Enter` | Insert newline (or execute slash command) |
| `Ctrl+Q` | Quit (press twice) |
| `F1` | Toggle help overlay |
| `Esc` | Close dialogs and clear toasts |
| `Tab` | Switch focus (sidebar/chat) |
| `Up/Down` | Scroll chat or move sidebar selection |
| `PgUp/PgDn` | Fast scroll |
| `Mouse wheel` | Scroll chat history |
| `Right-click` | Paste clipboard content (chat input and slash commands only) |
| `Ctrl+C` | Copy selected text from chat |

### Input Editing
| Key | Action |
|-----|--------|
| `Left/Right` | Move cursor |
| `Up/Down` | Move cursor between lines |
| `Home` | Move to start of line |
| `End` | Move to end of line |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |

## Slash Commands

### Model Commands
| Command | Description |
|---------|-------------|
| `/model <name>` | Switch active model |
| `/models` | List available models |
| `/models refresh` | Refresh model list |
| `/models add <name-or-url>` | Add local model or enable cloud model |
| `/models remove <name>` | Remove a model |
| `/url <http://host:port>` | Update Ollama server URL |

### Workspace Commands
| Command | Description |
|---------|-------------|
| `/workspace list` | List all workspaces |
| `/workspace switch <id|name>` | Switch to a workspace |
| `/workspace create <name> --path <folder>` | Create workspace with path |
| `/workspace rename <new name>` | Rename active workspace |
| `/workspace delete` | Delete active workspace (opens confirmation) |
| `/workspace delete <id|name>` | Delete specific workspace |
| `/workspace where` | Show current workspace path |
| `/workspace set-path <id|name> --path <folder>` | Set workspace path |
| `/workspace export <path>` | Export workspace |
| `/workspace import <path>` | Import workspace |

### Session Commands
| Command | Description |
|---------|-------------|
| `/session list` | List sessions in current workspace |
| `/session new <title>` | Create new session |
| `/session switch <id|title>` | Switch to a session |
| `/session rename <id> <new title>` | Rename a session |
| `/session delete` | Delete active session (opens confirmation) |
| `/session delete <id|title>` | Delete specific session |

### Utility Commands
| Command | Description |
|---------|-------------|
| `/commands` | List all available commands |
| `/themes` | Open theme picker modal |
| `/search <query>` | Search chat history (full-text search) |
| `/export json|md <path>` | Export conversation |
| `/checkpoint save <name>` | Save checkpoint |
| `/checkpoint restore <name>` | Restore checkpoint |
| `/attach <image_path>` | Queue image attachment |
| `/clear-attachments` | Clear queued attachments |

### Tool Commands
| Command | Description |
|---------|-------------|
| `/tools` | Show tool execution status |
| `/tools enable` or `/tools on` | Enable tool execution |
| `/tools disable` or `/tools off` | Disable tool execution |
| `/tools policy auto` | Set auto-execution policy |
| `/tools policy confirm` | Set confirmation-required policy |
| `/tools list` | List available tools |
| `/tools allow <tool>` | Add tool to allowlist |
| `/tools deny <tool>` | Add tool to denylist |
| `/tools reset` | Clear allowlist and denylist |

## Tool Execution System

### Policies
- **Auto**: Tools execute without confirmation when allowlisted
- **Confirm**: Each tool execution requires user approval via modal

### Safety Sandboxing
- Path-like executable tokens are rejected (`/`, `\`, or leading `.`)
- Only `.exe`, `.cmd`, and `.bat` wrapper suffixes are normalized
- Dotted suffix forms like `git.evil` are rejected
- Allowlist/denylist for fine-grained tool control

## Themes

8 built-in themes with live preview via `/themes`:

| Theme | Type |
|-------|------|
| Emerald Dusk | Dark (default) |
| Neon Circuit | Dark |
| Sunset Ember | Dark |
| Ocean Midnight | Dark |
| Mono Amber | Dark |
| Sakura Night | Dark |
| Paper Mint | Light |
| Daybreak Canvas | Light |

## Sidebar

### Models Panel
- Local and cloud model groups (collapsible)
- Active model indicator
- Enter to toggle groups or switch model

### Workspaces Panel
- Workspace list with session trees (collapsible)
- Session selection per workspace
- Enter to toggle or switch workspace/session

### Status Bar
- Active workspace and session name
- Model name
- Connection status
- Context telemetry (`ctx:used/max`)

## Persistence

### Workspaces & Sessions
- SQLite-backed workspace and session management
- Per-workspace session history
- Auto-restore last active workspace/session on startup

### UI State
- Theme, collapsed panels, sidebar position persisted to `ui_state.json`
- Session context (model, token counts) restored across restarts
- Startup precedence: CLI flags > saved state

### Chat History
- Full message history per session
- Image attachments and tool call records
- Full-text search via SQLite FTS

## Configuration

Config file location:
- **Linux**: `~/.config/olly/config.toml`
- **macOS**: `~/Library/Application Support/olly/config.toml`
- **Windows**: `%APPDATA%\olly\config.toml`

```toml
[ollama]
url = "http://localhost:11434"
timeout_secs = 300

[defaults]
model = "llama3.2:latest"
workspace = "Default Workspace"

[storage]
db_path = "/path/to/olly/workspaces.db"

[ui]
syntax_highlighting = true

[keybindings]
send = "Ctrl+Enter"
newline = "Enter"
quit = "Ctrl+Q"
help = "F1"
dismiss = "Esc"
switch_focus = "Tab"
autocomplete = "Tab"
copy = "Ctrl+C"
```
