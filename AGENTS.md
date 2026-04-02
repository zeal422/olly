# Olly - Project Context

**Olly** is a Rust TUI client for Ollama. Keyboard-driven chat with tool execution, workspace persistence, and text selection.

## Getting Started

For LLMs and agents working on this codebase, read these files in order:

1. **`SOURCE_MAP.md`** — Module dependency graph and import relationships
2. **`docs/architecture.md`** — Module hierarchy, data flow, design decisions
3. **`docs/features.md`** — User-facing features, shortcuts, commands
4. **`docs/workflow.md`** — Event loop, input pipeline, rendering cycle

## Core Features

- Streaming chat with live token rendering
- Model listing/switching (local + cloud)
- **Tool execution** with confirmation modals, policy controls (auto/confirm), workspace sandbox
- Workspace/session persistence with SQLite
- Full-text search, file-context ingestion, markdown with syntax highlighting
- 8 themes with live preview, configurable keybindings
- Mouse drag text selection, Shift+click range selection, Ctrl+C copy
- Right-click paste, mouse-wheel scroll, drag-to-scroll

## Architecture

Modular structure with clear separation of concerns:

```
src/
|-- main.rs                  # CLI entry point
|-- lib.rs                   # Module exports
|-- app/
|   |-- mod.rs               # Coordinator + re-exports (~325 lines)
|   |-- state.rs             # App struct + methods (~1,061 lines)
|   |-- input_editor.rs      # Cursor movement and text editing
|   |-- chat_navigation.rs   # Scroll and selection management
|   |-- messages_and_conversation.rs  # Message push operations
|   |-- overlay_toast.rs     # Toast, help overlay, confirmations
|   |-- theme_picker.rs      # Theme picker state
|   |-- input.rs             # InputAction enum, key/mouse resolution
|   |-- chat_pipeline.rs     # Send + stream + persist
|   |-- commands/            # Slash-command parser/dispatcher/handlers
|   |-- runtime/             # Startup, event loop, key actions, shutdown
|   |-- platform/            # Windows console helpers
|   |-- autocomplete/        # Command suggestions
|   |-- domain/              # Workspace/session helpers
|   |-- tools/               # Tool registry, executor, safety
|   |-- mod_tests/           # App-level unit tests
|   `-- tui/                 # Rendering components
|       |-- mod.rs           # Draw orchestration + shared utils (~765 lines)
|       |-- chat.rs          # Chat panel
|       |-- input.rs         # Input panel
|       |-- sidebar.rs       # Sidebar + status bar
|       |-- palette.rs       # Theme palettes
|       |-- markdown.rs      # Markdown + syntax highlighting
|       `-- modals.rs        # Modal rendering
|-- config/                  # Configuration
|-- history/                 # Chat history persistence
|-- ollama/                  # Ollama API client
`-- workspace/               # Workspace/session SQLite management
```

Full module dependency graph: see `SOURCE_MAP.md`.
Refactoring history and principles: see `docs/refactoring.md`.

## Building and Running

```bash
cargo build              # Debug
cargo build --release    # Release
cargo run                # Run with defaults
cargo run -- --url http://localhost:11434 --model llama3.2:latest
```

### Quality Checks

```bash
./scripts/check.ps1   # or ./scripts/check.sh
```

Runs: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`, `cargo test`.

### Development Scripts

| Script | Purpose |
|--------|---------|
| `scripts/fmt-fix.ps1` / `.sh` | Auto-apply formatting + clippy fixes |
| `scripts/test-unit.ps1` / `.sh` | Unit tests only (fast) |
| `scripts/test-integration.ps1` / `.sh` | Integration tests only |
| `scripts/run-dev.ps1` / `.sh` | Quick dev launch |
| `scripts/clean.ps1` / `.sh` | Remove build artifacts |
| `scripts/audit.ps1` / `.sh` | Check dependency vulnerabilities |
| `scripts/coverage.ps1` / `.sh` | Generate coverage report |
| `scripts/loc.ps1` / `.sh` | Count lines per module |
| `scripts/docs-check.ps1` / `.sh` | Verify documentation health |

Full script reference: see `docs/release.md`.

## Key Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+Enter` | Send message |
| `Enter` | Newline / execute slash command |
| `Ctrl+Q` | Quit (press twice) |
| `F1` | Toggle help |
| `Esc` | Close dialogs, clear toasts |
| `Tab` | Switch focus (sidebar/chat) |
| `Up/Down` | Scroll chat or move selection |
| `PgUp/PgDn` | Fast scroll |
| `Mouse wheel` | Scroll chat |
| `Right-click` | Paste (chat input only) |
| `Ctrl+C` | Copy selected text |

Full shortcut reference: see `docs/features.md`.

## Configuration

Config: `~/.config/olly/config.toml` (Linux), `%APPDATA%\olly\config.toml` (Windows).
Data: `~/.local/share/olly/` (Linux), `%LOCALAPPDATA%\olly/` (Windows).

Full config reference: see `docs/features.md`.

## Development Conventions

- **Formatting**: `rustfmt`, `max_width = 100`, no hard tabs
- **Linting**: `clippy -D warnings`
- **Tests**: `tests/` for integration, `src/app/mod_tests/` for unit. Every bug fix needs a reproducer test.
- **Commits**: Clear messages, focused PRs. Update `README.md` if config/commands change. Update `SOURCE_MAP.md` if module structure changes.

## Agent Discipline & Git Safety

### CRITICAL RULES

1. **NEVER run destructive Git commands without explicit user confirmation** — always show `git status` first and ask.
2. **When stuck, STOP and ask** — never act impulsively.
3. **Always verify current state** — read files before modifying, check `git status`.
4. **Make small, reversible changes** — use precise edits, commit frequently.
5. **Acknowledge mistakes immediately** — offer recovery options, do not hide errors.

### Mental Discipline Checklist

Before any data-risking operation:
- [ ] Read the file(s) recently?
- [ ] `git status` shows no uncommitted changes that could be lost?
- [ ] Is there a non-destructive alternative?
- [ ] Would a commit or stash be safer?
- [ ] Am I acting out of frustration?
- [ ] Communicated the plan and received confirmation?

**Remember: It's better to ask for permission than to apologize for irreversible damage.**
