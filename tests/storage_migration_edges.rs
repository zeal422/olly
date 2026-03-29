use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use olly::workspace::{error::WorkspaceError, manager::WorkspaceManager, storage::Storage};
use rusqlite::Connection;

fn unique_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("olly-storage-tests-{label}-{stamp}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn query_i64(path: &PathBuf, sql: &str) -> i64 {
    let conn = Connection::open(path).expect("open sqlite connection");
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .expect("execute scalar query")
}

#[test]
fn storage_open_creates_parent_directories() {
    let root = unique_root("open-creates-parents");
    let db_path = root.join("nested").join("more").join("workspaces.db");
    let parent = db_path.parent().expect("db parent exists").to_path_buf();
    assert!(!parent.exists());

    let _storage = Storage::open(&db_path).expect("open storage should create directories");
    assert!(parent.exists());
}

#[test]
fn migrate_is_idempotent_across_reopens() {
    let root = unique_root("migrate-idempotent");
    let db_path = root.join("workspaces.db");

    {
        let storage = Storage::open(&db_path).expect("open storage first time");
        storage.migrate().expect("migrate first time");
    }
    {
        let storage = Storage::open(&db_path).expect("open storage second time");
        storage.migrate().expect("migrate second time");
    }

    let conn = Connection::open(&db_path).expect("open sqlite connection");
    let mut statement = conn
        .prepare("PRAGMA table_info(workspaces)")
        .expect("prepare table info query");
    let mut rows = statement.query([]).expect("query table info");
    let mut root_path_column_count = 0_i64;
    while let Some(row) = rows.next().expect("next row") {
        let name: String = row.get(1).expect("column name");
        if name == "root_path" {
            root_path_column_count += 1;
        }
    }
    assert_eq!(root_path_column_count, 1);
}

#[test]
fn migrate_adds_root_path_column_to_legacy_schema() {
    let root = unique_root("legacy-root-path-column");
    let db_path = root.join("legacy.db");
    let conn = Connection::open(&db_path).expect("open sqlite connection");
    conn.execute_batch(
        r#"
        CREATE TABLE workspaces (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            context_paths TEXT NOT NULL DEFAULT '[]',
            system_prompt TEXT,
            model_preset_id INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .expect("create legacy workspaces schema");
    drop(conn);

    let _storage = Storage::open(&db_path).expect("open storage should migrate legacy schema");

    let conn = Connection::open(&db_path).expect("open sqlite connection");
    let mut statement = conn
        .prepare("PRAGMA table_info(workspaces)")
        .expect("prepare table info query");
    let mut rows = statement.query([]).expect("query table info");
    let mut has_root_path = false;
    while let Some(row) = rows.next().expect("next row") {
        let name: String = row.get(1).expect("column name");
        if name == "root_path" {
            has_root_path = true;
        }
    }
    assert!(has_root_path);
}

#[test]
fn delete_workspace_cascades_related_records() {
    let root = unique_root("delete-cascade");
    let db_path = root.join("workspaces.db");
    let manager = WorkspaceManager::open(&db_path).expect("open manager");
    let session = manager
        .bootstrap_default("Cascade Workspace", "llama3.2:latest")
        .expect("bootstrap default");

    manager
        .save_message(session.conversation_id, "user", "hello cascade")
        .expect("save message");
    manager
        .save_checkpoint(session.conversation_id, "cp-cascade")
        .expect("save checkpoint");

    let context_file = root.join("ctx.txt");
    fs::write(&context_file, "context content").expect("write context file");
    manager
        .add_context_file(session.workspace_id, &context_file)
        .expect("add context file");

    assert!(
        query_i64(
            &db_path,
            &format!(
                "SELECT COUNT(*) FROM conversations WHERE workspace_id = {}",
                session.workspace_id
            )
        ) > 0
    );
    assert!(
        query_i64(
            &db_path,
            &format!(
                "SELECT COUNT(*) FROM context_files WHERE workspace_id = {}",
                session.workspace_id
            )
        ) > 0
    );
    assert!(
        query_i64(
            &db_path,
            &format!(
                "SELECT COUNT(*) FROM checkpoints WHERE conversation_id = {}",
                session.conversation_id
            )
        ) > 0
    );

    manager
        .delete_workspace(session.workspace_id)
        .expect("delete workspace");

    assert_eq!(
        query_i64(
            &db_path,
            &format!(
                "SELECT COUNT(*) FROM workspaces WHERE id = {}",
                session.workspace_id
            )
        ),
        0
    );
    assert_eq!(
        query_i64(
            &db_path,
            &format!(
                "SELECT COUNT(*) FROM conversations WHERE workspace_id = {}",
                session.workspace_id
            )
        ),
        0
    );
    assert_eq!(query_i64(&db_path, "SELECT COUNT(*) FROM messages"), 0);
    assert_eq!(query_i64(&db_path, "SELECT COUNT(*) FROM checkpoints"), 0);
    assert_eq!(query_i64(&db_path, "SELECT COUNT(*) FROM context_files"), 0);
}

#[test]
fn rename_conversation_missing_id_returns_not_found() {
    let root = unique_root("rename-conversation-missing");
    let db_path = root.join("workspaces.db");
    let storage = Storage::open(&db_path).expect("open storage");

    let err = storage
        .rename_conversation(999_999, "missing")
        .expect_err("rename should fail");
    assert!(matches!(err, WorkspaceError::NotFound(_)));
}

#[test]
fn update_workspace_context_paths_persists_exact_array() {
    let root = unique_root("context-paths-persist");
    let db_path = root.join("workspaces.db");
    let storage = Storage::open(&db_path).expect("open storage");

    let preset_id = storage
        .get_or_create_preset("default", "llama3.2:latest")
        .expect("create preset");
    let workspace_id = storage
        .create_workspace("Context Paths Workspace", preset_id, None, None)
        .expect("create workspace");

    let expected_paths = vec![
        String::from("C:/repo/src/main.rs"),
        String::from("C:/repo/README.md"),
    ];
    storage
        .update_workspace_context_paths(workspace_id, &expected_paths)
        .expect("update context paths");

    let loaded = storage
        .get_workspace_by_id(workspace_id)
        .expect("load workspace")
        .expect("workspace should exist");
    assert_eq!(loaded.context_paths, expected_paths);
}
