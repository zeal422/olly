use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::workspace::{
    error::WorkspaceError, Checkpoint, ContextFileMeta, Conversation, ConversationMessage,
    SearchHit, Workspace,
};

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self, WorkspaceError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_millis(500))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn migrate(&self) -> Result<(), WorkspaceError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS presets (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                model TEXT NOT NULL,
                options TEXT NOT NULL DEFAULT '{}',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS workspaces (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                root_path TEXT,
                context_paths TEXT NOT NULL DEFAULT '[]',
                system_prompt TEXT,
                model_preset_id INTEGER NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (model_preset_id) REFERENCES presets(id)
            );

            CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY,
                workspace_id INTEGER NOT NULL,
                title TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY,
                conversation_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                images TEXT,
                tool_calls TEXT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS context_files (
                id INTEGER PRIMARY KEY,
                workspace_id INTEGER NOT NULL,
                path TEXT NOT NULL,
                hash TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(workspace_id, path),
                FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS checkpoints (
                id INTEGER PRIMARY KEY,
                conversation_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                snapshot_json TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(conversation_id, name),
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                content,
                role,
                conversation_id,
                content=messages,
                content_rowid=id
            );

            CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, content, role, conversation_id)
                VALUES (new.id, new.content, new.role, new.conversation_id);
            END;

            CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, role, conversation_id)
                VALUES ('delete', old.id, old.content, old.role, old.conversation_id);
            END;

            CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, role, conversation_id)
                VALUES ('delete', old.id, old.content, old.role, old.conversation_id);
                INSERT INTO messages_fts(rowid, content, role, conversation_id)
                VALUES (new.id, new.content, new.role, new.conversation_id);
            END;
            "#,
        )?;
        self.ensure_workspace_root_path_column()?;
        self.ensure_message_metadata_columns()?;
        Ok(())
    }

    fn ensure_workspace_root_path_column(&self) -> Result<(), WorkspaceError> {
        let mut statement = self.conn.prepare("PRAGMA table_info(workspaces)")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let column_name: String = row.get(1)?;
            if column_name == "root_path" {
                return Ok(());
            }
        }

        self.conn
            .execute("ALTER TABLE workspaces ADD COLUMN root_path TEXT", [])?;
        Ok(())
    }

    fn ensure_message_metadata_columns(&self) -> Result<(), WorkspaceError> {
        let mut has_images = false;
        let mut has_tool_calls = false;
        let mut statement = self.conn.prepare("PRAGMA table_info(messages)")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let column_name: String = row.get(1)?;
            if column_name == "images" {
                has_images = true;
            } else if column_name == "tool_calls" {
                has_tool_calls = true;
            }
        }

        if !has_images {
            self.conn
                .execute("ALTER TABLE messages ADD COLUMN images TEXT", [])?;
        }
        if !has_tool_calls {
            self.conn
                .execute("ALTER TABLE messages ADD COLUMN tool_calls TEXT", [])?;
        }
        Ok(())
    }

    pub fn get_or_create_preset(&self, name: &str, model: &str) -> Result<i64, WorkspaceError> {
        let existing = self
            .conn
            .query_row(
                "SELECT id FROM presets WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO presets (name, model, options) VALUES (?1, ?2, '{}')",
            params![name, model],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn create_workspace(
        &self,
        name: &str,
        model_preset_id: i64,
        root_path: Option<&str>,
        system_prompt: Option<&str>,
    ) -> Result<i64, WorkspaceError> {
        self.conn.execute(
            "INSERT INTO workspaces (name, root_path, model_preset_id, system_prompt, context_paths) VALUES (?1, ?2, ?3, ?4, '[]')",
            params![name, root_path, model_preset_id, system_prompt],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_workspace_by_name(&self, name: &str) -> Result<Option<Workspace>, WorkspaceError> {
        self.conn
            .query_row(
                "SELECT id, name, root_path, context_paths, system_prompt, model_preset_id FROM workspaces WHERE name = ?1",
                params![name],
                |row| {
                    let context_paths_json: String = row.get(3)?;
                    let context_paths = serde_json::from_str::<Vec<String>>(&context_paths_json)
                        .unwrap_or_default();
                    Ok(Workspace {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        root_path: row.get(2)?,
                        context_paths,
                        system_prompt: row.get(4)?,
                        model_preset_id: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(WorkspaceError::from)
    }

    pub fn get_workspace_by_id(&self, id: i64) -> Result<Option<Workspace>, WorkspaceError> {
        self.conn
            .query_row(
                "SELECT id, name, root_path, context_paths, system_prompt, model_preset_id FROM workspaces WHERE id = ?1",
                params![id],
                |row| {
                    let context_paths_json: String = row.get(3)?;
                    let context_paths =
                        serde_json::from_str::<Vec<String>>(&context_paths_json).unwrap_or_default();
                    Ok(Workspace {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        root_path: row.get(2)?,
                        context_paths,
                        system_prompt: row.get(4)?,
                        model_preset_id: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(WorkspaceError::from)
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, WorkspaceError> {
        let mut statement = self.conn.prepare(
            "SELECT id, name, root_path, context_paths, system_prompt, model_preset_id FROM workspaces ORDER BY id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            let context_paths_json: String = row.get(3)?;
            let context_paths =
                serde_json::from_str::<Vec<String>>(&context_paths_json).unwrap_or_default();
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                root_path: row.get(2)?,
                context_paths,
                system_prompt: row.get(4)?,
                model_preset_id: row.get(5)?,
            })
        })?;
        let mut workspaces = Vec::new();
        for row in rows {
            workspaces.push(row?);
        }
        Ok(workspaces)
    }

    pub fn delete_workspace(&self, workspace_id: i64) -> Result<(), WorkspaceError> {
        self.conn.execute(
            "DELETE FROM workspaces WHERE id = ?1",
            params![workspace_id],
        )?;
        Ok(())
    }

    pub fn rename_workspace(&self, workspace_id: i64, name: &str) -> Result<(), WorkspaceError> {
        let updated = self.conn.execute(
            "UPDATE workspaces SET name = ?1 WHERE id = ?2",
            params![name, workspace_id],
        )?;
        if updated == 0 {
            return Err(WorkspaceError::NotFound(format!(
                "workspace {workspace_id}"
            )));
        }
        Ok(())
    }

    pub fn update_workspace_root_path(
        &self,
        workspace_id: i64,
        root_path: Option<&str>,
    ) -> Result<(), WorkspaceError> {
        let updated = self.conn.execute(
            "UPDATE workspaces SET root_path = ?1 WHERE id = ?2",
            params![root_path, workspace_id],
        )?;
        if updated == 0 {
            return Err(WorkspaceError::NotFound(format!(
                "workspace {workspace_id}"
            )));
        }
        Ok(())
    }

    pub fn create_conversation(
        &self,
        workspace_id: i64,
        title: Option<&str>,
    ) -> Result<i64, WorkspaceError> {
        self.conn.execute(
            "INSERT INTO conversations (workspace_id, title) VALUES (?1, ?2)",
            params![workspace_id, title],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn latest_conversation(
        &self,
        workspace_id: i64,
    ) -> Result<Option<Conversation>, WorkspaceError> {
        self.conn
            .query_row(
                "SELECT id, workspace_id, title FROM conversations WHERE workspace_id = ?1 ORDER BY id DESC LIMIT 1",
                params![workspace_id],
                |row| {
                    Ok(Conversation {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        title: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(WorkspaceError::from)
    }

    pub fn list_conversations(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<Conversation>, WorkspaceError> {
        let mut statement = self.conn.prepare(
            "SELECT id, workspace_id, title FROM conversations WHERE workspace_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![workspace_id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
            })
        })?;
        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(row?);
        }
        Ok(conversations)
    }

    pub fn rename_conversation(
        &self,
        conversation_id: i64,
        title: &str,
    ) -> Result<(), WorkspaceError> {
        let updated = self.conn.execute(
            "UPDATE conversations SET title = ?1 WHERE id = ?2",
            params![title, conversation_id],
        )?;
        if updated == 0 {
            return Err(WorkspaceError::NotFound(format!(
                "conversation {conversation_id}"
            )));
        }
        Ok(())
    }

    pub fn delete_conversation(&self, conversation_id: i64) -> Result<(), WorkspaceError> {
        let deleted = self.conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![conversation_id],
        )?;
        if deleted == 0 {
            return Err(WorkspaceError::NotFound(format!(
                "conversation {conversation_id}"
            )));
        }
        Ok(())
    }

    pub fn save_message(
        &self,
        conversation_id: i64,
        role: &str,
        content: &str,
        images: Option<&str>,
        tool_calls: Option<&str>,
    ) -> Result<i64, WorkspaceError> {
        self.conn.execute(
            "INSERT INTO messages (conversation_id, role, content, images, tool_calls) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![conversation_id, role, content, images, tool_calls],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn load_messages(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<ConversationMessage>, WorkspaceError> {
        let mut statement = self.conn.prepare(
            "SELECT id, role, content, images, tool_calls, timestamp FROM messages WHERE conversation_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![conversation_id], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                images: row.get(3)?,
                tool_calls: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn search_messages(&self, query: &str) -> Result<Vec<SearchHit>, WorkspaceError> {
        let mut statement = self.conn.prepare(
            "SELECT rowid, conversation_id, role, content FROM messages_fts WHERE messages_fts MATCH ?1 ORDER BY rank",
        )?;
        let rows = statement.query_map(params![query], |row| {
            Ok(SearchHit {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
            })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok(hits)
    }

    pub fn upsert_checkpoint(
        &self,
        conversation_id: i64,
        name: &str,
        snapshot_json: &str,
    ) -> Result<(), WorkspaceError> {
        self.conn.execute(
            "INSERT INTO checkpoints (conversation_id, name, snapshot_json) VALUES (?1, ?2, ?3)
             ON CONFLICT(conversation_id, name) DO UPDATE SET snapshot_json = excluded.snapshot_json, created_at = CURRENT_TIMESTAMP",
            params![conversation_id, name, snapshot_json],
        )?;
        Ok(())
    }

    pub fn get_checkpoint(
        &self,
        conversation_id: i64,
        name: &str,
    ) -> Result<Option<Checkpoint>, WorkspaceError> {
        self.conn
            .query_row(
                "SELECT id, conversation_id, name, snapshot_json FROM checkpoints WHERE conversation_id = ?1 AND name = ?2",
                params![conversation_id, name],
                |row| {
                    Ok(Checkpoint {
                        id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        name: row.get(2)?,
                        snapshot_json: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(WorkspaceError::from)
    }

    pub fn upsert_context_file(&self, meta: &ContextFileMeta) -> Result<(), WorkspaceError> {
        self.conn.execute(
            "INSERT INTO context_files (workspace_id, path, hash, size_bytes) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(workspace_id, path) DO UPDATE SET hash = excluded.hash, size_bytes = excluded.size_bytes, updated_at = CURRENT_TIMESTAMP",
            params![meta.workspace_id, meta.path, meta.hash, meta.size_bytes as i64],
        )?;
        Ok(())
    }

    pub fn get_context_file(
        &self,
        workspace_id: i64,
        path: &str,
    ) -> Result<Option<ContextFileMeta>, WorkspaceError> {
        self.conn
            .query_row(
                "SELECT workspace_id, path, hash, size_bytes FROM context_files WHERE workspace_id = ?1 AND path = ?2",
                params![workspace_id, path],
                |row| {
                    Ok(ContextFileMeta {
                        workspace_id: row.get(0)?,
                        path: row.get(1)?,
                        hash: row.get(2)?,
                        size_bytes: row.get::<_, i64>(3)? as usize,
                    })
                },
            )
            .optional()
            .map_err(WorkspaceError::from)
    }

    pub fn update_workspace_context_paths(
        &self,
        workspace_id: i64,
        context_paths: &[String],
    ) -> Result<(), WorkspaceError> {
        let serialized = serde_json::to_string(context_paths)?;
        self.conn.execute(
            "UPDATE workspaces SET context_paths = ?1 WHERE id = ?2",
            params![serialized, workspace_id],
        )?;
        Ok(())
    }
}
