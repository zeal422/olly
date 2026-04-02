use super::helpers::*;
use crate::app::is_cloud_model_name;

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
