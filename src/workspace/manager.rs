use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    app::context::is_sensitive_or_excluded,
    workspace::{
        context::{file_meta_for_workspace, has_hash_changed, read_file},
        error::WorkspaceError,
        storage::Storage,
        ContextFileMeta, Conversation, ConversationMessage, SearchHit, Workspace, WorkspaceSession,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportMessage {
    role: String,
    content: String,
    timestamp: String,
    images: Option<String>,
    tool_calls: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportConversation {
    title: Option<String>,
    messages: Vec<ExportMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportWorkspace {
    version: u32,
    name: String,
    #[serde(default)]
    root_path: Option<String>,
    context_paths: Vec<String>,
    conversations: Vec<ExportConversation>,
}

pub struct WorkspaceManager {
    storage: Storage,
}

impl WorkspaceManager {
    pub fn open(db_path: &Path) -> Result<Self, WorkspaceError> {
        Ok(Self {
            storage: Storage::open(db_path)?,
        })
    }

    pub fn bootstrap_default(
        &self,
        workspace_name: &str,
        default_model: &str,
    ) -> Result<WorkspaceSession, WorkspaceError> {
        let preset_id = self
            .storage
            .get_or_create_preset("Default", default_model)?;
        let default_root_path = std::env::current_dir()
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        let workspace_id =
            if let Some(existing) = self.storage.get_workspace_by_name(workspace_name)? {
                if existing.root_path.is_none() {
                    self.storage
                        .update_workspace_root_path(existing.id, default_root_path.as_deref())?;
                }
                existing.id
            } else {
                self.storage.create_workspace(
                    workspace_name,
                    preset_id,
                    default_root_path.as_deref(),
                    None,
                )?
            };
        let conversation_id =
            if let Some(existing) = self.storage.latest_conversation(workspace_id)? {
                existing.id
            } else {
                self.storage
                    .create_conversation(workspace_id, Some("Main Conversation"))?
            };
        Ok(WorkspaceSession {
            workspace_id,
            conversation_id,
            preset_id,
        })
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, WorkspaceError> {
        self.storage.list_workspaces()
    }

    pub fn open_workspace_session(
        &self,
        workspace_id: i64,
        default_model: &str,
    ) -> Result<WorkspaceSession, WorkspaceError> {
        let workspace = self
            .storage
            .get_workspace_by_id(workspace_id)?
            .ok_or_else(|| WorkspaceError::NotFound(format!("workspace {workspace_id}")))?;
        let conversation_id =
            if let Some(existing) = self.storage.latest_conversation(workspace_id)? {
                existing.id
            } else {
                self.storage
                    .create_conversation(workspace_id, Some("Main Conversation"))?
            };
        let preset_id = if workspace.model_preset_id > 0 {
            workspace.model_preset_id
        } else {
            self.storage
                .get_or_create_preset(&format!("{} Preset", workspace.name), default_model)?
        };
        Ok(WorkspaceSession {
            workspace_id,
            conversation_id,
            preset_id,
        })
    }

    pub fn create_workspace(
        &self,
        name: &str,
        default_model: &str,
        root_path: Option<&Path>,
    ) -> Result<WorkspaceSession, WorkspaceError> {
        let preset_id = self
            .storage
            .get_or_create_preset(&format!("{name} Preset"), default_model)?;
        let normalized_root = if let Some(path) = root_path {
            Some(normalize_root_path(path)?)
        } else {
            None
        };
        let workspace_id =
            self.storage
                .create_workspace(name, preset_id, normalized_root.as_deref(), None)?;
        let conversation_id = self
            .storage
            .create_conversation(workspace_id, Some("Main Conversation"))?;
        Ok(WorkspaceSession {
            workspace_id,
            conversation_id,
            preset_id,
        })
    }

    pub fn delete_workspace(&self, workspace_id: i64) -> Result<(), WorkspaceError> {
        self.storage.delete_workspace(workspace_id)
    }

    pub fn rename_workspace(&self, workspace_id: i64, name: &str) -> Result<(), WorkspaceError> {
        self.storage.rename_workspace(workspace_id, name)
    }

    pub fn set_workspace_root_path(
        &self,
        workspace_id: i64,
        root_path: &Path,
    ) -> Result<String, WorkspaceError> {
        let normalized = normalize_root_path(root_path)?;
        self.storage
            .update_workspace_root_path(workspace_id, Some(&normalized))?;
        Ok(normalized)
    }

    pub fn list_conversations(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<Conversation>, WorkspaceError> {
        self.storage.list_conversations(workspace_id)
    }

    pub fn create_session(
        &self,
        workspace_id: i64,
        title: Option<&str>,
    ) -> Result<i64, WorkspaceError> {
        self.storage.create_conversation(workspace_id, title)
    }

    pub fn rename_session(&self, conversation_id: i64, title: &str) -> Result<(), WorkspaceError> {
        self.storage.rename_conversation(conversation_id, title)
    }

    pub fn delete_session(&self, conversation_id: i64) -> Result<(), WorkspaceError> {
        self.storage.delete_conversation(conversation_id)
    }

    pub fn save_message(
        &self,
        conversation_id: i64,
        role: &str,
        content: &str,
    ) -> Result<i64, WorkspaceError> {
        self.save_message_with_meta(conversation_id, role, content, None, None)
    }

    pub fn save_message_with_meta(
        &self,
        conversation_id: i64,
        role: &str,
        content: &str,
        images: Option<&str>,
        tool_calls: Option<&str>,
    ) -> Result<i64, WorkspaceError> {
        self.storage
            .save_message(conversation_id, role, content, images, tool_calls)
    }

    pub fn load_messages(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<ConversationMessage>, WorkspaceError> {
        self.storage.load_messages(conversation_id)
    }

    pub fn search_messages(&self, query: &str) -> Result<Vec<SearchHit>, WorkspaceError> {
        self.storage.search_messages(query)
    }

    pub fn add_context_file(
        &self,
        workspace_id: i64,
        path: &Path,
    ) -> Result<ContextFileMeta, WorkspaceError> {
        let path_string = path.to_string_lossy().into_owned();
        if is_sensitive_or_excluded(&path_string) {
            return Err(WorkspaceError::InvalidContextPath(path_string));
        }

        let content = read_file(path)?;
        let meta = file_meta_for_workspace(workspace_id, path, &content)?;
        if let Some(existing) = self.storage.get_context_file(workspace_id, &meta.path)? {
            if !has_hash_changed(&existing.hash, &meta.hash) {
                return Ok(existing);
            }
        }
        self.storage.upsert_context_file(&meta)?;

        let mut workspaces = self.storage.list_workspaces()?;
        if let Some(workspace) = workspaces.iter_mut().find(|w| w.id == workspace_id) {
            if !workspace.context_paths.iter().any(|p| p == &meta.path) {
                workspace.context_paths.push(meta.path.clone());
                self.storage
                    .update_workspace_context_paths(workspace_id, &workspace.context_paths)?;
            }
        }

        Ok(meta)
    }

    pub fn save_checkpoint(&self, conversation_id: i64, name: &str) -> Result<(), WorkspaceError> {
        let messages = self.storage.load_messages(conversation_id)?;
        let export_messages = messages
            .into_iter()
            .map(|m| ExportMessage {
                role: m.role,
                content: m.content,
                timestamp: m.timestamp,
                images: m.images,
                tool_calls: m.tool_calls,
            })
            .collect::<Vec<_>>();
        let snapshot = serde_json::to_string(&export_messages)?;
        self.storage
            .upsert_checkpoint(conversation_id, name, &snapshot)
    }

    pub fn restore_checkpoint(
        &self,
        workspace_id: i64,
        conversation_id: i64,
        name: &str,
    ) -> Result<i64, WorkspaceError> {
        let checkpoint = self
            .storage
            .get_checkpoint(conversation_id, name)?
            .ok_or_else(|| WorkspaceError::NotFound(format!("checkpoint {name}")))?;
        let messages: Vec<ExportMessage> = serde_json::from_str(&checkpoint.snapshot_json)?;
        let new_conversation_id = self
            .storage
            .create_conversation(workspace_id, Some(&format!("Checkpoint: {name}")))?;
        for message in messages {
            self.storage.save_message(
                new_conversation_id,
                &message.role,
                &message.content,
                message.images.as_deref(),
                message.tool_calls.as_deref(),
            )?;
        }
        Ok(new_conversation_id)
    }

    pub fn export_conversation_json(
        &self,
        conversation_id: i64,
        output_path: &Path,
    ) -> Result<(), WorkspaceError> {
        let messages = self.storage.load_messages(conversation_id)?;
        let serialized = serde_json::to_string_pretty(&messages)?;
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, serialized)?;
        Ok(())
    }

    pub fn export_conversation_markdown(
        &self,
        conversation_id: i64,
        output_path: &Path,
    ) -> Result<(), WorkspaceError> {
        let messages = self.storage.load_messages(conversation_id)?;
        let mut out = String::new();
        for message in messages {
            out.push_str(&format!(
                "## {} [{}]\n\n{}\n\n",
                message.role.to_uppercase(),
                message.timestamp,
                message.content
            ));
            if let Some(images) = message.images {
                out.push_str(&format!("Images: {images}\n\n"));
            }
            if let Some(tool_calls) = message.tool_calls {
                out.push_str(&format!("Tool calls: {tool_calls}\n\n"));
            }
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, out)?;
        Ok(())
    }

    pub fn export_workspace_json(
        &self,
        workspace_id: i64,
        output_path: &Path,
    ) -> Result<(), WorkspaceError> {
        let workspace = self
            .storage
            .get_workspace_by_id(workspace_id)?
            .ok_or_else(|| WorkspaceError::NotFound("workspace".into()))?;
        let conversations = self.storage.list_conversations(workspace_id)?;

        let mut exported_conversations = Vec::new();
        for conversation in conversations {
            let messages = self.storage.load_messages(conversation.id)?;
            let exported_messages = messages
                .into_iter()
                .map(|m| ExportMessage {
                    role: m.role,
                    content: m.content,
                    timestamp: m.timestamp,
                    images: m.images,
                    tool_calls: m.tool_calls,
                })
                .collect::<Vec<_>>();
            exported_conversations.push(ExportConversation {
                title: conversation.title,
                messages: exported_messages,
            });
        }

        let payload = ExportWorkspace {
            version: 1,
            name: workspace.name,
            root_path: workspace.root_path,
            context_paths: workspace.context_paths,
            conversations: exported_conversations,
        };
        let raw = serde_json::to_string_pretty(&payload)?;
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, raw)?;
        Ok(())
    }

    pub fn import_workspace_json(
        &self,
        input_path: &Path,
        default_model: &str,
    ) -> Result<WorkspaceSession, WorkspaceError> {
        let raw = fs::read_to_string(input_path)?;
        let payload: ExportWorkspace = serde_json::from_str(&raw)?;

        let workspace_name = dedupe_workspace_name(&self.storage.list_workspaces()?, &payload.name);
        let root_path = payload
            .root_path
            .as_ref()
            .map(PathBuf::from)
            .and_then(|candidate| {
                let absolute = if candidate.is_absolute() {
                    candidate
                } else {
                    std::env::current_dir().ok()?.join(candidate)
                };
                if absolute.is_dir() {
                    Some(absolute)
                } else {
                    None
                }
            });
        let session =
            self.create_workspace(&workspace_name, default_model, root_path.as_deref())?;
        for context_path in payload.context_paths {
            let _ = self.add_context_file(session.workspace_id, &PathBuf::from(context_path));
        }

        let mut active_conversation = session.conversation_id;
        for (index, conversation) in payload.conversations.into_iter().enumerate() {
            let conversation_id = if index == 0 {
                active_conversation
            } else {
                self.storage
                    .create_conversation(session.workspace_id, conversation.title.as_deref())?
            };
            for message in conversation.messages {
                self.storage.save_message(
                    conversation_id,
                    &message.role,
                    &message.content,
                    message.images.as_deref(),
                    message.tool_calls.as_deref(),
                )?;
            }
            if index == 0 {
                active_conversation = conversation_id;
                // Apply the imported title to the pre-created conversation (BUG #7)
                if let Some(title) = conversation.title {
                    self.storage.rename_conversation(conversation_id, &title)?;
                }
            }
        }

        Ok(WorkspaceSession {
            workspace_id: session.workspace_id,
            conversation_id: active_conversation,
            preset_id: session.preset_id,
        })
    }
}

fn dedupe_workspace_name(existing: &[Workspace], desired: &str) -> String {
    if !existing.iter().any(|w| w.name == desired) {
        return desired.to_string();
    }
    let mut idx = 2_u32;
    loop {
        let candidate = format!("{desired} ({idx})");
        if !existing.iter().any(|w| w.name == candidate) {
            return candidate;
        }
        idx += 1;
    }
}

fn normalize_root_path(path: &Path) -> Result<String, WorkspaceError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if !absolute.exists() {
        return Err(WorkspaceError::NotFound(format!(
            "workspace root path not found: {}",
            absolute.display()
        )));
    }
    if !absolute.is_dir() {
        return Err(WorkspaceError::NotFound(format!(
            "workspace root path is not a directory: {}",
            absolute.display()
        )));
    }
    let canonical = std::fs::canonicalize(absolute)?;
    Ok(canonical.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn import_applies_first_conversation_title() {
        // BUG #7 regression test: first imported conversation title should be applied
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let manager = WorkspaceManager::open(&db_path).unwrap();

        // Build an export payload to import (must use private ExportWorkspace/ExportConversation)
        let payload = ExportWorkspace {
            version: 1,
            name: "Imported".to_string(),
            root_path: None,
            context_paths: Vec::new(),
            conversations: vec![
                ExportConversation {
                    title: Some("First Conversation".to_string()),
                    messages: Vec::new(),
                },
                ExportConversation {
                    title: Some("Second Conversation".to_string()),
                    messages: Vec::new(),
                },
            ],
        };

        // Write payload to a temp file (simulate import from file)
        let import_file = temp_dir.path().join("import.json");
        let raw = serde_json::to_string_pretty(&payload).unwrap();
        std::fs::write(&import_file, raw).unwrap();

        // Import creates a new workspace from the payload
        let result = manager
            .import_workspace_json(&import_file, "default-model")
            .unwrap();

        // Verify: should have 2 conversations in the imported workspace
        let convs = manager.list_conversations(result.workspace_id).unwrap();
        assert_eq!(convs.len(), 2);

        // The first conversation (which reused the pre-created one) should now have the imported title
        let first = convs
            .iter()
            .find(|c| c.title.as_deref() == Some("First Conversation"))
            .expect("first conversation should have imported title");

        // The second conversation should have its title as well
        let second = convs
            .iter()
            .find(|c| c.title.as_deref() == Some("Second Conversation"))
            .expect("second conversation should have its title");

        // The active conversation from import result should be the first one
        assert_eq!(first.id, result.conversation_id);
        assert_eq!(second.id, result.conversation_id + 1);
    }
}
