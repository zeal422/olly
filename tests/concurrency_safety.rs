mod common;

use std::sync::{Arc, Barrier};

use olly::workspace::{error::WorkspaceError, manager::WorkspaceManager};
use rusqlite::Connection;

fn scalar_count(db_path: &std::path::Path, sql: &str) -> i64 {
    let conn = Connection::open(db_path).expect("open sqlite connection");
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .expect("run scalar query")
}

#[test]
fn two_managers_append_messages_concurrently_to_same_conversation() {
    let db = common::temp_db::TempDb::new("concurrent-append");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Concurrent Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let barrier = Arc::new(Barrier::new(3));
    let db_path_a = db.path().to_path_buf();
    let db_path_b = db.path().to_path_buf();

    let barrier_a = barrier.clone();
    let t1 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_a).expect("open manager A");
        barrier_a.wait();
        for idx in 0..20 {
            mgr.save_message(session.conversation_id, "user", &format!("A-{idx}"))
                .expect("save message A");
        }
    });

    let barrier_b = barrier.clone();
    let t2 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_b).expect("open manager B");
        barrier_b.wait();
        for idx in 0..20 {
            mgr.save_message(session.conversation_id, "assistant", &format!("B-{idx}"))
                .expect("save message B");
        }
    });

    barrier.wait();
    t1.join().expect("join writer A");
    t2.join().expect("join writer B");

    let messages = manager
        .load_messages(session.conversation_id)
        .expect("load messages after concurrent writes");
    assert_eq!(messages.len(), 40);
}

#[test]
fn two_managers_create_sessions_concurrently_in_same_workspace() {
    let db = common::temp_db::TempDb::new("concurrent-create-sessions");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Concurrent Sessions", "llama3.2:latest")
        .expect("bootstrap");
    let before = manager
        .list_conversations(session.workspace_id)
        .expect("list sessions before")
        .len();
    let barrier = Arc::new(Barrier::new(3));
    let db_path_a = db.path().to_path_buf();
    let db_path_b = db.path().to_path_buf();

    let barrier_a = barrier.clone();
    let t1 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_a).expect("open manager A");
        barrier_a.wait();
        mgr.create_session(session.workspace_id, Some("A"))
            .expect("create session A");
    });

    let barrier_b = barrier.clone();
    let t2 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_b).expect("open manager B");
        barrier_b.wait();
        mgr.create_session(session.workspace_id, Some("B"))
            .expect("create session B");
    });

    barrier.wait();
    t1.join().expect("join create A");
    t2.join().expect("join create B");

    let after = manager
        .list_conversations(session.workspace_id)
        .expect("list sessions after")
        .len();
    assert_eq!(after, before + 2);
}

#[test]
fn rename_while_listing_sessions_keeps_consistent_state() {
    let db = common::temp_db::TempDb::new("concurrent-rename-list");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Concurrent Rename", "llama3.2:latest")
        .expect("bootstrap");
    manager
        .create_session(session.workspace_id, Some("Target Session"))
        .expect("create target session");
    let target = manager
        .list_conversations(session.workspace_id)
        .expect("list sessions")
        .into_iter()
        .find(|entry| entry.title.as_deref() == Some("Target Session"))
        .expect("target session exists")
        .id;
    let db_path_a = db.path().to_path_buf();
    let db_path_b = db.path().to_path_buf();

    let renamer = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_a).expect("open manager renamer");
        for idx in 0..15 {
            mgr.rename_session(target, &format!("Name {idx}"))
                .expect("rename session");
        }
    });

    let lister = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_b).expect("open manager lister");
        for _ in 0..40 {
            let sessions = mgr
                .list_conversations(session.workspace_id)
                .expect("list sessions under rename");
            assert!(!sessions.is_empty());
        }
    });

    renamer.join().expect("join renamer");
    lister.join().expect("join lister");

    let final_entry = manager
        .list_conversations(session.workspace_id)
        .expect("final list")
        .into_iter()
        .find(|entry| entry.id == target)
        .expect("target should exist");
    assert!(final_entry
        .title
        .as_deref()
        .unwrap_or_default()
        .starts_with("Name "));
}

#[test]
fn concurrent_checkpoint_upserts_keep_single_checkpoint_row() {
    let db = common::temp_db::TempDb::new("concurrent-checkpoint-upsert");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Concurrent Checkpoint", "llama3.2:latest")
        .expect("bootstrap");
    let barrier = Arc::new(Barrier::new(3));
    let db_path_a = db.path().to_path_buf();
    let db_path_b = db.path().to_path_buf();

    let barrier_a = barrier.clone();
    let t1 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_a).expect("open manager A");
        barrier_a.wait();
        mgr.save_message(session.conversation_id, "user", "payload-a")
            .expect("save msg a");
        mgr.save_checkpoint(session.conversation_id, "same-name")
            .expect("save checkpoint a");
    });

    let barrier_b = barrier.clone();
    let t2 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_b).expect("open manager B");
        barrier_b.wait();
        mgr.save_message(session.conversation_id, "user", "payload-b")
            .expect("save msg b");
        mgr.save_checkpoint(session.conversation_id, "same-name")
            .expect("save checkpoint b");
    });

    barrier.wait();
    t1.join().expect("join checkpoint A");
    t2.join().expect("join checkpoint B");

    let count = scalar_count(
        db.path(),
        &format!(
            "SELECT COUNT(*) FROM checkpoints WHERE conversation_id = {} AND name = 'same-name'",
            session.conversation_id
        ),
    );
    assert_eq!(count, 1);

    let checkpoint = manager
        .restore_checkpoint(session.workspace_id, session.conversation_id, "same-name")
        .expect("restore checkpoint");
    assert!(checkpoint > 0);
}

#[test]
fn concurrent_workspace_list_and_delete_stays_consistent() {
    let db = common::temp_db::TempDb::new("concurrent-list-delete-workspace");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let _session = manager
        .bootstrap_default("Main Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let root = db
        .path()
        .parent()
        .expect("temp root parent")
        .join("delete-target-root");
    std::fs::create_dir_all(&root).expect("create root dir");
    let target = manager
        .create_workspace("Delete Target", "llama3.2:latest", Some(&root))
        .expect("create target workspace");
    let db_path = db.path().to_path_buf();

    let lister = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path).expect("open lister manager");
        for _ in 0..60 {
            let _ = mgr.list_workspaces().expect("list workspaces");
        }
    });

    manager
        .delete_workspace(target.workspace_id)
        .expect("delete workspace");
    lister.join().expect("join lister");

    assert!(!manager
        .list_workspaces()
        .expect("list after delete")
        .iter()
        .any(|workspace| workspace.id == target.workspace_id));
}

#[test]
fn concurrent_set_workspace_paths_keeps_canonical_final_value() {
    let db = common::temp_db::TempDb::new("concurrent-set-path");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Path Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let root_base = db.path().parent().expect("temp root parent");
    let path_a = root_base.join("path-a");
    let path_b = root_base.join("path-b");
    std::fs::create_dir_all(&path_a).expect("create path a");
    std::fs::create_dir_all(&path_b).expect("create path b");
    let canonical_a = std::fs::canonicalize(&path_a)
        .expect("canonical a")
        .to_string_lossy()
        .into_owned();
    let canonical_b = std::fs::canonicalize(&path_b)
        .expect("canonical b")
        .to_string_lossy()
        .into_owned();
    let barrier = Arc::new(Barrier::new(3));
    let db_path_a = db.path().to_path_buf();
    let db_path_b = db.path().to_path_buf();
    let path_a_thread = path_a.clone();
    let path_b_thread = path_b.clone();

    let barrier_a = barrier.clone();
    let t1 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_a).expect("open manager A");
        barrier_a.wait();
        mgr.set_workspace_root_path(session.workspace_id, &path_a_thread)
            .expect("set path a");
    });

    let barrier_b = barrier.clone();
    let t2 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_b).expect("open manager B");
        barrier_b.wait();
        mgr.set_workspace_root_path(session.workspace_id, &path_b_thread)
            .expect("set path b");
    });

    barrier.wait();
    t1.join().expect("join path writer A");
    t2.join().expect("join path writer B");

    let final_workspace = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .find(|workspace| workspace.id == session.workspace_id)
        .expect("workspace should exist");
    let final_path = final_workspace.root_path.expect("root path should be set");
    assert!(final_path == canonical_a || final_path == canonical_b);
}

#[test]
fn concurrent_context_upserts_for_same_file_keep_single_entry() {
    let db = common::temp_db::TempDb::new("concurrent-context-upsert");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Context Workspace", "llama3.2:latest")
        .expect("bootstrap");
    let context_file = db.path().with_file_name("shared-context.txt");
    std::fs::write(&context_file, "shared context payload").expect("write context file");
    let barrier = Arc::new(Barrier::new(3));
    let db_path_a = db.path().to_path_buf();
    let db_path_b = db.path().to_path_buf();
    let context_a = context_file.clone();
    let context_b = context_file.clone();

    let barrier_a = barrier.clone();
    let t1 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_a).expect("open manager A");
        barrier_a.wait();
        mgr.add_context_file(session.workspace_id, &context_a)
            .expect("add context file A");
    });

    let barrier_b = barrier.clone();
    let t2 = std::thread::spawn(move || {
        let mgr = WorkspaceManager::open(&db_path_b).expect("open manager B");
        barrier_b.wait();
        mgr.add_context_file(session.workspace_id, &context_b)
            .expect("add context file B");
    });

    barrier.wait();
    t1.join().expect("join context writer A");
    t2.join().expect("join context writer B");

    let count = scalar_count(
        db.path(),
        &format!(
            "SELECT COUNT(*) FROM context_files WHERE workspace_id = {}",
            session.workspace_id
        ),
    );
    assert_eq!(count, 1);
    let workspace = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .find(|workspace| workspace.id == session.workspace_id)
        .expect("workspace should exist");
    assert_eq!(workspace.context_paths.len(), 1);
}

#[test]
fn lock_contention_returns_controlled_db_error() {
    let db = common::temp_db::TempDb::new("concurrent-lock-contention");
    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Locked Workspace", "llama3.2:latest")
        .expect("bootstrap");

    let lock_conn = Connection::open(db.path()).expect("open lock connection");
    lock_conn
        .execute_batch("BEGIN EXCLUSIVE;")
        .expect("begin exclusive lock");

    let err = manager
        .create_session(session.workspace_id, Some("blocked"))
        .expect_err("write should fail while exclusive lock is active");
    assert!(matches!(err, WorkspaceError::Db(_)));

    lock_conn
        .execute_batch("ROLLBACK;")
        .expect("release lock transaction");
}
