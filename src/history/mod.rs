use crate::workspace::{error::WorkspaceError, manager::WorkspaceManager, ConversationMessage};

pub fn load_conversation(
    manager: &WorkspaceManager,
    conversation_id: i64,
) -> Result<Vec<ConversationMessage>, WorkspaceError> {
    manager.load_messages(conversation_id)
}

pub fn append_message(
    manager: &WorkspaceManager,
    conversation_id: i64,
    role: &str,
    content: &str,
) -> Result<i64, WorkspaceError> {
    manager.save_message(conversation_id, role, content)
}

pub fn append_message_with_meta(
    manager: &WorkspaceManager,
    conversation_id: i64,
    role: &str,
    content: &str,
    images: Option<&str>,
    tool_calls: Option<&str>,
) -> Result<i64, WorkspaceError> {
    manager.save_message_with_meta(conversation_id, role, content, images, tool_calls)
}
