mod common;

#[test]
fn workspace_crud_and_message_persistence() {
    let db = common::temp_db::TempDb::new("crud");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");

    let session = manager
        .bootstrap_default("Default Workspace", "llama3.2:latest")
        .expect("bootstrap");
    manager
        .save_message(session.conversation_id, "user", "hello")
        .expect("save user message");
    manager
        .save_message(session.conversation_id, "assistant", "world")
        .expect("save assistant message");

    let messages = manager
        .load_messages(session.conversation_id)
        .expect("load messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "hello");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "world");

    let extra = manager
        .create_workspace("Side Project", "llama3.2:latest", None)
        .expect("create workspace");
    manager
        .rename_workspace(extra.workspace_id, "Renamed Project")
        .expect("rename workspace");
    let workspaces = manager.list_workspaces().expect("list workspaces");
    assert!(workspaces.iter().any(|w| w.id == extra.workspace_id));
    assert!(workspaces
        .iter()
        .any(|w| w.id == extra.workspace_id && w.name == "Renamed Project"));

    manager
        .delete_workspace(extra.workspace_id)
        .expect("delete workspace");
    let workspaces_after = manager.list_workspaces().expect("list workspaces after");
    assert!(!workspaces_after.iter().any(|w| w.id == extra.workspace_id));
}
