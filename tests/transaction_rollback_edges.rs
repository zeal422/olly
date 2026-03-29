mod common;

use std::fs;

use olly::workspace::error::WorkspaceError;
use rusqlite::Connection;

fn scalar_count(db_path: &std::path::Path, sql: &str) -> i64 {
    let conn = Connection::open(db_path).expect("open sqlite connection");
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .expect("execute scalar query")
}

#[test]
fn import_missing_file_does_not_create_new_workspaces() {
    let db = common::temp_db::TempDb::new("tx-import-missing");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let before = manager
        .list_workspaces()
        .expect("list workspaces before")
        .len();

    let err = manager
        .import_workspace_json(
            &db.path().with_file_name("missing-import.json"),
            "llama3.2:latest",
        )
        .expect_err("missing import file should fail");
    assert!(matches!(err, WorkspaceError::Io(_)));

    let after = manager
        .list_workspaces()
        .expect("list workspaces after")
        .len();
    assert_eq!(after, before);
}

#[test]
fn import_malformed_json_does_not_create_new_workspaces() {
    let db = common::temp_db::TempDb::new("tx-import-malformed");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let import_path = db.path().with_file_name("malformed-import.json");
    fs::write(&import_path, "{invalid-json").expect("write malformed import");
    let before = manager
        .list_workspaces()
        .expect("list workspaces before")
        .len();

    let err = manager
        .import_workspace_json(&import_path, "llama3.2:latest")
        .expect_err("malformed import should fail");
    assert!(matches!(err, WorkspaceError::Json(_)));

    let after = manager
        .list_workspaces()
        .expect("list workspaces after")
        .len();
    assert_eq!(after, before);
}

#[test]
fn restore_missing_checkpoint_does_not_create_conversation() {
    let db = common::temp_db::TempDb::new("tx-restore-missing-checkpoint");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let before = manager
        .list_conversations(session.workspace_id)
        .expect("list conversations before")
        .len();

    let err = manager
        .restore_checkpoint(session.workspace_id, session.conversation_id, "missing")
        .expect_err("missing checkpoint should fail");
    assert!(matches!(err, WorkspaceError::NotFound(_)));

    let after = manager
        .list_conversations(session.workspace_id)
        .expect("list conversations after")
        .len();
    assert_eq!(after, before);
}

#[test]
fn set_workspace_root_path_failure_keeps_existing_root_path() {
    let db = common::temp_db::TempDb::new("tx-set-path-failure");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Path Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let before = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .find(|workspace| workspace.id == session.workspace_id)
        .expect("workspace should exist")
        .root_path;

    let err = manager
        .set_workspace_root_path(
            session.workspace_id,
            &db.path().with_file_name("missing-folder"),
        )
        .expect_err("missing folder should fail");
    assert!(matches!(err, WorkspaceError::NotFound(_)));

    let after = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .find(|workspace| workspace.id == session.workspace_id)
        .expect("workspace should exist")
        .root_path;
    assert_eq!(after, before);
}

#[test]
fn rename_missing_workspace_keeps_workspace_count() {
    let db = common::temp_db::TempDb::new("tx-rename-missing-workspace");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Rename Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let before = manager
        .list_workspaces()
        .expect("list workspaces before")
        .len();

    let err = manager
        .rename_workspace(session.workspace_id + 10_000, "Never Applies")
        .expect_err("rename missing workspace should fail");
    assert!(matches!(err, WorkspaceError::NotFound(_)));

    let after = manager
        .list_workspaces()
        .expect("list workspaces after")
        .len();
    assert_eq!(after, before);
}

#[test]
fn rename_missing_session_keeps_session_count() {
    let db = common::temp_db::TempDb::new("tx-rename-missing-session");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Session Rename Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let before = manager
        .list_conversations(session.workspace_id)
        .expect("list sessions before")
        .len();

    let err = manager
        .rename_session(session.conversation_id + 10_000, "Never Applies")
        .expect_err("rename missing session should fail");
    assert!(matches!(err, WorkspaceError::NotFound(_)));

    let after = manager
        .list_conversations(session.workspace_id)
        .expect("list sessions after")
        .len();
    assert_eq!(after, before);
}

#[test]
fn delete_missing_session_keeps_message_count() {
    let db = common::temp_db::TempDb::new("tx-delete-missing-session");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Delete Workspace", "llama3.2:latest")
        .expect("bootstrap");
    manager
        .save_message(session.conversation_id, "user", "hello")
        .expect("save baseline message");
    let before = scalar_count(db.path(), "SELECT COUNT(*) FROM messages");

    let err = manager
        .delete_session(session.conversation_id + 10_000)
        .expect_err("delete missing session should fail");
    assert!(matches!(err, WorkspaceError::NotFound(_)));

    let after = scalar_count(db.path(), "SELECT COUNT(*) FROM messages");
    assert_eq!(after, before);
}

#[test]
fn blocked_context_path_does_not_mutate_workspace_context_paths() {
    let db = common::temp_db::TempDb::new("tx-blocked-context");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Context Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let before = manager
        .list_workspaces()
        .expect("list workspaces before")
        .into_iter()
        .find(|workspace| workspace.id == session.workspace_id)
        .expect("workspace should exist")
        .context_paths;

    let err = manager
        .add_context_file(session.workspace_id, &db.path().with_file_name(".env"))
        .expect_err("blocked context path should fail");
    assert!(matches!(err, WorkspaceError::InvalidContextPath(_)));

    let after = manager
        .list_workspaces()
        .expect("list workspaces after")
        .into_iter()
        .find(|workspace| workspace.id == session.workspace_id)
        .expect("workspace should exist")
        .context_paths;
    assert_eq!(after, before);
}

#[test]
fn export_workspace_invalid_parent_error_keeps_workspace_count() {
    let db = common::temp_db::TempDb::new("tx-export-workspace-path-error");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Export Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let blocker = db.path().with_file_name("blocker");
    fs::write(&blocker, "file-not-directory").expect("write blocker file");
    let invalid_output = blocker.join("nested").join("workspace.json");
    let before = manager
        .list_workspaces()
        .expect("list workspaces before")
        .len();

    let err = manager
        .export_workspace_json(session.workspace_id, &invalid_output)
        .expect_err("invalid parent should fail");
    assert!(matches!(err, WorkspaceError::Io(_)));

    let after = manager
        .list_workspaces()
        .expect("list workspaces after")
        .len();
    assert_eq!(after, before);
}

#[test]
fn export_markdown_invalid_parent_error_keeps_message_count() {
    let db = common::temp_db::TempDb::new("tx-export-markdown-path-error");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Tx Markdown Workspace", "llama3.2:latest")
        .expect("bootstrap");
    manager
        .save_message(session.conversation_id, "assistant", "payload")
        .expect("save message");
    let blocker = db.path().with_file_name("markdown-blocker");
    fs::write(&blocker, "file-not-directory").expect("write blocker file");
    let invalid_output = blocker.join("nested").join("conversation.md");
    let before = scalar_count(db.path(), "SELECT COUNT(*) FROM messages");

    let err = manager
        .export_conversation_markdown(session.conversation_id, &invalid_output)
        .expect_err("invalid parent should fail");
    assert!(matches!(err, WorkspaceError::Io(_)));

    let after = scalar_count(db.path(), "SELECT COUNT(*) FROM messages");
    assert_eq!(after, before);
}
