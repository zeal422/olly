# Test Coverage Implementation Plan

Date: 2026-03-29
Scope: Improve runtime safety and regression resistance by adding missing high-risk tests.

## Goal

Raise confidence in command execution, workspace/session lifecycle behavior, streaming behavior, config persistence, and TUI helper logic.

This plan is implementation-oriented and intended to be executed as written.

## Execution Order

1. Phase 1: P1 command-flow tests
2. Phase 2: P1 streaming success-path tests
3. Phase 3: P1 workspace/session edge-case tests
4. Phase 4: P2 import/export and root-path normalization tests
5. Phase 5: P2 config create/load/save tests
6. Phase 6: P3 TUI helper regression tests
7. Final: full validation gate

## Phase 1: Command-Flow Tests (P1)

Target file:
- `src/app/mod.rs` (append to existing `#[cfg(test)] mod tests`)

Planned tests:
- `/workspace create --path <folder>` uses default naming (`New Workspace`, `New Workspace 2`).
- `/workspace rename <new name>` rejects duplicate names.
- `/workspace delete` selects sidebar/active workspace and opens confirmation flow correctly.
- `/session create` sets active session and loads empty conversation state.
- `/session delete` deletes active session and falls back to remaining session or "No Session" state.
- `/session rename` supports both `id + new title` and `<title> --to <new title>` paths.
- `/model <name>` behavior for exact match, prefix match, and unknown model.
- `/url <...>` updates session URL state.

Acceptance criteria:
- New tests pass deterministically without network dependency.
- Existing autocomplete/unit tests continue to pass.

## Phase 2: Streaming Success-Path Tests (P1)

Target files:
- `tests/streaming_success.rs` (new)
- `tests/common/mock_ollama.rs` (extend helper if needed)

Planned tests:
- `chat_stream` handles multiple NDJSON lines in one chunk.
- `chat_stream` handles one NDJSON line split across multiple chunks.
- `chat_stream` stops callback processing once `done=true` is received.
- `chat_stream` processes trailing buffered line when stream ends without final newline.

Acceptance criteria:
- Callback order and accumulated content assertions are exact.
- No flaky timing assumptions.

## Phase 3: Workspace/Session Edge Cases (P1)

Target files:
- `tests/workspace_sessions.rs` (new)
- `tests/persistence_crud.rs` (extend)
- `src/app/mod.rs` (extend existing command-flow tests for UI/session fallback semantics)

Planned tests:
- Creating and deleting sessions keeps conversation ordering and active-session fallback stable.
- Deleting last session transitions to zero-session-safe state semantics (validated through app command path in `src/app/mod.rs` tests).
- Renaming non-existent session/workspace returns `NotFound`.
- Opening a non-existent workspace session returns `NotFound`.

Acceptance criteria:
- Error type assertions are explicit (not string-only where avoidable).
- All DB side effects are asserted after each mutation.

## Phase 4: Import/Export + Root Path Edge Tests (P2)

Target file:
- `tests/workspace_import_export_edges.rs` (new)

Planned tests:
- Import dedupes workspace names when target name already exists.
- Import with invalid/non-directory `root_path` does not crash and safely falls back.
- `set_workspace_root_path` rejects missing path and non-directory path.
- Export/import roundtrip preserves `images` and `tool_calls` metadata.

Acceptance criteria:
- Roundtrip checks include message metadata, not only message content.
- Invalid-path behavior is verified with concrete assertions.

## Phase 5: Config Persistence Tests (P2)

Target file:
- `tests/config_persistence.rs` (new)

Planned tests:
- `save_to_path` creates parent config/data directories when missing.
- `save_to_path` persists changed keybindings/UI settings and reload matches exactly.
- Partial config TOML loads with defaults for missing sections.
- Invalid TOML returns decode error.
- `default_config_dir`/`default_data_dir` path helpers return non-empty deterministic `.../olly` suffixes.

Acceptance criteria:
- Uses isolated temp directories only.
- No reliance on user global config path or mutation of real user config.

## Phase 6: TUI Helper Regression Tests (P3)

Target file:
- `src/app/tui/mod.rs` (add `#[cfg(test)]` tests near helper functions)

Planned tests:
- `truncate_middle` and `truncate_end` preserve expected lengths and marker placement.
- `scroll_window` and marquee behavior produce stable output windows.
- `sidebar_action_hint` returns expected hint text by selected sidebar item type.

Acceptance criteria:
- String assertions are exact and stable.
- Tests do not require terminal runtime initialization.

## Final Validation Gate

Run in order:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build
```

Optional on-demand:

```powershell
./scripts/build-installer.ps1 -Version 0.0.6
```

## Definition of Done

- All phases completed with committed tests.
- No existing test regressions.
- Full validation gate passes.
- Any new helpers in `tests/common/` documented inline.
