mod common;

use olly::workspace::error::WorkspaceError;

#[test]
fn create_and_delete_sessions_preserve_expected_ordering() {
    let db = common::temp_db::TempDb::new("workspace-sessions-order");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Default Workspace", "llama3.2:latest")
        .expect("bootstrap");

    let second = manager
        .create_session(session.workspace_id, Some("Second"))
        .expect("create second");
    let third = manager
        .create_session(session.workspace_id, Some("Third"))
        .expect("create third");

    let before_delete = manager
        .list_conversations(session.workspace_id)
        .expect("list conversations");
    assert_eq!(
        before_delete
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![session.conversation_id, second, third]
    );

    manager
        .delete_session(second)
        .expect("delete second session");
    let after_second_delete = manager
        .list_conversations(session.workspace_id)
        .expect("list conversations after delete");
    assert_eq!(
        after_second_delete
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![session.conversation_id, third]
    );

    manager.delete_session(third).expect("delete third session");
    manager
        .delete_session(session.conversation_id)
        .expect("delete main session");
    let after_all_delete = manager
        .list_conversations(session.workspace_id)
        .expect("list conversations after all delete");
    assert!(after_all_delete.is_empty());
}

#[test]
fn rename_nonexistent_entities_returns_not_found() {
    let db = common::temp_db::TempDb::new("workspace-sessions-not-found");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");

    let workspace_err = manager
        .rename_workspace(99_999, "Renamed")
        .expect_err("renaming unknown workspace should fail");
    assert!(matches!(workspace_err, WorkspaceError::NotFound(_)));

    let session_err = manager
        .rename_session(88_888, "Renamed Session")
        .expect_err("renaming unknown session should fail");
    assert!(matches!(session_err, WorkspaceError::NotFound(_)));
}

#[test]
fn open_nonexistent_workspace_session_returns_not_found() {
    let db = common::temp_db::TempDb::new("workspace-open-not-found");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");

    let err = manager
        .open_workspace_session(77_777, "llama3.2:latest")
        .expect_err("opening unknown workspace should fail");
    assert!(matches!(err, WorkspaceError::NotFound(_)));
}
