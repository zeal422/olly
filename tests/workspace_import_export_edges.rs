mod common;

use std::fs;

use olly::workspace::error::WorkspaceError;

#[test]
fn import_dedupes_workspace_name_when_name_already_exists() {
    let db = common::temp_db::TempDb::new("workspace-import-dedupe");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Export Workspace", "llama3.2:latest")
        .expect("bootstrap");

    let export_path = db.path().with_file_name("dedupe-export.json");
    manager
        .export_workspace_json(session.workspace_id, &export_path)
        .expect("export workspace");
    manager
        .import_workspace_json(&export_path, "llama3.2:latest")
        .expect("import workspace");

    let names = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .map(|workspace| workspace.name)
        .collect::<Vec<_>>();
    assert!(names.iter().any(|name| name == "Export Workspace"));
    assert!(names.iter().any(|name| name == "Export Workspace (2)"));
}

#[test]
fn import_with_invalid_root_path_falls_back_safely() {
    let db = common::temp_db::TempDb::new("workspace-import-invalid-root");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let import_path = db.path().with_file_name("invalid-root-import.json");
    let payload = r#"{
  "version": 1,
  "name": "Workspace With Invalid Root",
  "root_path": "C:/definitely/not/a/real/folder/for/olly/tests",
  "context_paths": [],
  "conversations": [
    {
      "title": "Main Conversation",
      "messages": [
        {
          "role": "user",
          "content": "hello",
          "timestamp": "2026-03-29 00:00:00",
          "images": null,
          "tool_calls": null
        }
      ]
    }
  ]
}"#;
    fs::write(&import_path, payload).expect("write import payload");

    let imported = manager
        .import_workspace_json(&import_path, "llama3.2:latest")
        .expect("import workspace");
    let imported_workspace = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .find(|workspace| workspace.id == imported.workspace_id)
        .expect("imported workspace exists");
    assert_eq!(imported_workspace.name, "Workspace With Invalid Root");
    assert!(imported_workspace.root_path.is_none());
}

#[test]
fn set_workspace_root_path_rejects_missing_or_non_directory_paths() {
    let db = common::temp_db::TempDb::new("workspace-set-path-errors");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Root Path Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let root = db
        .path()
        .parent()
        .expect("temp db parent should exist")
        .to_path_buf();

    let missing = root.join("missing-folder");
    let missing_err = manager
        .set_workspace_root_path(session.workspace_id, &missing)
        .expect_err("missing path should fail");
    assert!(matches!(missing_err, WorkspaceError::NotFound(_)));

    let file_path = root.join("not-a-directory.txt");
    fs::write(&file_path, "content").expect("write test file");
    let file_err = manager
        .set_workspace_root_path(session.workspace_id, &file_path)
        .expect_err("file path should fail");
    assert!(matches!(file_err, WorkspaceError::NotFound(_)));
}

#[test]
fn workspace_export_import_roundtrip_preserves_images_and_tool_calls() {
    let db = common::temp_db::TempDb::new("workspace-export-import-meta");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Meta Workspace", "llama3.2:latest")
        .expect("bootstrap");

    let images = r#"["image-a-base64","image-b-base64"]"#;
    let tool_calls = r#"[{"function":{"name":"search","arguments":{"q":"olly"}}}]"#;
    manager
        .save_message_with_meta(
            session.conversation_id,
            "assistant",
            "meta message",
            Some(images),
            Some(tool_calls),
        )
        .expect("save message with metadata");

    let export_path = db.path().with_file_name("workspace-meta-export.json");
    manager
        .export_workspace_json(session.workspace_id, &export_path)
        .expect("export workspace");

    let imported = manager
        .import_workspace_json(&export_path, "llama3.2:latest")
        .expect("import workspace");
    let imported_messages = manager
        .load_messages(imported.conversation_id)
        .expect("load imported messages");
    let restored = imported_messages
        .iter()
        .find(|message| message.content == "meta message")
        .expect("meta message should exist after import");
    assert_eq!(restored.images.as_deref(), Some(images));
    assert_eq!(restored.tool_calls.as_deref(), Some(tool_calls));
}
