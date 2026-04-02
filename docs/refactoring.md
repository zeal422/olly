# Refactoring Guide

Module boundaries, extraction principles, and design decisions for Olly's codebase.

## Current Module Map

```
src/app/
├── mod.rs                 # ~325 lines - Coordinator/facade
├── state.rs               # ~1,061 lines - App struct + all methods
├── input_editor.rs        # ~140 lines - Cursor & text editing
├── chat_navigation.rs     # ~63 lines - Scroll, selection, auto-scroll
├── messages_and_conversation.rs  # ~74 lines - Message push ops
├── overlay_toast.rs       # ~58 lines - Toast, confirmations
├── theme_picker.rs        # ~53 lines - Theme picker state
├── input.rs               # ~321 lines - InputAction, bindings (unchanged)
├── chat_pipeline.rs       # ~631 lines - Send/stream/persist (unchanged)
├── context.rs             # ~39 lines - AppContext (unchanged)
├── mod_tests/             # 11 test files
├── autocomplete/          # Command suggestion
├── commands/              # Slash command handlers
├── domain/                # Sidebar helpers
├── platform/              # Windows console hardening
├── runtime/               # Event loop, key actions, overlays
├── tools/                 # Tool registry, executor
└── tui/
    ├── mod.rs             # ~765 lines - Draw orchestration + shared utils
    ├── palette.rs         # ~436 lines - ThemePalette
    ├── sidebar.rs         # ~318 lines - Sidebar + status bar
    ├── chat.rs            # ~344 lines - Chat panel
    ├── input.rs           # ~263 lines - Input panel
    ├── markdown.rs        # ~311 lines - Markdown + syntax
    └── modals.rs          # ~227 lines - Modal rendering
```

## Extraction Principles

### 1. Keep Impl Blocks Together

The `App` struct and its methods live in `state.rs` (~1,061 lines). This is intentional — fragmenting an impl block across multiple files makes navigation harder and increases merge conflicts.

**Rule**: If methods operate on the same struct, keep them together unless the group is large enough to be a standalone concern (e.g., `input_editor.rs` uses an `impl App` block but only for cursor/edit methods).

### 2. Shared Utilities Stay in Parent Module

Utilities like `manual_reflow`, `apply_selection_to_text`, and `truncate_*` remain in `tui/mod.rs` because they're used by multiple submodules (chat.rs, input.rs, sidebar.rs).

**Rule**: Don't extract shared utilities into new files if doing so would create circular dependencies or require re-exports that add indirection.

### 3. Prefer Existing Files Over New Ones

The TUI submodules (`palette.rs`, `sidebar.rs`, `chat.rs`, `input.rs`, `markdown.rs`) existed as thin forwarding wrappers before refactoring. The refactoring populated them with real implementations rather than creating new files.

**Rule**: Fill existing stubs before creating new modules.

### 4. Stable Re-Export Surface

`app/mod.rs` re-exports types from `state.rs` so external callers (`commands`, `runtime`, `chat_pipeline`, `tui`) don't need to change their imports.

**Rule**: When extracting code, maintain the existing import paths via `pub use` re-exports.

### 5. File-Level Ownership Comments

Each extracted module should have a doc comment at the top describing its responsibility:

```rust
//! Input cursor and text editing operations for the chat input panel.
//!
//! This module owns:
//! - Cursor movement (left/right/up/down)
//! - Character insertion and deletion
//! - Line/column calculation for multi-line input
//! - Preferred column persistence
//!
//! This module does NOT own:
//! - Input rendering (see tui/input.rs)
//! - Key binding resolution (see input.rs)
```

## Module Boundaries

### `state.rs` - App State
**Owns**: `App` struct, constructor, all methods that mutate state, UI state snapshot/restore, sidebar entry building, model/workspace/session management.

**Does NOT own**: Rendering, input resolution, command parsing, tool execution.

### `input_editor.rs` - Input Editing
**Owns**: Cursor movement, character insertion/deletion, line/column calculation, preferred column persistence.

**Does NOT own**: Input rendering, key binding resolution, command autocomplete.

### `chat_navigation.rs` - Scroll & Selection
**Owns**: Chat scroll up/down, mouse drag selection, Shift+click selection, drag-to-scroll auto-scroll, selection clearing.

**Does NOT own**: Chat rendering, text reflow, selection highlighting (those are in TUI).

### `tui/mod.rs` - Render Orchestration
**Owns**: `draw()` function, layout calculation, shared utilities (`manual_reflow`, `apply_selection_to_text`, `truncate_*`, `scroll_window`, `sidebar_action_hint`).

**Does NOT own**: Individual panel rendering (delegated to submodules), palette definitions, markdown parsing.

### `tui/chat.rs` - Chat Panel
**Owns**: `render_chat_panel`, message area layout, footer context strip.

**Does NOT own**: Message data (in `state.rs`), text reflow (in `tui/mod.rs`).

### `tui/sidebar.rs` - Sidebar & Status Bar
**Owns**: `render_models_panel`, `render_status_bar`, sidebar hints, marquee behavior.

**Does NOT own**: Sidebar entry building (in `state.rs`), domain types (in `domain/`).

## Why state.rs Is Large

`state.rs` is ~1,061 lines because it contains:
- `App` struct definition (35+ fields)
- Constructor and default state
- 40+ methods covering: UI state, session context, sidebar, models, workspaces, messages, quit handling, autocomplete, focus switching

Further splitting would require:
- Breaking the impl block into multiple files
- Creating wrapper types for field groups
- Adding indirection for method calls

The tradeoff is file size vs. navigability. Keeping all `App` methods together means contributors can find any state operation by searching `state.rs`.

## Why tui/mod.rs Retains Shared Utilities

The following functions remain in `tui/mod.rs`:
- `manual_reflow` - Used by chat.rs for text wrapping
- `apply_selection_to_text` - Used by chat.rs for selection highlighting
- `truncate_middle`, `truncate_end` - Used by sidebar.rs for text truncation
- `scroll_window` - Used by sidebar.rs for marquee
- `sidebar_action_hint` - Used by sidebar.rs for context hints
- `sidebar_line_with_marquee` - Used by sidebar.rs for animated text

Extracting these would require:
- Creating a `tui/utils.rs` module
- Re-exporting from `tui/mod.rs` for all consumers
- Adding indirection that doesn't improve clarity

## Adding New Features

### When to Create a New Module

Create a new module when:
- A cohesive set of functions/types doesn't fit existing modules
- The group is large enough (>50 lines) to justify a file
- It has a clear single responsibility

### When to Add to Existing Module

Add to existing module when:
- The functionality is closely related to existing code
- It's small (<50 lines) and doesn't warrant a file
- It's a method on an existing struct

### When to Use `impl App` in a New File

Use `impl App` in a new file when:
- The methods are a cohesive group (e.g., all cursor operations)
- They don't logically belong in `state.rs`'s existing method groups
- The group is large enough to improve navigation when separated

## Rollback Strategy

If a refactoring causes issues:
1. Identify the offending file change
2. Revert only that file (not the entire refactor)
3. Run `cargo check` to verify compilation
4. Run `cargo test` to verify behavior

Keep refactoring changes small and isolated to simplify bisect/debug.
