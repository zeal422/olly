mod common;

use std::fs;

use olly::workspace::error::WorkspaceError;

#[test]
fn import_malformed_json_returns_json_error() {
    let db = common::temp_db::TempDb::new("import-malformed-json");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let path = db.path().with_file_name("malformed.json");
    fs::write(&path, "{not-valid-json").expect("write malformed json");

    let err = manager
        .import_workspace_json(&path, "llama3.2:latest")
        .expect_err("import should fail");
    assert!(matches!(err, WorkspaceError::Json(_)));
}

#[test]
fn import_missing_required_fields_returns_json_error() {
    let db = common::temp_db::TempDb::new("import-missing-fields");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let path = db.path().with_file_name("missing-fields.json");
    fs::write(
        &path,
        r#"{
  "version": 1,
  "root_path": null,
  "context_paths": []
}"#,
    )
    .expect("write incomplete json payload");

    let err = manager
        .import_workspace_json(&path, "llama3.2:latest")
        .expect_err("import should fail");
    assert!(matches!(err, WorkspaceError::Json(_)));
}

#[test]
fn export_conversation_markdown_includes_images_and_tool_calls_sections() {
    let db = common::temp_db::TempDb::new("export-markdown-metadata");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Markdown Export Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let images = r#"["image-1","image-2"]"#;
    let tool_calls = r#"[{"function":{"name":"search","arguments":{"q":"olly"}}}]"#;
    manager
        .save_message_with_meta(
            session.conversation_id,
            "assistant",
            "response with metadata",
            Some(images),
            Some(tool_calls),
        )
        .expect("save message with metadata");

    let path = db.path().with_file_name("conversation.md");
    manager
        .export_conversation_markdown(session.conversation_id, &path)
        .expect("export markdown");
    let raw = fs::read_to_string(&path).expect("read markdown export");

    assert!(raw.contains("Images: [\"image-1\",\"image-2\"]"));
    assert!(raw.contains(
        "Tool calls: [{\"function\":{\"name\":\"search\",\"arguments\":{\"q\":\"olly\"}}}]"
    ));
}

#[test]
fn export_conversation_json_creates_missing_parent_directories() {
    let db = common::temp_db::TempDb::new("export-json-parent-dirs");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Json Export Workspace", "llama3.2:latest")
        .expect("bootstrap");

    let nested = db.path().with_file_name("exports").join("nested");
    let output_path = nested.join("conversation.json");
    assert!(!nested.exists());

    manager
        .export_conversation_json(session.conversation_id, &output_path)
        .expect("export conversation json");
    assert!(nested.exists());
    assert!(output_path.exists());
}
