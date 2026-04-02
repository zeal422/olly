use chrono::Local;

use super::{state::App, Message};

impl App {
    pub(crate) fn now_label() -> String {
        Local::now().format("%H:%M").to_string()
    }

    pub(crate) fn normalize_storage_timestamp(raw: &str) -> String {
        raw.get(11..16).unwrap_or(raw).to_string()
    }

    fn push_message(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        image_count: usize,
        tool_calls_preview: Option<String>,
    ) {
        self.clear_chat_selection();
        self.messages.push(Message {
            role: role.into(),
            content: content.into(),
            timestamp: Self::now_label(),
            image_count,
            tool_calls_preview,
            tool_name: None,
            tool_calls: None,
        });
        self.chat_scroll = 0;
    }

    pub(crate) fn push_user(&mut self, content: String, image_count: usize) {
        self.push_message("You", content, image_count, None);
    }

    pub(crate) fn push_assistant(&mut self, content: String, tool_calls_preview: Option<String>) {
        self.push_message("Assistant", content, 0, tool_calls_preview);
    }

    pub(crate) fn push_tool(&mut self, content: String, tool_name: Option<String>) {
        self.clear_chat_selection();
        let tool_calls_preview = tool_name.as_ref().map(|name| format!("Tool: {}", name));
        self.messages.push(Message {
            role: "Tool".to_string(),
            content,
            timestamp: Self::now_label(),
            image_count: 0,
            tool_calls_preview,
            tool_name,
            tool_calls: None,
        });
        self.chat_scroll = 0;
    }

    pub(crate) fn push_system(&mut self, content: String) {
        self.push_message("System", content, 0, None);
    }

    #[allow(dead_code)]
    pub(crate) fn push_tool_notice(&mut self, tool_name: &str, step: usize, total_steps: usize) {
        let message = format!(
            "Executing tool: {} (step {}/{})...",
            tool_name, step, total_steps
        );
        self.push_system(message);
    }

    #[allow(dead_code)]
    pub(crate) fn push_tool_result_notice(&mut self, tool_name: &str, success: bool, step: usize) {
        let status = if success { "completed" } else { "failed" };
        let message = format!("Tool {}: {} (step {})", tool_name, status, step);
        self.push_system(message);
    }
}
