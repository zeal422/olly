# Plan: Add Workspace Context to System Prompt

## Problem

When using Olly with a workspace path configured, the AI model has no awareness of the working directory. Users report that asking "Do you know what folder you're in?" results in the model having no context about the workspace.

This is problematic because:
- The model cannot accurately answer where it's working
- When the model creates files via tools, it doesn't "know" where they'll be placed
- The model lacks essential environment context that coding agents need

## Research: How Others Do It

### OpenCode
```javascript
system.push(...(await SystemPrompt.environment())) 
// `  Working directory: ${Instance.directory}`  
// `  Platform: ${process.platform}`  
// Today's date: ${new Date().toDateString()}`
```

### Claude Code
```
<env>
Working directory: /tmp/claude-history-1760408587018-hnp437
Is directory a git repo: No
Platform: linux
OS Version: Linux 6.6.87.2-microsoft-standard-WSL2
Today's date: 2025-10-14
</env>
```

Both include the workspace/working directory as a **system message** sent with every request to the LLM.

## Solution

Include the workspace path as a system message prepended to the messages array sent to Ollama. This is **not** a chat message that can get lost - it's part of the system-level context that the model always sees.

## Implementation

### File: `src/app/chat_pipeline.rs`

**Step 1: Add import**

Add to imports at top of file (around line 8):
```rust
use chrono::Local;
```

**Step 2: Inject system context**

**Location:** After line 195 (`let mut request_messages...`), before line 236 (`let mut tool_loop_messages...`)

**Current code (lines 195-218):**
```rust
let mut request_messages: Vec<ChatMessage> = app.to_ollama_messages();
// ... (no system context with workspace path)
```

**Modified code:**
```rust
let mut request_messages: Vec<ChatMessage> = app.to_ollama_messages();

// Prepend system context with workspace information
if let Some(ref workspace_path) = app.workspace_root_path {
    let system_context = format!(
        "You are working in the workspace: {}\nPlatform: {}\nDate: {}",
        workspace_path,
        std::env::consts::OS,
        chrono::Local::now().format("%Y-%m-%d")
    );
    request_messages.insert(0, ChatMessage::new("system", system_context));
}
```

**Also update** the tool loop messages (line 236):
```rust
let mut tool_loop_messages = request_messages.clone();
```
This ensures the system context persists through the tool execution loop.

## Testing

### Unit Test: `src/app/mod_tests/regressions.rs`

**Step 1: Add import**

Update imports to include `ChatMessage`:
```rust
use crate::ollama::chat::{ChatMessage, ChatToolCall, ToolFunction};
```

**Step 2: Add test**

Add test to verify system message with workspace path is included:

```rust
#[test]
fn to_ollama_messages_includes_workspace_system_context() {
    let mut app = App::new("http://localhost:11434".to_string(), "test-model".to_string());
    app.workspace_root_path = Some("C:\\CODING\\my-project".to_string());
    
    // Simulate the system context injection
    let mut messages = app.to_ollama_messages();
    if let Some(ref workspace_path) = app.workspace_root_path {
        let system_context = format!(
            "You are working in the workspace: {}\nPlatform: {}\nDate: {}",
            workspace_path,
            std::env::consts::OS,
            chrono::Local::now().format("%Y-%m-%d")
        );
        messages.insert(0, ChatMessage::new("system", system_context));
    }
    
    // Verify system message is first
    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("C:\\CODING\\my-project"));
    assert!(messages[0].content.contains("Platform:"));
    assert!(messages[0].content.contains("Date:"));
}
```

### Manual Testing

1. Start Olly with a workspace path configured
2. Ask the model: "Do you know what folder you're in?"
3. Expected response: "Yes, I'm working in C:\CODING\my-project"
4. Ask: "Create a file called test.txt"
5. Verify file is created in the correct workspace directory

## Quality Gates

Run after implementation:
```bash
./scripts/check.ps1
```

This ensures:
- `cargo fmt --check` passes
- `cargo clippy -D warnings` passes
- `cargo build` succeeds
- `cargo test` passes (all 322 tests)

## Files Modified

- `src/app/chat_pipeline.rs` - Add system context injection
- `src/app/mod_tests/regressions.rs` - Add regression test

## Notes

- The system message is synthetic - it's not stored in the chat history
- It's prepended to every request, ensuring the model always has workspace context
- The implementation follows the same pattern as OpenCode and Claude Code
