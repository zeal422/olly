mod common;

use olly::{
    app::context::is_sensitive_or_excluded, workspace::error::WorkspaceError,
    workspace::manager::WorkspaceManager,
};

#[test]
fn sensitive_patterns_block_env_and_key_files() {
    assert!(is_sensitive_or_excluded("C:/repo/.env"));
    assert!(is_sensitive_or_excluded("C:/repo/.env.local"));
    assert!(is_sensitive_or_excluded("C:/repo/secrets/private.pem"));
    assert!(is_sensitive_or_excluded("C:/repo/secrets/service.key"));
    assert!(is_sensitive_or_excluded("C:/Users/name/.ssh/id_rsa"));
}

#[test]
fn traversal_like_sensitive_paths_are_still_blocked() {
    assert!(is_sensitive_or_excluded("../.env"));
    assert!(is_sensitive_or_excluded("../../keys/secret.key"));
    assert!(is_sensitive_or_excluded("../repo/.git/config"));
}

#[test]
fn matching_is_case_insensitive_for_windows_style_inputs() {
    assert!(is_sensitive_or_excluded(r"C:\Repo\.ENV"));
    assert!(is_sensitive_or_excluded(
        r"C:\Repo\NODE_MODULES\pkg\index.js"
    ));
    assert!(is_sensitive_or_excluded(r"C:\Repo\TARGET\debug\olly.exe"));
}

#[test]
fn mixed_slash_and_backslash_forms_are_consistent() {
    assert!(is_sensitive_or_excluded(r"C:/repo/.git\objects\aa\bb"));
    assert!(is_sensitive_or_excluded(
        r"C:\repo\node_modules/pkg/index.js"
    ));
    assert!(is_sensitive_or_excluded(r"C:/repo\target\debug/olly.exe"));
}

#[test]
fn normal_project_files_remain_allowed() {
    assert!(!is_sensitive_or_excluded("C:/repo/src/main.rs"));
    assert!(!is_sensitive_or_excluded("C:/repo/README.md"));
    assert!(!is_sensitive_or_excluded("C:/repo/tests/app_test.rs"));
}

#[test]
fn blocked_context_add_does_not_change_workspace_context_paths() {
    let db = common::temp_db::TempDb::new("path-security-context-side-effects");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Security Workspace", "llama3.2:latest")
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
    assert_eq!(before, after);
}
