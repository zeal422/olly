mod common;

use std::fs;

use olly::workspace::manager::WorkspaceManager;

#[test]
fn save_and_load_many_messages_preserves_full_count() {
    let db = common::temp_db::TempDb::new("large-many-messages");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Large Workspace", "llama3.2:latest")
        .expect("bootstrap");

    for idx in 0..220 {
        manager
            .save_message(
                session.conversation_id,
                "user",
                &format!("bulk message {idx}"),
            )
            .expect("save message");
    }

    let loaded = manager
        .load_messages(session.conversation_id)
        .expect("load messages");
    assert_eq!(loaded.len(), 220);
    assert_eq!(loaded[0].content, "bulk message 0");
    assert_eq!(loaded[219].content, "bulk message 219");
}

#[test]
fn export_large_workspace_json_is_valid_and_parseable() {
    let db = common::temp_db::TempDb::new("large-export-json");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Export Large Workspace", "llama3.2:latest")
        .expect("bootstrap");

    for idx in 0..160 {
        manager
            .save_message(
                session.conversation_id,
                "assistant",
                &format!("export payload {idx}"),
            )
            .expect("save message");
    }

    let output = db.path().with_file_name("large-workspace.json");
    manager
        .export_workspace_json(session.workspace_id, &output)
        .expect("export workspace");
    let raw = fs::read_to_string(&output).expect("read export");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse exported json");
    let conversations = parsed["conversations"]
        .as_array()
        .expect("conversations array");
    assert!(!conversations.is_empty());
    let message_count = conversations[0]["messages"]
        .as_array()
        .expect("messages array")
        .len();
    assert_eq!(message_count, 160);
}

#[test]
fn import_large_workspace_json_preserves_message_count_exactly() {
    let db = common::temp_db::TempDb::new("large-import-json");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let payload_path = db.path().with_file_name("import-large.json");

    let mut message_entries = Vec::new();
    for idx in 0..180 {
        message_entries.push(serde_json::json!({
            "role": if idx % 2 == 0 { "user" } else { "assistant" },
            "content": format!("import line {idx}"),
            "timestamp": "2026-03-29 00:00:00",
            "images": serde_json::Value::Null,
            "tool_calls": serde_json::Value::Null
        }));
    }
    let payload = serde_json::json!({
        "version": 1,
        "name": "Imported Large Workspace",
        "root_path": serde_json::Value::Null,
        "context_paths": [],
        "conversations": [{
            "title": "Main Conversation",
            "messages": message_entries
        }]
    });
    fs::write(
        &payload_path,
        serde_json::to_string_pretty(&payload).expect("serialize payload"),
    )
    .expect("write import payload");

    let imported = manager
        .import_workspace_json(&payload_path, "llama3.2:latest")
        .expect("import workspace");
    let loaded = manager
        .load_messages(imported.conversation_id)
        .expect("load imported messages");
    assert_eq!(loaded.len(), 180);
    assert_eq!(loaded[0].content, "import line 0");
    assert_eq!(loaded[179].content, "import line 179");
}

#[test]
fn search_over_large_message_corpus_returns_expected_hits() {
    let db = common::temp_db::TempDb::new("large-search-corpus");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Search Large Workspace", "llama3.2:latest")
        .expect("bootstrap");

    for idx in 0..200 {
        let content = if idx % 25 == 0 {
            format!("criticaltoken marker {idx}")
        } else {
            format!("noise message {idx}")
        };
        manager
            .save_message(session.conversation_id, "assistant", &content)
            .expect("save message");
    }

    let hits = manager
        .search_messages("criticaltoken")
        .expect("search messages");
    assert_eq!(hits.len(), 8);
    assert!(hits.iter().all(|hit| hit.content.contains("criticaltoken")));
}
