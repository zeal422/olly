# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project follows Semantic Versioning.

## [0.0.5] - 2026-03-29

### Added
- Release workflow with cross-platform build matrix for Linux (`x86_64`, `aarch64`), macOS (`x86_64`, `aarch64`), and Windows (`x86_64`).
- Artifact packaging and checksum generation (`SHA256SUMS.txt`) for release assets.
- GitHub release publishing automation with release-note generation via `git-cliff`.
- Release helper scripts: `scripts/release.sh` and `scripts/release.ps1`.
- Contributor documentation in `CONTRIBUTING.md` and release gating documentation in `docs/RELEASE_POLICY.md`.
- Dual-license files: `LICENSE`, `LICENSE-MIT`, and `LICENSE-APACHE`.
- README expansion with installation, screenshot, features, config guide, shortcuts, troubleshooting, and release workflow details.
- Screenshot asset at `docs/screenshots/olly-tui.svg`.

### Changed
- CI workflow now separates PR fast checks from full-suite checks for merge/nightly events.
- `Cargo.toml` now includes crates.io-ready metadata (description, readme, repository/homepage/docs links, keywords, categories).
- `PROJECT_PLAN.md` Phase 5 checklist marked complete with implementation notes.

## [0.0.4] - 2026-03-29

### Added
- Slash-command power features: `/commands`, `/search`, `/export`, `/checkpoint`, `/workspace`, `/attach`, `/clear-attachments`, `/model`, `/url`.
- Conversation checkpoint save/restore flow (branch-style restore into a new conversation).
- Conversation and workspace export/import flows (JSON and Markdown conversation export).
- Image attachment pipeline (base64 payloads on outgoing user messages for multimodal-capable models).
- Tool-call capture and visualization in chat output.
- Configurable keybinding layer (including command autocomplete binding).
- Phase 4 integration tests for checkpoint restore and workspace export/import roundtrip.

### Changed
- Chat message schema/storage now carries optional image and tool-call metadata.
- Input handling now supports command autocomplete on slash input and config-driven bindings.
- TUI status/help output now reflects attachments and command-centric power workflows.

## [0.0.3] - 2026-03-29

### Added
- Polished TUI with WhatsApp-inspired visual theme, role-based chat bubbles, avatars, and message timestamps.
- Status bar details for workspace, model, connection status, and loading state.
- Help overlay (`?`) and keyboard UX improvements for focus switching (`Tab`) and dismiss behavior (`Esc`).
- Toast notifications for info/success/error feedback.
- Smooth chat scrolling controls (`Up/Down`, `PgUp/PgDn`) and responsive layout behavior for narrower terminals.
- `syntect`-based language-aware syntax highlighting for fenced code blocks in assistant messages.
- Config toggle for highlighting (`[ui] syntax_highlighting = true`) with backward-compatible config defaults.
- New config tests for UI highlighting defaults and missing-section fallback behavior.

### Changed
- Markdown rendering now supports styled inline formatting (lists, bold, italics, inline code).
- Code block rendering now uses syntax-highlighted output when enabled, with safe plain-styled fallback when disabled or unavailable.

## [0.0.2] - 2026-03-29

### Added
- SQLite-backed persistence layer with migrations for presets, workspaces, conversations, messages, and context files.
- Full-text search (FTS5) with insert/update/delete triggers for message indexing.
- Workspace manager for default bootstrap, workspace CRUD, conversation lifecycle, and message persistence.
- Config module with first-run config creation and platform-aware config/data paths.
- History loading on startup and automatic user/assistant message persistence during chat.
- Context file metadata support (hashing and change detection).
- Deterministic persistence test fixtures and real CRUD/FTS/hash-change tests.

### Changed
- Runtime now loads settings from config by default, with CLI URL/model overrides.
- App startup now restores previous conversation state from the database.

## [0.0.1] - 2026-03-29

### Added
- Initial Rust crate scaffold with `Cargo.toml`, `src/`, and basic project setup.
- Ratatui + Crossterm terminal UI loop with model panel, chat panel, and input box.
- Input behavior and keybindings (`Ctrl+Enter` send, `Enter` newline, `Ctrl+Q` quit).
- Ollama integration for model listing and chat requests.
- Streaming NDJSON parsing and token-by-token live rendering in the TUI.
- Early quality gates: rust toolchain pinning, formatter/linter configs, CI workflow, and check scripts.
- Test suite scaffold and MVP-critical tests (stream parsing, keybindings, startup smoke, model cache, API error mapping, context rules).

### Fixed
- FTS schema linkage corrected to use `content=messages`.
