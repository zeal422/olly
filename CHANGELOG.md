# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project follows Semantic Versioning.

## [0.0.12] - 2026-03-31

### Added
- Mouse-wheel chat scrolling support routed through the runtime input pipeline.
- Faster wheel step behavior for chat scrolling to improve navigation responsiveness.
- Comprehensive test coverage for scroll semantics, mouse-wheel routing, and TUI scroll calculation (27 new tests).
- High-risk test coverage: toasts, input editor, pending expiration, model switching (18 new tests).
- **Right-click paste support** for chat input and slash commands:
  - Cross-platform clipboard integration using copypasta library (Windows, macOS, Linux)
  - Focus-aware paste: works in chat input and slash command mode, ignored elsewhere
  - Silent error handling for clipboard access failures
  - Comprehensive test coverage for mouse event resolution and string insertion (11 new tests)

### Fixed
- Fixed chat scroll drift/overscroll behavior where repeated wheel-up at the top could accumulate hidden offset and delay visible scroll-down recovery.
- Fixed animation coupling where high mouse-event throughput could accelerate sidebar marquee animation.

### Modified
- Chat scroll state now uses bounded from-bottom semantics with viewport-aware clamping to keep wheel navigation consistent and prevent out-of-range offsets.
- Event-loop ticking now runs at a fixed interval to keep UI animation timing stable regardless of mouse movement frequency.

## [0.0.11] - 2026-03-30

### Added
- **Tool execution system** for coding workflows with Ollama models:
  - Built-in tools: `read_file`, `list_dir`, `grep_search`, `find_by_name`, `execute_command`
  - Tool registry with JSON schema definitions for Ollama API
  - Safety sandbox: workspace boundary enforcement for all file operations
  - Command allowlist: only `cargo`, `git`, `rg`, `python`, `node` permitted
  - Command denylist: blocks dangerous patterns (`rm`, `dd`, `sudo`, `mkfs`, force-push, etc.)
  - Confirmation modal flow: Enter to confirm, Esc to cancel when `policy=confirm`
  - Footer telemetry: shows `tools:on/off` and `policy:auto/confirm` in status bar
  - Compact chat notices for tool execution start/end
  - Max 4 tool steps per conversation turn to prevent runaway execution
  - Output truncation with `[truncated]` markers for large outputs
- `/tools` command group for tool management:
  - `/tools` - show status and available commands
  - `/tools enable|on` / `/tools disable|off` - toggle tool execution
  - `/tools policy auto|confirm` - set execution policy
  - `/tools list` - list available tools
  - `/tools allow <tool>` / `/tools deny <tool>` - manage allow/deny lists
  - `/tools reset` - clear allow/deny lists
- Tool settings persistence in UI runtime state (backward compatible with old state files)
- Integration tests for tool execution: `tools_executor`, `tools_command`, `tools_loop`, `tools_registry`
- Overlay routing tests for tool confirmation modal
- Tool safety regression tests for:
  - command-token hardening (`git.evil` rejection, path-like executable rejection)
  - recursive traversal boundary enforcement against symlink/junction escape paths

### Security
- Tools are **disabled by default** and require explicit enablement
- All file operations are sandboxed to the workspace root
- Command execution has strict allowlist/denylist controls
- Confirmation modal prevents accidental tool execution when policy is set to `confirm`
- Hardened executable-token validation for `execute_command`:
  - rejects path-like executable tokens (`/`, `\`, leading `.`)
  - allows only exact allowlist roots (or immediate ASCII-digit suffix variants)
  - strips only known executable wrappers (`.exe`, `.cmd`, `.bat`)
  - rejects permissive dotted suffix forms like `git.evil`

## [0.0.10] - 2026-03-30

### Added
- New `src/app/platform/mod.rs` module for platform-specific console/window behavior (maximize, scrollbar hiding, cell-grid snapping, and buffer/window synchronization).
- New command orchestration module `src/app/commands/execute.rs` to run slash-command execution flow outside `src/app/mod.rs`.
- New runtime confirm-delete module `src/app/runtime/confirm_delete.rs` for workspace/session delete confirmation execution logic.
- New extracted app test suite under `src/app/mod_tests/` (`mod.rs`, `helpers.rs`, `autocomplete.rs`, `workspace_session.rs`, `ui_runtime_state.rs`, `regressions.rs`), moving the large in-file tests out of `src/app/mod.rs`.

### Fixed
- Removed dead-code risk from legacy delete-confirm paths by routing all runtime/handler call sites through `runtime::confirm_delete`.
- Preserved slash-command behavior while relocating execution path from `app::mod` to `commands::execute`.

### Modified
- `src/app/mod.rs` was reduced from 3267 lines to 1455 lines by extracting platform helpers, command execution flow, delete-confirm execution logic, and the monolithic in-file test module.
- Runtime modules now call `app::platform` helpers directly for startup/resize console handling.
- Docs updated to reflect the new architecture modules (`README.md`, `AGENTS.md`).

## [0.0.9] - 2026-03-30

### Added
- Completed refactor architecture slices under `src/app/`:
  - command decomposition (`commands/parser`, `commands/dispatch`, `commands/handlers/*`),
  - runtime loop decomposition (`runtime/startup`, `runtime/event_loop`, `runtime/key_actions`, `runtime/overlays`, `runtime/shutdown`),
  - chat send/stream pipeline extraction (`chat_pipeline.rs`),
  - workspace/session domain helpers (`domain/workspace_session`, `domain/context_state`),
  - autocomplete provider split (`autocomplete/core` + `autocomplete/providers/*`),
  - TUI modal module ownership (`tui/modals.rs`) with shared confirm-delete body helper.
- New targeted refactor regression tests:
  - runtime scroll-route precedence tests (`runtime::key_actions`),
  - overlay interception and dismissal tests (`runtime::overlays`),
  - deterministic modal guard tests (`tui::modals`).
- Refactor baseline checklist doc: `docs/REFACTOR_BASELINE.md`.

### Fixed
- Clippy `question_mark` violations in autocomplete providers (`/session`, `/workspace`) so the refactor tree stays warning-clean under `-D warnings`.
- Overlay-path regressions by explicitly validating overlay consumption vs non-overlay routing in runtime tests.

### Modified
- `src/app/mod.rs` command handling now remains an orchestration adapter over dispatcher/handler modules.
- `run()` now delegates startup/loop/shutdown responsibilities to `runtime::*` modules.
- TUI draw path now delegates help/delete/theme modal rendering through `tui/modals` helpers.
- `REFACTOR_PLAN.md` is fully checked off and recorded as completed implementation work.

## [0.0.8] - 2026-03-30

### Added
- Session context usage tracking in the chat footer (`ctx:<used>/<max>`) so each active session shows real token budget progress against the model context window.
- Context-window metadata fetch/parsing support from Ollama model details, including robust fallbacks when provider metadata is incomplete.

### Fixed
- Session deletion UX now opens a confirmation modal for `/session delete` (Enter confirm, Esc cancel), matching workspace deletion safety behavior.
- `/session delete` now defaults to the sidebar-selected session target (with active-session fallback only when nothing is selected), reducing accidental deletes and command ambiguity.
- Fixed footer context usage restore after app restart so `ctx:<used>/<max>` no longer drops to `?` in matching session/model startup paths.

### Modified
- `/session delete` autocomplete now stays focused on the single modal-driven command flow instead of emitting multiple delete variants.
- Session-delete post-confirm refresh now updates workspace/session sidebar state immediately so deleted sessions disappear consistently from navigation.
- UI runtime state now persists active model and per-session context telemetry snapshot to keep footer context usage stable across restarts (`--model` CLI override still takes precedence).

## [0.0.7] - 2026-03-29

### Added
- High-risk test coverage expansion to a total of **160 tests** (up from 100, net +60).
- New migration compatibility suite: `tests/migration_matrix.rs` (10 tests) covering legacy schema repair, idempotent migration runs, trigger recreation, and post-migration CRUD/search/checkpoint smoke.
- New concurrency safety suite: `tests/concurrency_safety.rs` (8 tests) covering concurrent message/session operations, checkpoint upserts, context upserts, path races, and lock-contention behavior.
- New network reliability suite: `tests/network_reliability.rs` (10 tests) covering timeout mapping, abrupt disconnects, malformed stream chunks, slow-response budget handling, and callback sequence invariants.
- New parser robustness suite: `tests/streaming_parser_robustness.rs` (8 tests) covering unknown fields, partial tool-call payloads, unicode/large chunks, mixed newline handling, blank-line interleaving, and bounded malformed-input no-panic checks.
- New path security suite: `tests/path_security_edges.rs` (6 tests) covering sensitive file blocking, traversal-like inputs, case-insensitive matching, slash/backslash normalization, and context side-effect assertions.
- New large payload boundary suite: `tests/large_payload_boundaries.rs` (4 tests) covering high-volume message persistence, large workspace export/import integrity, and large-corpus search behavior.
- Additional quoted-command parsing tests in `src/app/mod.rs` for `/workspace create`, `/workspace set-path`, `/export`, and `/session rename --to`.

### Fixed
- Legacy DB compatibility gap for `messages` metadata by auto-adding missing `images` and `tool_calls` columns during migration.
- Concurrent DB write stability by applying SQLite busy-timeout handling instead of immediate lock failure in normal contention windows.
- Windows console startup rendering bug that left a persistent right-side scrollbar gutter until manual unmaximize/maximize.
- Windows resize instability that could corrupt chat layout when shrinking the window.

### Modified
- Slash-command argument handling now strips surrounding quotes for path/name/title values in key command flows (`/workspace create`, `/workspace set-path`, `/export`, `/session rename ... --to ...`).
- Mock Ollama test harness was extended with delayed-response and abrupt-disconnect server modes to support deterministic network-failure coverage.
- Windows TUI sizing pipeline now uses deterministic startup normalization (`maximize -> snap to cell grid -> hide native scrollbars -> sync buffer`) and safe runtime resize handling (`terminal.autoresize` + `terminal.clear`).

## [0.0.6] - 2026-03-29

### Added
- Session rename command support: `/session rename <id> <new title>` and `/session rename <title> --to <new title>`.
- Session lifecycle commands: `/session create <title>` (alias of `new`) and `/session delete [id|title]` with selected/active default targeting.
- Workspace root-path persistence (`root_path`) with DB migration support for existing installations.
- New workspace path commands: `/workspace where` and `/workspace set-path <id|name> --path <folder>`.
- Workspace rename command: `/workspace rename <new name>`, targeting selected workspace in sidebar (or active workspace fallback) without requiring IDs.
- Filesystem-aware path suggestions for workspace commands that use `--path`.
- Sidebar nesting of sessions under their respective workspace, with per-workspace session collapse/expand behavior.
- Sidebar model grouping under `Cloud` and `Local` subcategories with independent collapse/expand behavior.
- Theme system with 8 built-in color palettes (including 2 light themes), plus `/themes` modal with live preview while navigating options.
- Persistent UI state storage for sidebar preferences across restarts (models/workspaces/group collapse + collapsed workspace list + sidebar selection index).
- Chat footer context strip (inside chat panel) showing active session/workspace context without overlapping message content.
- Delete confirmation modal UX for workspaces (Enter confirm, Esc cancel).
- Welcome screen in chat area when no workspace is active, including quick-start guidance.
- Model management commands: `/models add <model-name-or-ollama-library-url>` and `/models remove <model-name>`.
- Windows installer support via Inno Setup (`installer/olly.iss`) plus build script (`scripts/build-installer.ps1`) producing `Olly-Setup-<version>.exe`.

### Fixed
- Resolved sidebar/input conflict so command suggestion navigation and sidebar navigation no longer fight for `Up/Down` behavior.
- Prevented accidental command auto-selection from arrow movement; command selection now applies only when explicitly accepted.
- Fixed slash-command usability while sidebar-focused by allowing slash-command typing and execution without forcing focus switch.
- Fixed ambiguous session visibility by making session hierarchy explicit under each workspace in the sidebar.
- Fixed inability to delete the last workspace by supporting a safe zero-workspace state.
- Fixed command dismissal UX so `Esc` clears typed slash commands directly from input.
- Fixed truncated selected-item readability on smaller terminals via marquee-style horizontal scrolling in the sidebar.

### Modified
- Workspace creation defaults now use human-friendly auto names (`New Workspace`, `New Workspace 2`, ... ) when no name is provided.
- Sidebar presentation was refined to improve readability on smaller screens and reduce redundant path clutter.
- Workspace delete flow now defaults to currently selected workspace context, with optional explicit target still supported.
- Startup/session state handling now supports a no-active-workspace mode and gracefully restores active context when available.
- Model command wording now differentiates local vs cloud behavior (`pull local or enable cloud`) and uses "enabled" messaging in toasts/help text.
- README command documentation was updated to reflect new workspace/session behaviors and modal confirmation flow.
- Release/installation documentation now includes Windows installer generation and artifact expectations.
- Licensing was consolidated to a single proprietary VectorMedia license (`LICENSE`) and dual-license references were removed.

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
