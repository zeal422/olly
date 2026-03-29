mod common;

use olly::workspace::{manager::WorkspaceManager, storage::Storage};
use rusqlite::Connection;

fn table_exists(db_path: &std::path::Path, table: &str) -> bool {
    let conn = Connection::open(db_path).expect("open sqlite connection");
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
        [table],
        |row| row.get::<_, i64>(0),
    )
    .expect("table existence query")
        > 0
}

fn trigger_exists(db_path: &std::path::Path, trigger: &str) -> bool {
    let conn = Connection::open(db_path).expect("open sqlite connection");
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name = ?1",
        [trigger],
        |row| row.get::<_, i64>(0),
    )
    .expect("trigger existence query")
        > 0
}

fn has_column(db_path: &std::path::Path, table: &str, column: &str) -> bool {
    let conn = Connection::open(db_path).expect("open sqlite connection");
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("prepare table info");
    let mut rows = statement.query([]).expect("query table info");
    while let Some(row) = rows.next().expect("next table info row") {
        let name: String = row.get(1).expect("column name");
        if name == column {
            return true;
        }
    }
    false
}

fn setup_legacy_core_schema(db_path: &std::path::Path) {
    let conn = Connection::open(db_path).expect("open sqlite connection");
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE presets (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            model TEXT NOT NULL,
            options TEXT NOT NULL DEFAULT '{}',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE workspaces (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            context_paths TEXT NOT NULL DEFAULT '[]',
            system_prompt TEXT,
            model_preset_id INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (model_preset_id) REFERENCES presets(id)
        );
        CREATE TABLE conversations (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL,
            title TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        );
        CREATE TABLE messages (
            id INTEGER PRIMARY KEY,
            conversation_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );
        "#,
    )
    .expect("create legacy core schema");
}

#[test]
fn migrate_adds_root_path_column_when_missing() {
    let db = common::temp_db::TempDb::new("migration-root-path-column");
    setup_legacy_core_schema(db.path());

    let _storage = Storage::open(db.path()).expect("open storage");
    assert!(has_column(db.path(), "workspaces", "root_path"));
}

#[test]
fn migrate_creates_missing_checkpoints_table() {
    let db = common::temp_db::TempDb::new("migration-checkpoints-table");
    setup_legacy_core_schema(db.path());

    let _storage = Storage::open(db.path()).expect("open storage");
    assert!(table_exists(db.path(), "checkpoints"));
}

#[test]
fn migrate_creates_missing_context_files_table() {
    let db = common::temp_db::TempDb::new("migration-context-files-table");
    setup_legacy_core_schema(db.path());

    let _storage = Storage::open(db.path()).expect("open storage");
    assert!(table_exists(db.path(), "context_files"));
}

#[test]
fn migrate_adds_missing_message_metadata_columns() {
    let db = common::temp_db::TempDb::new("migration-message-meta-columns");
    setup_legacy_core_schema(db.path());

    let _storage = Storage::open(db.path()).expect("open storage");
    assert!(has_column(db.path(), "messages", "images"));
    assert!(has_column(db.path(), "messages", "tool_calls"));
}

#[test]
fn migrate_recreates_missing_fts_triggers() {
    let db = common::temp_db::TempDb::new("migration-fts-triggers");
    setup_legacy_core_schema(db.path());
    let conn = Connection::open(db.path()).expect("open sqlite connection");
    conn.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            content,
            role,
            conversation_id,
            content=messages,
            content_rowid=id
        );
        DROP TRIGGER IF EXISTS messages_ai;
        DROP TRIGGER IF EXISTS messages_ad;
        DROP TRIGGER IF EXISTS messages_au;
        "#,
    )
    .expect("prepare missing-trigger state");
    drop(conn);

    let _storage = Storage::open(db.path()).expect("open storage");
    assert!(trigger_exists(db.path(), "messages_ai"));
    assert!(trigger_exists(db.path(), "messages_ad"));
    assert!(trigger_exists(db.path(), "messages_au"));
}

#[test]
fn migrate_handles_partially_initialized_schema_without_panic() {
    let db = common::temp_db::TempDb::new("migration-partial-schema");
    let conn = Connection::open(db.path()).expect("open sqlite connection");
    conn.execute_batch(
        r#"
        CREATE TABLE presets (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            model TEXT NOT NULL,
            options TEXT NOT NULL DEFAULT '{}',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .expect("create partial schema");
    drop(conn);

    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Partial Schema Workspace", "llama3.2:latest")
        .expect("bootstrap should succeed");
    assert!(session.workspace_id > 0);
    assert!(session.conversation_id > 0);
}

#[test]
fn repeated_migration_is_idempotent() {
    let db = common::temp_db::TempDb::new("migration-idempotent");
    let storage = Storage::open(db.path()).expect("open storage");

    for _ in 0..5 {
        storage.migrate().expect("repeat migrate");
    }

    assert!(has_column(db.path(), "workspaces", "root_path"));
    assert!(has_column(db.path(), "messages", "images"));
    assert!(has_column(db.path(), "messages", "tool_calls"));
}

#[test]
fn legacy_workspace_rows_survive_migration() {
    let db = common::temp_db::TempDb::new("migration-legacy-workspaces");
    setup_legacy_core_schema(db.path());
    let conn = Connection::open(db.path()).expect("open sqlite connection");
    conn.execute(
        "INSERT INTO presets (id, name, model, options) VALUES (1, 'Default', 'llama3.2:latest', '{}')",
        [],
    )
    .expect("insert preset");
    conn.execute(
        "INSERT INTO workspaces (id, name, context_paths, system_prompt, model_preset_id) VALUES (11, 'Legacy Workspace', '[]', NULL, 1)",
        [],
    )
    .expect("insert legacy workspace");
    drop(conn);

    let storage = Storage::open(db.path()).expect("open storage");
    let workspace = storage
        .get_workspace_by_id(11)
        .expect("query workspace")
        .expect("workspace should exist");
    assert_eq!(workspace.name, "Legacy Workspace");
    assert_eq!(workspace.root_path, None);
}

#[test]
fn legacy_conversation_rows_survive_and_are_queryable() {
    let db = common::temp_db::TempDb::new("migration-legacy-conversations");
    setup_legacy_core_schema(db.path());
    let conn = Connection::open(db.path()).expect("open sqlite connection");
    conn.execute(
        "INSERT INTO presets (id, name, model, options) VALUES (1, 'Default', 'llama3.2:latest', '{}')",
        [],
    )
    .expect("insert preset");
    conn.execute(
        "INSERT INTO workspaces (id, name, context_paths, system_prompt, model_preset_id) VALUES (12, 'Conversation Workspace', '[]', NULL, 1)",
        [],
    )
    .expect("insert workspace");
    conn.execute(
        "INSERT INTO conversations (id, workspace_id, title) VALUES (41, 12, 'Legacy Session')",
        [],
    )
    .expect("insert conversation");
    drop(conn);

    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let sessions = manager
        .list_conversations(12)
        .expect("list migrated conversations");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title.as_deref(), Some("Legacy Session"));
}

#[test]
fn post_migration_crud_search_and_checkpoint_operations_work() {
    let db = common::temp_db::TempDb::new("migration-post-crud");
    setup_legacy_core_schema(db.path());

    let manager = WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Post Migration", "llama3.2:latest")
        .expect("bootstrap");
    manager
        .save_message(session.conversation_id, "user", "migration smoke message")
        .expect("save message");
    let hits = manager
        .search_messages("migration")
        .expect("search messages after migration");
    assert!(!hits.is_empty());

    manager
        .save_checkpoint(session.conversation_id, "post-migrate")
        .expect("save checkpoint");
    let restored = manager
        .restore_checkpoint(
            session.workspace_id,
            session.conversation_id,
            "post-migrate",
        )
        .expect("restore checkpoint");
    let restored_messages = manager
        .load_messages(restored)
        .expect("load restored messages");
    assert!(!restored_messages.is_empty());
}
