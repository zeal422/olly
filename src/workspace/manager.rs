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
        ContextFileMeta, ConversationMessage, SearchHit, Workspace, WorkspaceSession,
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
        let workspace_id =
            if let Some(existing) = self.storage.get_workspace_by_name(workspace_name)? {
                existing.id
            } else {
                self.storage
                    .create_workspace(workspace_name, preset_id, None)?
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
    ) -> Result<WorkspaceSession, WorkspaceError> {
        let preset_id = self
            .storage
            .get_or_create_preset(&format!("{name} Preset"), default_model)?;
        let workspace_id = self.storage.create_workspace(name, preset_id, None)?;
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
        let session = self.create_workspace(&workspace_name, default_model)?;
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
