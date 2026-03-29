# Olly - Project Context

## Project Overview

**Olly** is a Rust terminal UI (TUI) client for Ollama, designed for fast chat workflows with coding-focused features. Built with `ratatui`, it provides a keyboard-driven interface for interacting with local and cloud Ollama models.

### Core Features
- Streaming chat with live token rendering
- Model listing and switching (local + cloud models)
- Workspace and per-workspace session persistence with SQLite
- Full-text search over stored messages
- File-context ingestion with safety guardrails
- Markdown rendering with optional syntax highlighting
- Extensive slash-command system (`/search`, `/export`, `/checkpoint`, `/workspace`, `/session`, `/attach`, `/model`, `/url`, `/themes`)
- Configurable keybindings and runtime Ollama URL
- 8 built-in UI themes with live preview

### Architecture

The codebase follows a modular structure:

```
src/
├── main.rs          # CLI entry point with clap argument parsing
├── lib.rs           # Module exports
├── app/             # Core application logic and TUI
│   ├── mod.rs       # Main App struct, event handling, state management
│   ├── context.rs   # Application context
│   ├── input.rs     # Input handling
│   └── tui/         # Terminal UI rendering components
├── config/          # Configuration management (config.toml, platform paths)
├── history/         # Chat history and SQLite persistence
├── ollama/          # Ollama API client (streaming chat, models)
└── workspace/       # Workspace and session management
```

### Key Technologies
- **TUI Framework**: `ratatui` 0.29.0 with `crossterm` for terminal handling
- **Async Runtime**: `tokio` with multi-threaded runtime
- **HTTP Client**: `reqwest` with rustls-tls for Ollama API communication
- **Database**: `rusqlite` for local SQLite persistence
- **CLI Parsing**: `clap` with derive macros
- **Serialization**: `serde` + `serde_json`
- **Syntax Highlighting**: `syntect` for markdown code blocks
- **Error Handling**: `thiserror` for custom error types

## Building and Running

### Prerequisites
- Rust stable toolchain (configured via `rust-toolchain.toml`)
- Ollama server running locally (default: `http://localhost:11434`)

### Build Commands

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run with defaults
cargo run

# Run with custom Ollama URL and model
cargo run -- --url http://localhost:11434 --model llama3.2:latest
```

### Quality Checks

```bash
# PowerShell
./scripts/check.ps1

# Bash
./scripts/check.sh
```

These run:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`

### Running Tests

```bash
# All tests
cargo test --all-targets --all-features

# Library tests only
cargo test --lib

# Specific test file
cargo test --test streaming_parser
```

### Building Windows Installer

```powershell
# Requires Inno Setup 6 (ISCC.exe)
./scripts/build-installer.ps1
```

Output: `dist/windows-installer/Olly-Setup-<version>.exe`

### Release Preparation

```bash
# PowerShell
./scripts/release.ps1

# Bash
./scripts/release.sh
```

Uses `git-cliff` with `.git-cliff.toml` for changelog generation.

## Configuration

### Config File Location
- **Linux**: `~/.config/olly/config.toml`
- **macOS**: `~/Library/Application Support/olly/config.toml`
- **Windows**: `%APPDATA%\olly\config.toml`

### Data Storage
- **Linux**: `~/.local/share/olly/`
- **macOS**: `~/Library/Application Support/olly/`
- **Windows**: `%LOCALAPPDATA%\olly/`

### Example config.toml

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
```

## Development Conventions

### Code Style
- **Formatting**: `rustfmt` with `max_width = 100`, no hard tabs
- **Linting**: `clippy` with all warnings treated as errors
- **Edition**: Rust 2021

### Testing Practices
- Tests are in `tests/` directory as integration tests
- Common test utilities in `tests/common/`
- Every bug fix should include a reproducer test
- Keep shared fixtures deterministic (no real network calls)
- Test categories:
  - `streaming_*` - Streaming response handling
  - `persistence_*` - SQLite storage operations
  - `workspace_*` - Workspace/session management
  - `config_*` - Configuration handling
  - `ollama_*` - Ollama client behavior
  - `smoke_*` - Boot and basic functionality

### Commit Guidelines
- Clear, concise commit messages
- Focused PRs with single responsibility
- Document user-visible behavior changes
- Update `README.md` if config/commands change

### Pre-commit Hook

```bash
# PowerShell
./scripts/install-hooks.ps1

# Bash
./scripts/install-hooks.sh
```

## Key Commands and Shortcuts

### Chat Operations
- `Ctrl+Enter`: Send message
- `Enter`: Insert newline (or execute slash command)
- `Ctrl+Q`: Quit (press twice)
- `F1`: Toggle help
- `Esc`: Close dialogs and clear toasts
- `Tab`: Switch focus (sidebar/chat)
- `Up/Down`: Scroll chat or move selection
- `PgUp/PgDn`: Fast scroll

### Slash Commands
- `/commands` - List available commands
- `/models` - List/refresh/add/remove models
- `/themes` - Open theme picker modal
- `/search <query>` - Search chat history (FTS)
- `/export json|md <path>` - Export conversation
- `/checkpoint save|restore <name>` - Checkpoint management
- `/workspace list|switch|create|rename|delete` - Workspace operations
- `/session list|new|switch|rename|delete` - Session operations
- `/attach <image_path>` - Queue image attachment
- `/clear-attachments` - Clear queued attachments
- `/model <name>` - Switch active model
- `/url <http://host:port>` - Update Ollama URL

## Project Structure Summary

| Directory/File | Purpose |
|----------------|---------|
| `src/app/mod.rs` | Core App struct, event loop, state management (~4200 lines) |
| `src/app/tui/` | Terminal UI rendering components |
| `src/ollama/` | Ollama API client with streaming support |
| `src/workspace/` | Workspace and session SQLite management |
| `src/history/` | Chat history persistence |
| `src/config/` | Configuration loading and defaults |
| `tests/` | Integration tests (26 test files) |
| `scripts/` | Build, check, release, installer scripts |
| `docs/` | Documentation including release policy |
| `installer/` | Windows installer configuration |
| `.git-cliff.toml` | Changelog generation config |

## Important Notes

- **License**: Proprietary software owned by VectorMedia
- **Ownership**: All contributions licensed to VectorMedia
- **Security**: Report security issues privately to maintainers
- **Release Process**: Requires all quality gates to pass (fmt, clippy, tests)
- **Cross-platform**: Supports Linux (x86_64, aarch64), macOS (Intel, Apple Silicon), Windows (x86_64)
