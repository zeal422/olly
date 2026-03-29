pub mod context;
pub mod error;
pub mod manager;
pub mod storage;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub context_paths: Vec<String>,
    pub system_prompt: Option<String>,
    pub model_preset_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub images: Option<String>,
    pub tool_calls: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: i64,
    pub workspace_id: i64,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSession {
    pub workspace_id: i64,
    pub conversation_id: i64,
    pub preset_id: i64,
}

#[derive(Debug, Clone)]
pub struct ContextFileMeta {
    pub workspace_id: i64,
    pub path: String,
    pub hash: String,
    pub size_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub message_id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub id: i64,
    pub conversation_id: i64,
    pub name: String,
    pub snapshot_json: String,
}
use serde::{Deserialize, Serialize};
