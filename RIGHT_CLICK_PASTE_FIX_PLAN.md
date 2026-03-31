# Right-Click Paste Fix Plan

## Root Cause
Right-click paste stopped working because the mouse event handling in `src/app/input.rs` only processes scroll events (`ScrollUp`/`ScrollDown`) and ignores all other mouse events, including `Down(MouseButton::Right)`.

## Required Changes

### 1. Add Clipboard Dependency
- Add `copypasta = "0.10"` to `[dependencies]` in `Cargo.toml`
- Cross-platform clipboard support (Windows, macOS, Linux)
- **Note**: copypasta has minimal dependencies (~0-9.5MB) and is well-maintained

### 2. Extend Input Action System
- Add `Paste` variant to `InputAction` enum in `src/app/input.rs`
- Update `resolve_mouse_event` to handle `Down(MouseButton::Right)` → `InputAction::Paste`
- Need to import `MouseButton` from crossterm

### 3. Implement Paste Handler
- Add paste handling in `src/app/runtime/key_actions.rs` 
- Use clipboard crate to read content and insert into input field
- Handle focus state (only paste when chat input is focused)
- **Critical**: Must handle `Result<(), Box<dyn std::error::Error>>` return type

### 4. Add Test Coverage
- Test right-click mouse event resolution
- Test paste action handling with different focus states
- Test clipboard content insertion
- Add to `src/app/mod_tests/mod.rs`

## Files to Modify
1. `Cargo.toml` - Add clipboard dependency
2. `src/app/input.rs` - Add Paste action and mouse event handling  
3. `src/app/runtime/key_actions.rs` - Implement paste logic
4. `src/app/mod.rs` - **NEW**: Add `insert_input_string()` method
5. `src/app/mod_tests/mod.rs` - Add paste test module

## Infrastructure Compatibility
- ✅ Follows existing `InputAction` enum pattern
- ✅ Integrates with existing `handle_action` function signature
- ✅ Uses established focus handling with `FocusPane::Chat` and `is_slash_command_input()`
- ✅ Fits existing test structure in `mod_tests/`

## Critical Issues Found & Fixed

### ❌ Missing String Insert Method
**Issue**: The codebase only has `insert_input_char(ch: char)` but no method to insert strings at cursor position.

**Solution**: Need to add `insert_input_string(&mut self, text: String)` method to `src/app/mod.rs`.

### ✅ Error Handling Compatibility  
**Issue**: copypasta returns `Result<String, Box<dyn Error + Send + Sync + 'static>>` but `handle_action` returns `Result<(), Box<dyn std::error::Error>>`.

**Solution**: These are compatible - both use boxed error traits.

### ✅ Clipboard Context Creation
**Issue**: Need to handle clipboard context creation safely.

**Solution**: Use lazy static or create context on-demand with error handling.

## Implementation Notes
- Only allow paste when `FocusPane::Chat` or slash command input (consistent with text insertion)
- Handle empty clipboard gracefully (no-op)
- Preserve existing scroll behavior
- Add proper error handling for clipboard access failures
- Use same async pattern as other actions in `handle_action`
- **NEW**: Must implement `insert_input_string()` method for string insertion at cursor

## Done Criteria & Verification

### ✅ Functional Tests
1. **Right-click paste works**: Right-click in chat input inserts clipboard content at cursor position
2. **Focus handling**: Paste only works when `FocusPane::Chat` or in slash command mode
3. **Empty clipboard**: Right-click with empty clipboard does nothing (no error)
4. **Multi-line content**: Pasting multi-line text works correctly with cursor navigation
5. **Scroll preservation**: Mouse wheel scrolling continues working after paste implementation

### ✅ Test Suite
1. **Unit tests pass**: `cargo test` runs successfully with new paste tests
2. **Integration tests**: All existing tests still pass (no regressions)
3. **Coverage**: New tests cover mouse event resolution, paste handling, and edge cases

### ✅ Quality Gates
1. **Formatting**: `cargo fmt --all -- --check` passes
2. **Linting**: `cargo clippy --all-targets --all-features -- -D warnings` passes  
3. **Build**: `cargo build` succeeds
4. **Runtime**: App starts and runs without clipboard-related panics

### ✅ Cross-Platform Verification
1. **Windows**: Right-click paste works on Windows terminal
2. **Linux**: Paste works on terminal with X11/Wayland clipboard support
3. **Error handling**: Graceful fallback when clipboard unavailable
