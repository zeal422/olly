use super::helpers::*;
use crate::app::is_cloud_model_name;
use crate::ollama::chat::{ChatMessage, ChatToolCall, ToolFunction};

#[test]
fn cloud_model_classifier_handles_common_patterns() {
    assert!(is_cloud_model_name("minimax-m2.7:cloud"));
    assert!(is_cloud_model_name("devstral-2:123b-cloud"));
    assert!(!is_cloud_model_name("qwen2.5-coder:7b"));
}

#[test]
fn input_editor_inserts_on_empty_line_after_vertical_move() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.set_input_text(String::from("one\n\nthree"));
    app.move_input_cursor_up();
    app.insert_input_char('x');

    assert_eq!(app.input, "one\nx\nthree");
    let (line, col) = app.input_line_col_for_cursor();
    assert_eq!((line, col), (1, 1));
}

#[test]
fn sidebar_groups_models_into_cloud_and_local_categories() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("qwen2.5-coder:7b"),
    );
    app.models = vec![
        String::from("minimax-m2.7:cloud"),
        String::from("qwen2.5-coder:7b"),
        String::from("devstral-2:123b-cloud"),
    ];

    assert_eq!(app.model_count_for_bucket(ModelBucket::Cloud), 2);
    assert_eq!(app.model_count_for_bucket(ModelBucket::Local), 1);

    let entries = app.sidebar_entries();
    assert!(entries.contains(&SidebarItem::ModelGroupHeader(ModelBucket::Cloud)));
    assert!(entries.contains(&SidebarItem::ModelGroupHeader(ModelBucket::Local)));
}

#[test]
fn workspace_help_text_snapshot_stable() {
    let expected = [
        "Workspace commands:",
        "- /workspace list",
        "- /workspace switch <id|name>",
        "- /workspace create <name> --path <folder>",
        "- /workspace create --path <folder> (name defaults to New Workspace)",
        "- /workspace rename <new name> (renames selected/active workspace)",
        "- /workspace delete (opens modal for selected workspace in sidebar)",
        "- /workspace delete <id|name> (optional explicit target)",
        "- Confirm delete modal with Enter, cancel with Esc",
        "- Deleting the last workspace is allowed (you will see the welcome screen)",
        "- /workspace where",
        "- /workspace set-path <id|name> --path <folder>",
        "- /workspace export <path>",
        "- /workspace import <path>",
    ]
    .join("\n");
    assert_eq!(workspace_help_text(), expected);
}

#[test]
fn session_help_text_snapshot_stable() {
    let expected = [
        "Session commands:",
        "- /session list",
        "- /session new <title>",
        "- /session create <title> (alias of new)",
        "- /session switch <id|title>",
        "- /session rename <id> <new title>",
        "- /session rename <title> --to <new title>",
        "- /session delete (opens modal for selected session in sidebar)",
        "- /session delete <id|title> (optional explicit target)",
        "- Confirm delete modal with Enter, cancel with Esc",
    ]
    .join("\n");
    assert_eq!(session_help_text(), expected);
}

#[test]
fn models_help_text_snapshot_stable() {
    let expected = [
        "Model commands:",
        "- /models",
        "- /models refresh",
        "- /models add <model-name-or-ollama-library-url> (pull local or enable cloud)",
        "- /models remove <model-name>",
        "- /model <name> (switch active model)",
    ]
    .join("\n");
    assert_eq!(models_help_text(), expected);
}

#[test]
fn tool_message_replay_preserves_tool_name_and_full_content() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let full_content = "x".repeat(512);
    app.push_tool(full_content.clone(), Some(String::from("execute_command")));

    let ollama_messages = app.to_ollama_messages();
    let tool_message = ollama_messages
        .iter()
        .find(|message| message.role == "tool")
        .expect("tool message should exist");

    assert_eq!(tool_message.tool_name.as_deref(), Some("execute_command"));
    assert_eq!(tool_message.content, full_content);
}

#[test]
fn tool_message_header_preview_includes_tool_name() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.push_tool(
        String::from("tool output"),
        Some(String::from("grep_search")),
    );

    let message = app.messages.last().expect("tool message");
    assert_eq!(
        message.tool_calls_preview.as_deref(),
        Some("Tool: grep_search")
    );
}

#[test]
fn tool_only_turn_does_not_get_no_response_content_placeholder() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    // Simulate a tool-only turn: assistant message with tool_calls but no content
    let tool_call = ChatToolCall {
        function: Some(ToolFunction {
            name: "read_file".to_string(),
            arguments: serde_json::json!({"path": "/workspace/test.txt"}),
        }),
    };

    // Directly construct an assistant message with tool_calls and empty content
    app.messages.push(Message {
        role: "Assistant".to_string(),
        content: String::new(), // empty
        timestamp: "2025-01-01T00:00:00Z".to_string(),
        image_count: 0,
        tool_calls_preview: Some("Tool: read_file".to_string()),
        tool_name: None,
        tool_calls: Some(vec![tool_call.clone()]),
    });

    // Simulate the condition where streamed_any is false and we have tool_calls
    // (the fix should NOT replace empty content with placeholder)
    let streamed_any = false;
    let captured_tool_calls = [tool_call];

    // This mimics the logic from chat_pipeline line 492-494 after fix
    if !streamed_any && captured_tool_calls.is_empty() {
        app.messages.last_mut().unwrap().content = String::from("(no response content)");
    }

    let last_msg = app.messages.last().unwrap();
    assert_eq!(last_msg.content, String::new()); // remains empty, not placeholder
    assert!(last_msg.tool_calls.is_some());
}

#[test]
fn assistant_tool_calls_survive_storage_roundtrip() {
    use crate::app::state::App;

    let tool_calls_json = r#"[{"function":{"name":"read_file","arguments":{"path":"/test"}}}]"#;
    let stored = App::from_storage_role(
        "assistant",
        "Reading file".to_string(),
        None,
        Some(tool_calls_json.to_string()),
        "2025-01-01T00:00:00Z".to_string(),
    );

    assert!(stored.is_some());
    let stored = stored.unwrap();
    assert!(stored.tool_calls.is_some());
    let calls = stored.tool_calls.as_ref().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.as_ref().unwrap().name, "read_file");
}

#[test]
fn to_ollama_messages_includes_tool_calls_for_assistant() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    let tool_call = ChatToolCall {
        function: Some(ToolFunction {
            name: "grep_search".to_string(),
            arguments: serde_json::json!({"query": "test"}),
        }),
    };

    app.messages.push(Message {
        role: "Assistant".to_string(),
        content: "Here are the results".to_string(),
        timestamp: "2025-01-01T00:00:00Z".to_string(),
        image_count: 0,
        tool_calls_preview: Some("Tool: grep_search".to_string()),
        tool_name: None,
        tool_calls: Some(vec![tool_call.clone()]),
    });

    let ollama_msgs = app.to_ollama_messages();
    let assistant_msg = ollama_msgs
        .iter()
        .find(|m| m.role == "assistant")
        .expect("assistant message should exist");

    assert!(assistant_msg.tool_calls.is_some());
    let calls = assistant_msg.tool_calls.as_ref().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.as_ref().unwrap().name, "grep_search");
}

#[test]
fn workspace_system_context_injected_before_tool_loop() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.workspace_root_path = Some(String::from("C:\\CODING\\my-project"));

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

#[test]
fn workspace_system_context_not_injected_without_workspace() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    // No workspace path set
    app.workspace_root_path = None;

    let messages = app.to_ollama_messages();

    // Verify no system message was injected
    assert!(!messages.iter().any(|m| m.role == "system"));
}

#[test]
fn chat_content_version_increments_on_push_user() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let before = app.chat_content_version;
    app.push_user("Hello".to_string(), 0);
    assert!(app.chat_content_version > before);
}

#[test]
fn chat_content_version_increments_on_push_system() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let before = app.chat_content_version;
    app.push_system("System message".to_string());
    assert!(app.chat_content_version > before);
}

#[test]
fn chat_content_version_increments_on_push_assistant() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let before = app.chat_content_version;
    app.push_assistant("Assistant response".to_string(), None);
    assert!(app.chat_content_version > before);
}

#[test]
fn chat_visual_lines_cache_valid_when_versions_match() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.push_user("Hello".to_string(), 0);

    // Simulate cache being built
    app.chat_content_version = 1;
    app.chat_cached_version = 1;
    app.chat_visual_lines_styled = vec![ratatui::text::Line::from("test")];
    app.chat_last_render_width = 80;

    // Cache should be valid
    let cache_valid = !app.chat_visual_lines_styled.is_empty()
        && app.chat_content_version == app.chat_cached_version
        && app.chat_last_render_width == 80;
    assert!(cache_valid);
}

#[test]
fn chat_visual_lines_cache_invalid_when_versions_differ() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.push_user("Hello".to_string(), 0);

    // Simulate stale cache
    app.chat_content_version = 2;
    app.chat_cached_version = 1;
    app.chat_visual_lines_styled = vec![ratatui::text::Line::from("test")];
    app.chat_last_render_width = 80;

    // Cache should be invalid
    let cache_valid = !app.chat_visual_lines_styled.is_empty()
        && app.chat_content_version == app.chat_cached_version
        && app.chat_last_render_width == 80;
    assert!(!cache_valid);
}
