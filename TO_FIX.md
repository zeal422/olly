# Bugs Found — Bug Hunt 2026-04-02

## BUG #1: `String::truncate()` panics on multi-byte UTF-8 boundaries (CRITICAL)

**Type:** Runtime Error (Severity: 10)
**File:** `src/app/tools/executor.rs` — lines 224, 337, 473, 655, 951
**Confidence:** 100%

`String::truncate(n)` panics if `n` is not a valid UTF-8 character boundary. When tool output exceeds `MAX_OUTPUT_SIZE` (10KB) and contains multi-byte characters (CJK, emojis, accented text), `truncate(MAX_OUTPUT_SIZE)` splits a character, and the subsequent `push_str()` panics.

```rust
// Buggy
if output.len() > MAX_OUTPUT_SIZE {
    output.truncate(MAX_OUTPUT_SIZE);  // PANICS on multi-byte boundary
    output.push_str("\n\n[Output truncated: exceeded size limit]");
}

// Fix
if output.len() > MAX_OUTPUT_SIZE {
    let mut truncate_at = MAX_OUTPUT_SIZE;
    while truncate_at > 0 && !output.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }
    output.truncate(truncate_at);
    output.push_str("\n\n[Output truncated: exceeded size limit]");
}
```

---

## BUG #2: Assistant `tool_calls` dropped when reloading conversation from storage

**Type:** Logical Error / Data Loss (Severity: 8)
**File:** `src/app/state.rs` — lines 490–511, 532–554, 141–149
**Confidence:** 90%

`Message` struct stores only `tool_calls_preview` (display string), not the structured `Vec<ChatToolCall>`. When `from_storage_role()` loads messages from SQLite, it parses the `tool_calls` JSON but discards the structured data. `to_ollama_messages()` then creates assistant messages with `tool_calls: None`, breaking the tool-call → tool-result chain required by the Ollama API.

```rust
// state.rs:507 — tool_calls always None
Some(ChatMessage::new(role, m.content.clone()))

// state.rs:532-537 — parsed data computed but dropped
let parsed_tool_calls = tool_calls
    .as_ref()
    .and_then(|raw| serde_json::from_str::<Vec<ChatToolCall>>(raw).ok());
let tool_preview = parsed_tool_calls
    .as_ref()
    .and_then(|calls| tool_calls_preview(calls));
// parsed_tool_calls dropped — only preview string survives
```

**Fix:** Add `tool_calls: Option<Vec<ChatToolCall>>` field to `Message`, populate it in `from_storage_role()`, and use it in `to_ollama_messages()`.

---

## BUG #3: Intermediate assistant messages with `tool_calls` not persisted to history

**Type:** Logical Error / Data Loss (Severity: 8)
**File:** `src/app/chat_pipeline.rs` — lines 381–486
**Confidence:** 90%

During multi-step tool execution, the assistant message containing `tool_calls` is added to `tool_loop_messages` (for the API) but never persisted to SQLite. Only the tool results are persisted. On reload, the conversation becomes `user → tool → tool → ... → assistant` instead of the required `user → assistant(tool_calls) → tool → assistant(tool_calls) → tool → ... → assistant`.

```rust
// line 390: added to API context but NOT persisted
tool_loop_messages.push(assistant_msg);

// lines 478-485: only tool result is persisted
let _ = history::append_message_with_meta(
    manager, session.conversation_id, "tool", &model_tool_content, ...
);
// Missing: persist assistant_msg with tool_calls before each tool result
```

**Fix:** Persist the assistant message (with `tool_calls`) to history before executing tools.

---

## BUG #4: `(no response content)` overwrites assistant message when LLM returns only tool calls

**Type:** Logical/UX Error (Severity: 5)
**File:** `src/app/chat_pipeline.rs` - lines 492-494
**Confidence:** 85%

When the LLM returns tool calls without text content, `streamed_any` stays `false` and the assistant message is overwritten with `"(no response content)"`.
This does **not** remove persisted tool-result messages, but it degrades assistant-turn semantics and can be confusing in replay/history.

```rust
// Current
if !streamed_any {
    app.messages[assistant_index].content = String::from("(no response content)");
}

// Better guard
if !streamed_any && captured_tool_calls.is_empty() {
    app.messages[assistant_index].content = String::from("(no response content)");
}
```

---

## BUG #5: Keybinding modifier matching uses subset instead of exact match

**Type:** Logical Error (Severity: 7)
**File:** `src/app/input.rs` — line 108
**Confidence:** 85%

`event.modifiers.contains(self.modifiers)` returns `true` when `self.modifiers` is `NONE` (all bits zero), meaning an unmodified binding matches any modified keypress. This causes `newline: "Enter"` to shadow `send: "Ctrl+Enter"` in custom configurations.

```rust
// Buggy
fn matches(&self, event: &KeyEvent) -> bool {
    event.code == self.code && event.modifiers.contains(self.modifiers)
}

// Fix
fn matches(&self, event: &KeyEvent) -> bool {
    event.code == self.code && event.modifiers == self.modifiers
}
```

---

## BUG #6: Wrong model used when creating workspace preset with CLI model override

**Type:** Semantic Error (Severity: 7)
**File:** `src/app/runtime/startup.rs` — line 75
**Confidence:** 85%

When `--model` is provided on the CLI, `startup_model` holds the override, but line 75 passes `config.defaults.model` to `open_workspace_session()`. Fresh workspaces get a preset with the wrong model.

```rust
let startup_model = model_override.unwrap_or_else(|| config.defaults.model.clone());
// ...
let next = manager.open_workspace_session(workspace.id, &config.defaults.model)?;
// Should be: &startup_model
```

**Fix:** Change line 75 to `&startup_model`.

---

## BUG #7: First imported conversation title silently discarded

**Type:** Logical Error (Severity: 6)
**File:** `src/workspace/manager.rs` — lines 420–439
**Confidence:** 80%

During workspace import, the first conversation reuses the pre-created conversation (titled "Main Conversation") and never updates its title to match the imported title.

**Fix:** After determining `conversation_id` for index 0, call `self.storage.rename_conversation(conversation_id, &conversation.title)`.

---

## BUG #8: `clean_path` drops excess `..` segments during normalization

**Type:** Path Normalization Error / Hardening Gap (Severity: 5)
**File:** `src/app/tools/executor.rs` - lines 87-89
**Confidence:** 75%

`stack.pop()` on an empty stack returns `None` silently, so leading/excess `..` components are discarded during normalization.
This is a correctness issue in normalization logic; treat it as a hardening gap (potential security risk), not a proven boundary-bypass by itself.

```rust
// Current
Component::ParentDir => {
    stack.pop();
}

// Better: preserve excess `..` components instead of dropping them
Component::ParentDir => {
    if !stack.is_empty() {
        stack.pop();
    } else {
        stack.push(std::ffi::OsString::from(".."));
    }
}
```

---

## BUG #9: `clean_path` root handling is incomplete on Windows path forms

**Type:** Path Normalization Error (Severity: 5)
**File:** `src/app/tools/executor.rs` - lines 91-94
**Confidence:** 75%

When `Component::RootDir` is encountered, the stack is cleared but root semantics are not explicitly represented in the reconstructed path.
This can produce incorrect normalization for some Windows path shapes (especially prefix/root combinations). The exact `C:\x -> C:x` transformation is not guaranteed for every case, but root handling is still fragile and should be made explicit.

```rust
// Current
Component::RootDir => {
    stack.clear();
}

// Better: preserve explicit root semantics
Component::RootDir => {
    stack.clear();
    // Keep root marker in the rebuilt path logic
    // (implementation detail depends on final path-building strategy)
}
```

---

## BUG #10: Migration ordering — FTS triggers reference columns before they exist on upgrade

**Type:** Logical Error (Severity: 6)
**File:** `src/workspace/storage.rs` — lines 27–117
**Confidence:** 75%

`execute_batch` creates FTS triggers that reference `images`/`tool_calls` columns before `ensure_message_metadata_columns()` runs. On upgrade from older schemas lacking these columns, trigger creation fails.

**Fix:** Move `ensure_workspace_root_path_column()` and `ensure_message_metadata_columns()` before the `execute_batch` call.

---

## Quick Wins (< 10 min)

1. Fix UTF-8 truncation panic in `executor.rs` (5 locations, same pattern)
2. Fix `input.rs` modifier matching: `contains` → `==`
3. Fix `startup.rs` line 75: `config.defaults.model` → `startup_model`
4. Fix `chat_pipeline.rs` line 492: add `&& captured_tool_calls.is_empty()` guard

## Moderate Fixes (30-60 min)

5. Add `tool_calls` field to `Message` struct and wire through `from_storage_role()` / `to_ollama_messages()`
6. Persist intermediate assistant messages with `tool_calls` in the tool loop
7. Fix `clean_path` for `..` on empty stack and missing root separator

## Strategic Fixes (1-2 hours)

8. Fix migration ordering in `storage.rs`
9. Fix first imported conversation title in `manager.rs`
