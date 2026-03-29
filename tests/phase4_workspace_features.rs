mod common;

#[test]
fn checkpoint_restore_creates_new_conversation_branch() {
    let db = common::temp_db::TempDb::new("phase4-checkpoint");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Phase4 WS", "llama3.2:latest")
        .expect("bootstrap");

    manager
        .save_message(session.conversation_id, "user", "alpha")
        .expect("save alpha");
    manager
        .save_message(session.conversation_id, "assistant", "beta")
        .expect("save beta");
    manager
        .save_checkpoint(session.conversation_id, "cp1")
        .expect("save checkpoint");

    let new_conversation_id = manager
        .restore_checkpoint(session.workspace_id, session.conversation_id, "cp1")
        .expect("restore checkpoint");
    assert_ne!(new_conversation_id, session.conversation_id);

    let restored = manager
        .load_messages(new_conversation_id)
        .expect("load restored messages");
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0].content, "alpha");
    assert_eq!(restored[1].content, "beta");
}

#[test]
fn workspace_export_and_import_roundtrip() {
    let db = common::temp_db::TempDb::new("phase4-export-import");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Export WS", "llama3.2:latest")
        .expect("bootstrap");
    manager
        .save_message(session.conversation_id, "user", "hello export")
        .expect("save message");

    let export_path = db.path().with_file_name("workspace-export.json");
    manager
        .export_workspace_json(session.workspace_id, &export_path)
        .expect("export workspace");

    let imported = manager
        .import_workspace_json(&export_path, "llama3.2:latest")
        .expect("import workspace");
    let imported_messages = manager
        .load_messages(imported.conversation_id)
        .expect("load imported messages");
    assert!(imported_messages
        .iter()
        .any(|m| m.content.contains("hello export")));
}
