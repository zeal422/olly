pub mod context;
pub mod input;
pub mod tui;

use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Local;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use serde::{Deserialize, Serialize};

use crate::{
    config::AppConfig,
    history,
    ollama::{
        self,
        chat::{ChatMessage, ChatToolCall, ParsedStreamChunk},
    },
    workspace::manager::WorkspaceManager,
};

const COMMAND_SUGGESTIONS: &[&str] = &[
    "/commands",
    "/models",
    "/session",
    "/search",
    "/export",
    "/workspace",
    "/checkpoint",
    "/attach",
    "/clear-attachments",
    "/model",
    "/url",
];
const AUTO_SCROLL_TARGET: u16 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Unknown,
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Workspaces,
    Chat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PendingImage {
    pub path: String,
    pub base64: String,
}

#[derive(Debug, Clone)]
pub struct PendingWorkspaceDelete {
    pub id: i64,
    pub name: String,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct WorkspaceEntry {
    pub id: i64,
    pub name: String,
    pub root_path: Option<String>,
    pub sessions: Vec<SessionEntry>,
}

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub image_count: usize,
    pub tool_calls_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
struct UiRuntimeState {
    models_collapsed: bool,
    cloud_models_collapsed: bool,
    local_models_collapsed: bool,
    workspaces_collapsed: bool,
    collapsed_workspace_ids: Vec<i64>,
    sidebar_index: usize,
}

#[derive(Debug, Clone)]
pub struct App {
    pub input: String,
    pub messages: Vec<Message>,
    pub models: Vec<String>,
    pub workspaces: Vec<WorkspaceEntry>,
    pub sessions: Vec<SessionEntry>,
    pub sidebar_index: usize,
    pub models_collapsed: bool,
    pub cloud_models_collapsed: bool,
    pub local_models_collapsed: bool,
    pub workspaces_collapsed: bool,
    pub collapsed_workspace_ids: HashSet<i64>,
    pub model: String,
    pub url: String,
    pub active_workspace_id: i64,
    pub workspace_name: String,
    pub workspace_root_path: Option<String>,
    pub active_session_id: i64,
    pub active_session_title: String,
    pub status: String,
    pub connection_status: ConnectionStatus,
    pub focus: FocusPane,
    pub show_help: bool,
    pub toast: Option<Toast>,
    pub chat_scroll: u16,
    pub is_loading: bool,
    pub spinner_frame: usize,
    pub syntax_highlighting: bool,
    pub pending_images: Vec<PendingImage>,
    pub pending_workspace_delete: Option<PendingWorkspaceDelete>,
    pub command_suggestion_index: usize,
    pub quit_armed_until: Option<Instant>,
    should_quit: bool,
}

impl App {
    pub fn new(url: String, model: String) -> Self {
        Self {
            input: String::new(),
            messages: Vec::new(),
            models: Vec::new(),
            workspaces: Vec::new(),
            sessions: Vec::new(),
            sidebar_index: 0,
            models_collapsed: false,
            cloud_models_collapsed: false,
            local_models_collapsed: false,
            workspaces_collapsed: false,
            collapsed_workspace_ids: HashSet::new(),
            model,
            url,
            active_workspace_id: 0,
            workspace_name: String::from("Default Workspace"),
            workspace_root_path: None,
            active_session_id: 0,
            active_session_title: String::from("Session"),
            status: String::from("Ready"),
            connection_status: ConnectionStatus::Unknown,
            focus: FocusPane::Chat,
            show_help: false,
            toast: None,
            chat_scroll: AUTO_SCROLL_TARGET,
            is_loading: false,
            spinner_frame: 0,
            syntax_highlighting: true,
            pending_images: Vec::new(),
            pending_workspace_delete: None,
            command_suggestion_index: 0,
            quit_armed_until: None,
            should_quit: false,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn snapshot_ui_state(&self) -> UiRuntimeState {
        let mut collapsed_workspace_ids = self
            .collapsed_workspace_ids
            .iter()
            .copied()
            .collect::<Vec<_>>();
        collapsed_workspace_ids.sort_unstable();
        UiRuntimeState {
            models_collapsed: self.models_collapsed,
            cloud_models_collapsed: self.cloud_models_collapsed,
            local_models_collapsed: self.local_models_collapsed,
            workspaces_collapsed: self.workspaces_collapsed,
            collapsed_workspace_ids,
            sidebar_index: self.sidebar_index,
        }
    }

    fn apply_ui_state(&mut self, state: UiRuntimeState) {
        self.models_collapsed = state.models_collapsed;
        self.cloud_models_collapsed = state.cloud_models_collapsed;
        self.local_models_collapsed = state.local_models_collapsed;
        self.workspaces_collapsed = state.workspaces_collapsed;
        self.collapsed_workspace_ids = state.collapsed_workspace_ids.into_iter().collect();
        self.sidebar_index = state.sidebar_index;
    }

    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        if let Some(toast) = &self.toast {
            if Instant::now() > toast.expires_at {
                self.toast = None;
            }
        }
        if let Some(pending_delete) = &self.pending_workspace_delete {
            if Instant::now() > pending_delete.expires_at {
                self.pending_workspace_delete = None;
            }
        }
    }

    fn now_label() -> String {
        Local::now().format("%H:%M").to_string()
    }

    fn normalize_storage_timestamp(raw: &str) -> String {
        raw.get(11..16).unwrap_or(raw).to_string()
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn request_quit(&mut self) {
        let now = Instant::now();
        if let Some(until) = self.quit_armed_until {
            if now <= until {
                self.quit();
                return;
            }
        }
        self.quit_armed_until = Some(now + Duration::from_secs(2));
        self.set_toast(ToastLevel::Info, "Press Ctrl+Q again to quit");
    }

    fn push_message(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        image_count: usize,
        tool_calls_preview: Option<String>,
    ) {
        self.messages.push(Message {
            role: role.into(),
            content: content.into(),
            timestamp: Self::now_label(),
            image_count,
            tool_calls_preview,
        });
        self.chat_scroll = AUTO_SCROLL_TARGET;
    }

    fn push_user(&mut self, content: String, image_count: usize) {
        self.push_message("You", content, image_count, None);
    }

    fn push_assistant(&mut self, content: String, tool_calls_preview: Option<String>) {
        self.push_message("Assistant", content, 0, tool_calls_preview);
    }

    fn push_tool(&mut self, content: String, tool_calls_preview: Option<String>) {
        self.push_message("Tool", content, 0, tool_calls_preview);
    }

    fn push_system(&mut self, content: String) {
        self.push_message("System", content, 0, None);
    }

    fn set_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(Toast {
            message: message.into(),
            level,
            expires_at: Instant::now() + Duration::from_secs(4),
        });
    }

    fn dismiss_overlay(&mut self) {
        if self.show_help {
            self.show_help = false;
            return;
        }
        self.toast = None;
    }

    fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    fn switch_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Workspaces => FocusPane::Chat,
            FocusPane::Chat => FocusPane::Workspaces,
        };
        let pane = match self.focus {
            FocusPane::Workspaces => "Workspaces",
            FocusPane::Chat => "Chat",
        };
        self.set_toast(ToastLevel::Info, format!("Focus: {pane}"));
    }

    fn scroll_up(&mut self, amount: u16) {
        self.chat_scroll = self.chat_scroll.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.chat_scroll = self
            .chat_scroll
            .saturating_add(amount)
            .min(AUTO_SCROLL_TARGET);
    }

    fn take_input_for_send(&mut self) -> Option<String> {
        let trimmed = self.input.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        self.input.clear();
        self.reset_command_selection();
        Some(trimmed)
    }

    fn reset_command_selection(&mut self) {
        self.command_suggestion_index = 0;
    }

    fn to_ollama_messages(&self) -> Vec<ChatMessage> {
        self.messages
            .iter()
            .filter_map(|m| {
                let role = match m.role.as_str() {
                    "You" => "user",
                    "Assistant" => "assistant",
                    "System" => "system",
                    "Tool" => "tool",
                    _ => return None,
                };
                Some(ChatMessage::new(role, m.content.clone()))
            })
            .collect()
    }

    fn from_storage_role(
        role: &str,
        content: String,
        images: Option<String>,
        tool_calls: Option<String>,
        timestamp: String,
    ) -> Option<Message> {
        let display_role = match role {
            "user" => "You",
            "assistant" => "Assistant",
            "system" => "System",
            "tool" => "Tool",
            _ => return None,
        };
        let image_count = images
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
            .map(|v| v.len())
            .unwrap_or(0);
        let tool_preview = tool_calls
            .as_ref()
            .and_then(|raw| serde_json::from_str::<Vec<ChatToolCall>>(raw).ok())
            .and_then(|calls| tool_calls_preview(&calls));
        Some(Message {
            role: display_role.to_string(),
            content,
            timestamp: Self::normalize_storage_timestamp(&timestamp),
            image_count,
            tool_calls_preview: tool_preview,
        })
    }

    fn refresh_workspaces(
        &mut self,
        manager: &WorkspaceManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.workspaces = manager
            .list_workspaces()?
            .into_iter()
            .map(|w| -> Result<WorkspaceEntry, Box<dyn std::error::Error>> {
                let sessions = manager
                    .list_conversations(w.id)?
                    .into_iter()
                    .map(|conversation| SessionEntry {
                        id: conversation.id,
                        title: conversation
                            .title
                            .unwrap_or_else(|| format!("Session {}", conversation.id)),
                    })
                    .collect::<Vec<_>>();
                Ok(WorkspaceEntry {
                    id: w.id,
                    name: w.name,
                    root_path: w.root_path,
                    sessions,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let existing_ids = self
            .workspaces
            .iter()
            .map(|workspace| workspace.id)
            .collect::<HashSet<_>>();
        self.collapsed_workspace_ids
            .retain(|workspace_id| existing_ids.contains(workspace_id));
        self.clamp_sidebar_index();
        Ok(())
    }

    fn sync_active_workspace_label(&mut self, workspace_id: i64) {
        if let Some(workspace) = self.workspaces.iter().find(|w| w.id == workspace_id) {
            self.active_workspace_id = workspace.id;
            self.workspace_name = workspace.name.clone();
            self.workspace_root_path = workspace.root_path.clone();
            self.sessions = workspace.sessions.clone();
        } else {
            self.active_workspace_id = 0;
            self.workspace_name = String::from("No Workspace");
            self.workspace_root_path = None;
            self.active_session_id = 0;
            self.active_session_title = String::from("No Session");
            self.sessions.clear();
        }
    }

    fn has_active_session(&self) -> bool {
        self.active_workspace_id > 0 && self.active_session_id > 0
    }

    fn clear_active_workspace_state(&mut self) {
        self.active_workspace_id = 0;
        self.workspace_name = String::from("No Workspace");
        self.workspace_root_path = None;
        self.active_session_id = 0;
        self.active_session_title = String::from("No Session");
        self.sessions.clear();
        self.messages.clear();
        self.status = String::from("No workspace active");
    }

    fn toggle_workspace_collapse(&mut self, workspace_id: i64) -> bool {
        if self.collapsed_workspace_ids.contains(&workspace_id) {
            self.collapsed_workspace_ids.remove(&workspace_id);
            false
        } else {
            self.collapsed_workspace_ids.insert(workspace_id);
            true
        }
    }

    fn refresh_sessions(
        &mut self,
        manager: &WorkspaceManager,
        workspace_id: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if workspace_id <= 0 {
            self.sessions.clear();
            self.active_session_id = 0;
            self.active_session_title = String::from("No Session");
            self.clamp_sidebar_index();
            return Ok(());
        }
        let sessions = manager
            .list_conversations(workspace_id)?
            .into_iter()
            .map(|conversation| SessionEntry {
                id: conversation.id,
                title: conversation
                    .title
                    .unwrap_or_else(|| format!("Session {}", conversation.id)),
            })
            .collect();
        self.sessions = sessions;
        if let Some(workspace) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            workspace.sessions = self.sessions.clone();
        }
        self.sync_active_session_label();
        self.clamp_sidebar_index();
        Ok(())
    }

    fn sync_active_session_label(&mut self) {
        if let Some(session) = self
            .sessions
            .iter()
            .find(|s| s.id == self.active_session_id)
        {
            self.active_session_title = session.title.clone();
        } else if let Some(latest) = self.sessions.last() {
            self.active_session_id = latest.id;
            self.active_session_title = latest.title.clone();
        } else {
            self.active_session_id = 0;
            self.active_session_title = String::from("No Session");
        }
    }

    fn clamp_sidebar_index(&mut self) {
        let total = self.sidebar_item_count();
        if total == 0 {
            self.sidebar_index = 0;
            return;
        }
        if self.sidebar_index >= total {
            self.sidebar_index = total - 1;
        }
    }

    fn sidebar_item_count(&self) -> usize {
        self.sidebar_entries().len()
    }

    pub fn sidebar_item(&self) -> Option<SidebarItem> {
        self.sidebar_entries().get(self.sidebar_index).copied()
    }

    fn model_indices_for_bucket(&self, bucket: ModelBucket) -> Vec<usize> {
        self.models
            .iter()
            .enumerate()
            .filter_map(|(index, model)| {
                let is_cloud = is_cloud_model_name(model);
                match bucket {
                    ModelBucket::Cloud if is_cloud => Some(index),
                    ModelBucket::Local if !is_cloud => Some(index),
                    _ => None,
                }
            })
            .collect()
    }

    pub fn model_count_for_bucket(&self, bucket: ModelBucket) -> usize {
        self.models
            .iter()
            .filter(|model| match bucket {
                ModelBucket::Cloud => is_cloud_model_name(model),
                ModelBucket::Local => !is_cloud_model_name(model),
            })
            .count()
    }

    pub fn sidebar_entries(&self) -> Vec<SidebarItem> {
        let mut entries = Vec::new();
        entries.push(SidebarItem::SectionHeader(SidebarSection::Models));
        if !self.models_collapsed {
            let cloud = self.model_indices_for_bucket(ModelBucket::Cloud);
            let local = self.model_indices_for_bucket(ModelBucket::Local);

            entries.push(SidebarItem::ModelGroupHeader(ModelBucket::Cloud));
            if !self.cloud_models_collapsed {
                entries.extend(cloud.into_iter().map(SidebarItem::Model));
            }

            entries.push(SidebarItem::ModelGroupHeader(ModelBucket::Local));
            if !self.local_models_collapsed {
                entries.extend(local.into_iter().map(SidebarItem::Model));
            }
        }
        entries.push(SidebarItem::SectionHeader(SidebarSection::Workspaces));
        if !self.workspaces_collapsed {
            entries.push(SidebarItem::WorkspaceCreate);
            for (workspace_idx, workspace) in self.workspaces.iter().enumerate() {
                entries.push(SidebarItem::Workspace(workspace_idx));
                if !self.collapsed_workspace_ids.contains(&workspace.id) {
                    entries.push(SidebarItem::WorkspaceSessionsLabel(workspace_idx));
                    for session_idx in 0..workspace.sessions.len() {
                        entries.push(SidebarItem::WorkspaceSession(workspace_idx, session_idx));
                    }
                }
            }
        }
        entries
    }

    fn sidebar_up(&mut self, amount: u16) {
        if self.sidebar_item_count() == 0 {
            return;
        }
        self.sidebar_index = self.sidebar_index.saturating_sub(amount as usize);
    }

    fn sidebar_down(&mut self, amount: u16) {
        if self.sidebar_item_count() == 0 {
            return;
        }
        let max = self.sidebar_item_count() - 1;
        self.sidebar_index = (self.sidebar_index + amount as usize).min(max);
    }

    fn focus_active_session(&mut self, workspace_id: i64, conversation_id: i64) {
        if workspace_id <= 0 || conversation_id <= 0 {
            self.clamp_sidebar_index();
            return;
        }
        self.collapsed_workspace_ids.remove(&workspace_id);
        if let Some(index) = self.sidebar_entries().iter().position(|item| match item {
            SidebarItem::WorkspaceSession(workspace_idx, session_idx) => self
                .workspaces
                .get(*workspace_idx)
                .and_then(|workspace| {
                    workspace
                        .sessions
                        .get(*session_idx)
                        .map(|session| (workspace.id, session.id))
                })
                .map(|(wid, sid)| wid == workspace_id && sid == conversation_id)
                .unwrap_or(false),
            _ => false,
        }) {
            self.sidebar_index = index;
            return;
        }
        self.clamp_sidebar_index();
    }

    fn activate_sidebar_selection(
        &mut self,
        manager: &WorkspaceManager,
        session: &mut crate::workspace::WorkspaceSession,
        default_model: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.sidebar_item() {
            Some(SidebarItem::SectionHeader(SidebarSection::Models)) => {
                self.models_collapsed = !self.models_collapsed;
                self.clamp_sidebar_index();
                let state = if self.models_collapsed {
                    "collapsed"
                } else {
                    "expanded"
                };
                self.set_toast(ToastLevel::Info, format!("Models {state}"));
            }
            Some(SidebarItem::ModelGroupHeader(bucket)) => {
                let (label, state) = match bucket {
                    ModelBucket::Cloud => {
                        self.cloud_models_collapsed = !self.cloud_models_collapsed;
                        (
                            "Cloud",
                            if self.cloud_models_collapsed {
                                "collapsed"
                            } else {
                                "expanded"
                            },
                        )
                    }
                    ModelBucket::Local => {
                        self.local_models_collapsed = !self.local_models_collapsed;
                        (
                            "Local",
                            if self.local_models_collapsed {
                                "collapsed"
                            } else {
                                "expanded"
                            },
                        )
                    }
                };
                self.clamp_sidebar_index();
                self.set_toast(ToastLevel::Info, format!("{label} models {state}"));
            }
            Some(SidebarItem::SectionHeader(SidebarSection::Workspaces)) => {
                self.workspaces_collapsed = !self.workspaces_collapsed;
                self.clamp_sidebar_index();
                let state = if self.workspaces_collapsed {
                    "collapsed"
                } else {
                    "expanded"
                };
                self.set_toast(ToastLevel::Info, format!("Workspaces {state}"));
            }
            Some(SidebarItem::Model(model_index)) => {
                if let Some(model) = self.models.get(model_index).cloned() {
                    if model == self.model {
                        self.set_toast(ToastLevel::Info, "Model already active");
                    } else {
                        self.model = model;
                        self.status = String::from("Model switched from sidebar");
                        self.set_toast(ToastLevel::Success, "Model switched");
                    }
                }
            }
            Some(SidebarItem::WorkspaceCreate) => {
                self.focus = FocusPane::Chat;
                self.input = String::from("/workspace create New Workspace --path ");
                self.reset_command_selection();
                self.set_toast(
                    ToastLevel::Info,
                    "Set folder path, then press Enter (Tab autocomplete works)",
                );
            }
            Some(SidebarItem::Workspace(workspace_index)) => {
                if let Some(workspace) = self.workspaces.get(workspace_index).cloned() {
                    if workspace.id == session.workspace_id {
                        let collapsed = self.toggle_workspace_collapse(workspace.id);
                        let state = if collapsed { "collapsed" } else { "expanded" };
                        self.set_toast(
                            ToastLevel::Info,
                            format!("Workspace sessions {state}: {}", workspace.name),
                        );
                        return Ok(());
                    }

                    let next_session =
                        manager.open_workspace_session(workspace.id, default_model)?;
                    session.workspace_id = next_session.workspace_id;
                    session.conversation_id = next_session.conversation_id;
                    session.preset_id = next_session.preset_id;
                    self.active_workspace_id = session.workspace_id;
                    self.workspace_name = workspace.name;
                    self.workspace_root_path = workspace.root_path;
                    self.active_session_id = session.conversation_id;
                    self.refresh_workspaces(manager)?;
                    self.refresh_sessions(manager, session.workspace_id)?;
                    self.focus_active_session(session.workspace_id, session.conversation_id);
                    load_conversation_into_app(self, manager, session.conversation_id)?;
                    self.status = String::from("Workspace switched from sidebar");
                    self.set_toast(ToastLevel::Success, "Workspace switched");
                }
            }
            Some(SidebarItem::WorkspaceSessionsLabel(workspace_index)) => {
                if let Some(workspace) = self.workspaces.get(workspace_index).cloned() {
                    let collapsed = self.toggle_workspace_collapse(workspace.id);
                    let state = if collapsed { "collapsed" } else { "expanded" };
                    self.set_toast(
                        ToastLevel::Info,
                        format!("Workspace sessions {state}: {}", workspace.name),
                    );
                }
            }
            Some(SidebarItem::WorkspaceSession(workspace_index, session_index)) => {
                let target = self.workspaces.get(workspace_index).and_then(|workspace| {
                    workspace
                        .sessions
                        .get(session_index)
                        .cloned()
                        .map(|session_entry| (workspace.clone(), session_entry))
                });
                if let Some((target_workspace, target_session)) = target {
                    if target_workspace.id == session.workspace_id
                        && target_session.id == session.conversation_id
                    {
                        self.set_toast(ToastLevel::Info, "Session already active");
                        return Ok(());
                    }

                    if target_workspace.id != session.workspace_id {
                        let next =
                            manager.open_workspace_session(target_workspace.id, default_model)?;
                        session.workspace_id = next.workspace_id;
                        session.preset_id = next.preset_id;
                        self.active_workspace_id = session.workspace_id;
                        self.workspace_name = target_workspace.name;
                        self.workspace_root_path = target_workspace.root_path;
                    }

                    session.conversation_id = target_session.id;
                    self.active_session_id = target_session.id;
                    self.active_session_title = target_session.title;
                    self.refresh_workspaces(manager)?;
                    self.refresh_sessions(manager, session.workspace_id)?;
                    load_conversation_into_app(self, manager, session.conversation_id)?;
                    self.status = String::from("Session switched from sidebar");
                    self.set_toast(ToastLevel::Success, "Session switched");
                }
            }
            None => {}
        }
        Ok(())
    }

    fn autocomplete_command(&mut self) {
        let suggestions =
            autocomplete_suggestions(&self.input, &self.models, &self.workspaces, &self.sessions);
        if suggestions.is_empty() {
            return;
        }
        let index = self
            .command_suggestion_index
            .min(suggestions.len().saturating_sub(1));
        if let Some(suggestion) = suggestions.get(index) {
            self.input = suggestion.clone();
            self.command_suggestion_index = (index + 1) % suggestions.len();
        }
    }

    fn autocomplete_command_prev(&mut self) {
        let suggestions =
            autocomplete_suggestions(&self.input, &self.models, &self.workspaces, &self.sessions);
        if suggestions.is_empty() {
            return;
        }
        if self.command_suggestion_index == 0 {
            self.command_suggestion_index = suggestions.len().saturating_sub(1);
        } else {
            self.command_suggestion_index -= 1;
        }
    }

    fn autocomplete_command_next(&mut self) {
        let suggestions =
            autocomplete_suggestions(&self.input, &self.models, &self.workspaces, &self.sessions);
        if suggestions.is_empty() {
            return;
        }
        self.command_suggestion_index = (self.command_suggestion_index + 1) % suggestions.len();
    }

    fn is_command_suggestion_mode(&self) -> bool {
        self.is_slash_command_input() && !self.autocomplete_options().is_empty()
    }

    fn is_slash_command_input(&self) -> bool {
        self.input.trim_start().starts_with('/') && !self.input.contains('\n')
    }

    pub fn autocomplete_preview(&self) -> Option<String> {
        let suggestions =
            autocomplete_suggestions(&self.input, &self.models, &self.workspaces, &self.sessions);
        if suggestions.is_empty() {
            return None;
        }
        let index = self
            .command_suggestion_index
            .min(suggestions.len().saturating_sub(1));
        suggestions.get(index).cloned()
    }

    pub fn autocomplete_options(&self) -> Vec<String> {
        autocomplete_suggestions(&self.input, &self.models, &self.workspaces, &self.sessions)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSection {
    Models,
    Workspaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelBucket {
    Cloud,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItem {
    SectionHeader(SidebarSection),
    ModelGroupHeader(ModelBucket),
    Model(usize),
    WorkspaceCreate,
    Workspace(usize),
    WorkspaceSessionsLabel(usize),
    WorkspaceSession(usize, usize),
}

fn is_cloud_model_name(model: &str) -> bool {
    let value = model.trim().to_ascii_lowercase();
    value.contains(":cloud") || value.ends_with("-cloud") || value.contains("-cloud:")
}

fn autocomplete_suggestions(
    input: &str,
    models: &[String],
    workspaces: &[WorkspaceEntry],
    sessions: &[SessionEntry],
) -> Vec<String> {
    let raw = input.trim_start();
    if !raw.starts_with('/') || raw.contains('\n') {
        return Vec::new();
    }
    let token = raw.trim_end();

    if token == "/models" {
        if raw.ends_with(' ') {
            return vec![
                String::from("/models refresh"),
                String::from("/models add "),
                String::from("/models remove "),
            ];
        }
        return vec![String::from("/models ")];
    }

    if let Some(rest_raw) = raw.strip_prefix("/models ") {
        let rest = rest_raw.trim_start();
        if rest.is_empty() {
            return vec![
                String::from("/models refresh"),
                String::from("/models add "),
                String::from("/models remove "),
            ];
        }
        let parts = rest.split_whitespace().collect::<Vec<_>>();
        let sub = parts.first().copied().unwrap_or_default();
        let subcommands = ["refresh", "add", "remove"];
        let has_sub_arg = rest.contains(' ');
        if !has_sub_arg {
            return subcommands
                .into_iter()
                .filter(|candidate| candidate.starts_with(sub))
                .map(|candidate| match candidate {
                    "refresh" => String::from("/models refresh"),
                    _ => format!("/models {candidate} "),
                })
                .collect();
        }
        if sub == "remove" {
            let arg_partial = rest[sub.len()..].trim();
            return models
                .iter()
                .filter(|model| {
                    arg_partial.is_empty()
                        || model
                            .to_lowercase()
                            .starts_with(&arg_partial.to_lowercase())
                })
                .map(|model| format!("/models remove {model}"))
                .collect();
        }
    }

    if token == "/model" || token == "/mod" {
        if raw.ends_with(' ') {
            return models
                .iter()
                .map(|model| format!("/model {model}"))
                .collect();
        }
        return vec![String::from("/model ")];
    }

    if let Some(partial) = raw
        .strip_prefix("/model ")
        .or_else(|| raw.strip_prefix("/mod "))
    {
        let partial = partial.trim();
        if partial.is_empty() {
            return models
                .iter()
                .map(|model| format!("/model {model}"))
                .collect();
        }
        return models
            .iter()
            .filter(|model| model.starts_with(partial))
            .map(|found| format!("/model {found}"))
            .collect();
    }

    if token == "/workspace" {
        if raw.ends_with(' ') {
            return workspace_subcommand_suggestions();
        }
        return vec![String::from("/workspace ")];
    }

    if token == "/workspaces" {
        return vec![String::from("/workspace ")];
    }

    if let Some(rest_raw) = raw.strip_prefix("/workspace ") {
        let rest = rest_raw.trim_start();
        if rest.is_empty() {
            return workspace_subcommand_suggestions();
        }

        let parts = rest.split_whitespace().collect::<Vec<_>>();
        let sub = parts.first().copied().unwrap_or_default();
        let subcommands = [
            "list", "switch", "create", "rename", "delete", "export", "import", "where", "set-path",
        ];
        let has_sub_arg = rest.contains(' ');

        if !has_sub_arg {
            return subcommands
                .into_iter()
                .filter(|candidate| candidate.starts_with(sub))
                .map(|candidate| match candidate {
                    "list" | "where" => format!("/workspace {candidate}"),
                    _ => format!("/workspace {candidate} "),
                })
                .collect();
        }

        if sub == "switch" || sub == "delete" {
            let arg_partial = rest[sub.len()..].trim();
            if sub == "delete" && arg_partial.is_empty() {
                return vec![
                    String::from("/workspace delete"),
                    String::from("/workspace delete --confirm"),
                    String::from("/workspace delete --cancel"),
                ];
            }
            let mut suggestions = Vec::new();
            if sub == "delete" {
                suggestions.push(String::from("/workspace delete --confirm "));
                suggestions.push(String::from("/workspace delete --cancel"));
            }
            suggestions.extend(workspaces.iter().flat_map(|workspace| {
                let by_id = format!("/workspace {sub} {}", workspace.id);
                let by_name = format!("/workspace {sub} {}", workspace.name);
                [by_id, by_name]
            }));
            let mut suggestions = suggestions
                .into_iter()
                .filter(|candidate| {
                    if arg_partial.is_empty() {
                        true
                    } else {
                        candidate.to_lowercase().starts_with(&format!(
                            "/workspace {sub} {}",
                            arg_partial.to_lowercase()
                        ))
                    }
                })
                .collect::<Vec<_>>();
            suggestions.sort();
            suggestions.dedup();
            return suggestions;
        }

        if sub == "rename" {
            let arg_partial = rest[sub.len()..].trim();
            if arg_partial.is_empty() {
                return vec![String::from("/workspace rename ")];
            }
            return vec![format!("/workspace rename {arg_partial}")];
        }

        if sub == "set-path" {
            let arg_partial = rest[sub.len()..].trim();
            if let Some((selector_part, path_partial)) = arg_partial.split_once(" --path ") {
                let command_prefix =
                    format!("/workspace set-path {} --path ", selector_part.trim());
                let path_suggestions = filesystem_directory_suggestions(path_partial);
                if path_suggestions.is_empty() {
                    return vec![command_prefix];
                }
                return path_suggestions
                    .into_iter()
                    .map(|path| format!("{command_prefix}{path}"))
                    .collect();
            }
            let mut suggestions = workspaces
                .iter()
                .flat_map(|workspace| {
                    let by_id = format!("/workspace set-path {} --path ", workspace.id);
                    let by_name = format!("/workspace set-path {} --path ", workspace.name);
                    [by_id, by_name]
                })
                .filter(|candidate| {
                    if arg_partial.is_empty() {
                        true
                    } else {
                        candidate.to_lowercase().starts_with(&format!(
                            "/workspace set-path {}",
                            arg_partial.to_lowercase()
                        ))
                    }
                })
                .collect::<Vec<_>>();
            suggestions.sort();
            suggestions.dedup();
            return suggestions;
        }

        if sub == "create" {
            let arg_partial = rest[sub.len()..].trim();
            if let Some((name_part, path_partial)) = arg_partial.split_once(" --path ") {
                let command_prefix = format!("/workspace create {} --path ", name_part.trim());
                let path_suggestions = filesystem_directory_suggestions(path_partial);
                if path_suggestions.is_empty() {
                    return vec![command_prefix];
                }
                return path_suggestions
                    .into_iter()
                    .map(|path| format!("{command_prefix}{path}"))
                    .collect();
            }
            if !arg_partial.is_empty() && !arg_partial.contains(" --path ") {
                return vec![format!("/workspace create {arg_partial} --path ")];
            }
        }
    }

    if token == "/session" {
        if raw.ends_with(' ') {
            return session_subcommand_suggestions();
        }
        return vec![String::from("/session ")];
    }

    if let Some(rest_raw) = raw.strip_prefix("/session ") {
        let rest = rest_raw.trim_start();
        if rest.is_empty() {
            return session_subcommand_suggestions();
        }

        let parts = rest.split_whitespace().collect::<Vec<_>>();
        let sub = parts.first().copied().unwrap_or_default();
        let subcommands = ["list", "new", "create", "switch", "rename", "delete"];
        let has_sub_arg = rest.contains(' ');

        if !has_sub_arg {
            return subcommands
                .into_iter()
                .filter(|candidate| candidate.starts_with(sub))
                .map(|candidate| match candidate {
                    "list" => String::from("/session list"),
                    _ => format!("/session {candidate} "),
                })
                .collect();
        }

        if sub == "switch" {
            let arg_partial = rest[sub.len()..].trim();
            let mut suggestions = sessions
                .iter()
                .flat_map(|session| {
                    let by_id = format!("/session switch {}", session.id);
                    let by_name = format!("/session switch {}", session.title);
                    [by_id, by_name]
                })
                .filter(|candidate| {
                    if arg_partial.is_empty() {
                        true
                    } else {
                        candidate
                            .to_lowercase()
                            .starts_with(&format!("/session switch {}", arg_partial.to_lowercase()))
                    }
                })
                .collect::<Vec<_>>();
            suggestions.sort();
            suggestions.dedup();
            return suggestions;
        }

        if sub == "rename" {
            let arg_partial = rest[sub.len()..].trim();
            let mut suggestions = sessions
                .iter()
                .flat_map(|session| {
                    let by_id = format!("/session rename {} ", session.id);
                    let by_name = format!("/session rename {} --to ", session.title);
                    [by_id, by_name]
                })
                .filter(|candidate| {
                    if arg_partial.is_empty() {
                        true
                    } else {
                        candidate
                            .to_lowercase()
                            .starts_with(&format!("/session rename {}", arg_partial.to_lowercase()))
                    }
                })
                .collect::<Vec<_>>();
            suggestions.sort();
            suggestions.dedup();
            return suggestions;
        }

        if sub == "delete" {
            let arg_partial = rest[sub.len()..].trim();
            let mut suggestions = sessions
                .iter()
                .flat_map(|session| {
                    let by_id = format!("/session delete {}", session.id);
                    let by_name = format!("/session delete {}", session.title);
                    [by_id, by_name]
                })
                .filter(|candidate| {
                    if arg_partial.is_empty() {
                        true
                    } else {
                        candidate
                            .to_lowercase()
                            .starts_with(&format!("/session delete {}", arg_partial.to_lowercase()))
                    }
                })
                .collect::<Vec<_>>();
            suggestions.sort();
            suggestions.dedup();
            if suggestions.is_empty() {
                suggestions.push(String::from("/session delete"));
            }
            return suggestions;
        }
    }

    if token.contains(' ') {
        return Vec::new();
    }

    COMMAND_SUGGESTIONS
        .iter()
        .find(|candidate| candidate.starts_with(token))
        .map(|found| vec![format!("{found} ")])
        .unwrap_or_default()
}

fn workspace_subcommand_suggestions() -> Vec<String> {
    vec![
        String::from("/workspace list"),
        String::from("/workspace switch "),
        String::from("/workspace create "),
        String::from("/workspace rename "),
        String::from("/workspace delete "),
        String::from("/workspace where"),
        String::from("/workspace set-path "),
        String::from("/workspace export "),
        String::from("/workspace import "),
    ]
}

fn session_subcommand_suggestions() -> Vec<String> {
    vec![
        String::from("/session list"),
        String::from("/session new "),
        String::from("/session create "),
        String::from("/session switch "),
        String::from("/session rename "),
        String::from("/session delete"),
    ]
}

#[cfg(test)]
fn next_autocomplete_value(current_input: &str, suggestions: &[String]) -> Option<String> {
    if suggestions.is_empty() {
        return None;
    }
    let normalized = current_input.trim();
    if let Some(index) = suggestions
        .iter()
        .position(|candidate| candidate.trim() == normalized)
    {
        let next = (index + 1) % suggestions.len();
        return Some(suggestions[next].clone());
    }
    Some(suggestions[0].clone())
}

fn filesystem_directory_suggestions(partial: &str) -> Vec<String> {
    let partial = partial.trim();
    let mut base = if partial.is_empty() {
        std::env::current_dir().ok()
    } else {
        let partial_path = PathBuf::from(partial);
        if partial.ends_with(std::path::MAIN_SEPARATOR)
            || partial.ends_with('/')
            || partial.ends_with('\\')
        {
            Some(partial_path)
        } else {
            partial_path.parent().map(Path::to_path_buf)
        }
    };

    let name_prefix = if partial.is_empty()
        || partial.ends_with(std::path::MAIN_SEPARATOR)
        || partial.ends_with('/')
        || partial.ends_with('\\')
    {
        String::new()
    } else {
        PathBuf::from(partial)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_default()
    };

    if let Some(path) = base.clone() {
        if !path.is_absolute() {
            if let Ok(cwd) = std::env::current_dir() {
                base = Some(cwd.join(path));
            }
        }
    }

    let Some(base_dir) = base else {
        return Vec::new();
    };
    if !base_dir.exists() || !base_dir.is_dir() {
        return Vec::new();
    }

    let lower_prefix = name_prefix.to_lowercase();
    let mut suggestions = match std::fs::read_dir(&base_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                let name = path.file_name()?.to_str()?;
                if !lower_prefix.is_empty() && !name.to_lowercase().starts_with(&lower_prefix) {
                    return None;
                }
                Some(path.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    suggestions.sort();
    suggestions
}

fn ui_state_path_for_config(config: &AppConfig) -> PathBuf {
    Path::new(&config.storage.db_path)
        .parent()
        .map(|dir| dir.join("ui_state.json"))
        .unwrap_or_else(|| crate::config::default_data_dir().join("ui_state.json"))
}

fn load_ui_state_from_path(path: &Path) -> Option<UiRuntimeState> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<UiRuntimeState>(&raw).ok()
}

fn save_ui_state_to_path(path: &Path, state: &UiRuntimeState) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(state)
        .map_err(|err| io::Error::other(format!("serialize ui state: {err}")))?;
    fs::write(path, raw)
}

pub async fn run(
    url_override: Option<String>,
    model_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_or_create()?;
    let ui_state_path = ui_state_path_for_config(&config);
    let url = url_override.unwrap_or_else(|| config.ollama.url.clone());
    let model = model_override.unwrap_or_else(|| config.defaults.model.clone());
    let mut app = App::new(url, model);
    app.workspace_name = config.defaults.workspace.clone();
    app.syntax_highlighting = config.ui.syntax_highlighting;
    if let Some(saved_ui_state) = load_ui_state_from_path(&ui_state_path) {
        app.apply_ui_state(saved_ui_state);
    }
    let input_bindings = input::InputBindings::from_config(&config.keybindings);

    let manager = WorkspaceManager::open(Path::new(&config.storage.db_path))?;
    let mut session = crate::workspace::WorkspaceSession {
        workspace_id: 0,
        conversation_id: 0,
        preset_id: 0,
    };

    app.refresh_workspaces(&manager)?;
    let initial_workspace = app
        .workspaces
        .iter()
        .find(|workspace| workspace.name == config.defaults.workspace)
        .cloned()
        .or_else(|| app.workspaces.first().cloned());
    if let Some(workspace) = initial_workspace {
        let next = manager.open_workspace_session(workspace.id, &config.defaults.model)?;
        session.workspace_id = next.workspace_id;
        session.conversation_id = next.conversation_id;
        session.preset_id = next.preset_id;
        app.active_workspace_id = session.workspace_id;
        app.active_session_id = session.conversation_id;
        load_conversation_into_app(&mut app, &manager, session.conversation_id)?;
        app.refresh_sessions(&manager, session.workspace_id)?;
        app.sync_active_workspace_label(session.workspace_id);
        app.focus_active_session(session.workspace_id, session.conversation_id);
    } else {
        app.clear_active_workspace_state();
    }

    let client = reqwest::Client::new();
    match ollama::models::list_models(&client, &app.url).await {
        Ok(models) => {
            app.connection_status = ConnectionStatus::Online;
            if !models.is_empty() {
                if !models.iter().any(|m| m == &app.model) {
                    app.model = models[0].clone();
                }
                app.models = models;
            }
        }
        Err(err) => {
            app.connection_status = ConnectionStatus::Offline;
            app.set_toast(ToastLevel::Error, format!("Model list unavailable: {err}"));
        }
    }

    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut last_saved_ui_state = app.snapshot_ui_state();
    let _ = save_ui_state_to_path(&ui_state_path, &last_saved_ui_state);

    let run_result = async {
        while !app.should_quit() {
            app.tick();
            let current_ui_state = app.snapshot_ui_state();
            if current_ui_state != last_saved_ui_state {
                let _ = save_ui_state_to_path(&ui_state_path, &current_ui_state);
                last_saved_ui_state = current_ui_state;
            }
            terminal.draw(|f| tui::draw(f, &app))?;
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    let action =
                        input::resolve_key_event_with_bindings(key, &input_bindings, &app.input);

                    if app.show_help {
                        match action {
                            input::InputAction::ToggleHelp | input::InputAction::DismissOverlay => {
                                app.show_help = false;
                            }
                            input::InputAction::Quit => app.request_quit(),
                            _ => {}
                        }
                        continue;
                    }

                    if app.pending_workspace_delete.is_some() {
                        match action {
                            input::InputAction::Quit => app.request_quit(),
                            input::InputAction::DismissOverlay => {
                                app.pending_workspace_delete = None;
                                app.set_toast(ToastLevel::Info, "Workspace delete canceled");
                            }
                            input::InputAction::NewLine | input::InputAction::Send => {
                                if let Err(err) = confirm_pending_workspace_delete(
                                    &mut app,
                                    &manager,
                                    &mut session,
                                    &config.defaults.model,
                                ) {
                                    app.push_system(format!("Workspace delete failed: {err}"));
                                    app.set_toast(ToastLevel::Error, "Workspace delete failed");
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match action {
                        input::InputAction::Quit => app.request_quit(),
                        input::InputAction::ToggleHelp => app.toggle_help(),
                        input::InputAction::DismissOverlay => {
                            if app.is_slash_command_input() && !app.input.trim().is_empty() {
                                app.input.clear();
                                app.reset_command_selection();
                            } else {
                                app.dismiss_overlay();
                            }
                        }
                        input::InputAction::SwitchFocus => app.switch_focus(),
                        input::InputAction::AutoCompleteCommand => {
                            if app.focus == FocusPane::Chat || app.is_slash_command_input() {
                                app.autocomplete_command();
                            }
                        }
                        input::InputAction::ScrollUp(amount) => {
                            if app.is_command_suggestion_mode() {
                                app.autocomplete_command_prev();
                            } else if app.focus == FocusPane::Workspaces {
                                app.sidebar_up(amount);
                            } else {
                                app.scroll_up(amount);
                            }
                        }
                        input::InputAction::ScrollDown(amount) => {
                            if app.is_command_suggestion_mode() {
                                app.autocomplete_command_next();
                            } else if app.focus == FocusPane::Workspaces {
                                app.sidebar_down(amount);
                            } else {
                                app.scroll_down(amount);
                            }
                        }
                        input::InputAction::Insert(c) => {
                            if app.focus == FocusPane::Chat {
                                app.input.push(c);
                                app.reset_command_selection();
                            } else if app.focus == FocusPane::Workspaces {
                                let can_start_command = app.input.is_empty() && c == '/';
                                let continue_command = app.is_slash_command_input();
                                if can_start_command || continue_command {
                                    app.input.push(c);
                                    app.reset_command_selection();
                                }
                            }
                        }
                        input::InputAction::Backspace => {
                            if app.focus == FocusPane::Chat
                                || (app.focus == FocusPane::Workspaces
                                    && app.is_slash_command_input())
                            {
                                app.input.pop();
                                app.reset_command_selection();
                            }
                        }
                        input::InputAction::NewLine => {
                            if app.focus == FocusPane::Workspaces && app.is_slash_command_input() {
                                if let Some(prompt) = app.take_input_for_send() {
                                    if let Err(err) = execute_slash_command(
                                        &mut app,
                                        &manager,
                                        &mut session,
                                        &config.defaults.model,
                                        &prompt,
                                    )
                                    .await
                                    {
                                        app.push_system(format!("Command failed: {err}"));
                                        app.set_toast(ToastLevel::Error, "Command failed");
                                    }
                                }
                            } else if app.focus == FocusPane::Workspaces {
                                if let Err(err) = app.activate_sidebar_selection(
                                    &manager,
                                    &mut session,
                                    &config.defaults.model,
                                ) {
                                    app.push_system(format!("Sidebar action failed: {err}"));
                                    app.set_toast(ToastLevel::Error, "Sidebar action failed");
                                }
                            } else if app.input.trim_start().starts_with('/')
                                && !app.input.contains('\n')
                            {
                                if let Some(prompt) = app.take_input_for_send() {
                                    if let Err(err) = execute_slash_command(
                                        &mut app,
                                        &manager,
                                        &mut session,
                                        &config.defaults.model,
                                        &prompt,
                                    )
                                    .await
                                    {
                                        app.push_system(format!("Command failed: {err}"));
                                        app.set_toast(ToastLevel::Error, "Command failed");
                                    }
                                }
                            } else {
                                app.input.push('\n');
                                app.reset_command_selection();
                            }
                        }
                        input::InputAction::Send => {
                            if app.focus != FocusPane::Chat && !app.is_slash_command_input() {
                                continue;
                            }
                            if let Some(prompt) = app.take_input_for_send() {
                                if prompt.starts_with('/') {
                                    if let Err(err) = execute_slash_command(
                                        &mut app,
                                        &manager,
                                        &mut session,
                                        &config.defaults.model,
                                        &prompt,
                                    )
                                    .await
                                    {
                                        app.push_system(format!("Command failed: {err}"));
                                        app.set_toast(ToastLevel::Error, "Command failed");
                                    }
                                    continue;
                                }

                                if !app.has_active_session() || session.conversation_id <= 0 {
                                    app.push_system(
                                        "No active workspace. Create one from the sidebar (+ New Workspace) or run `/workspace create ...`."
                                            .to_string(),
                                    );
                                    app.set_toast(
                                        ToastLevel::Info,
                                        "Create or switch to a workspace first",
                                    );
                                    continue;
                                }

                                let image_count = app.pending_images.len();
                                let images_base64 = app
                                    .pending_images
                                    .iter()
                                    .map(|i| i.base64.clone())
                                    .collect::<Vec<_>>();
                                let images_json = if images_base64.is_empty() {
                                    None
                                } else {
                                    Some(serde_json::to_string(&images_base64)?)
                                };

                                app.push_user(prompt.clone(), image_count);
                                if let Err(err) = history::append_message_with_meta(
                                    &manager,
                                    session.conversation_id,
                                    "user",
                                    &prompt,
                                    images_json.as_deref(),
                                    None,
                                ) {
                                    app.push_system(format!("Persist failed: {err}"));
                                    app.set_toast(ToastLevel::Error, "Failed to save user message");
                                }
                                let _ = terminal.draw(|f| tui::draw(f, &app));

                                let mut request_messages: Vec<ChatMessage> =
                                    app.to_ollama_messages();
                                if !images_base64.is_empty() {
                                    if let Some(last_user) =
                                        request_messages.iter_mut().rev().find(|m| m.role == "user")
                                    {
                                        last_user.images = Some(images_base64.clone());
                                    }
                                }
                                app.pending_images.clear();

                                app.push_assistant(String::new(), None);
                                let assistant_index = app.messages.len() - 1;
                                app.status = String::from("Receiving...");
                                app.is_loading = true;
                                let _ = terminal.draw(|f| tui::draw(f, &app));
                                let stream_url = app.url.clone();
                                let stream_model = app.model.clone();
                                let mut streamed_any = false;
                                let mut captured_tool_calls: Vec<ChatToolCall> = Vec::new();

                                let stream_result = ollama::chat::chat_stream(
                                    &client,
                                    &stream_url,
                                    &stream_model,
                                    &request_messages,
                                    |chunk: &ParsedStreamChunk| {
                                        if !chunk.content.is_empty() {
                                            streamed_any = true;
                                            {
                                                let message = &mut app.messages[assistant_index];
                                                message.content.push_str(&chunk.content);
                                            }
                                        }
                                        if !chunk.tool_calls.is_empty() {
                                            captured_tool_calls.extend(chunk.tool_calls.clone());
                                        }
                                        app.chat_scroll = AUTO_SCROLL_TARGET;
                                        let _ = terminal.draw(|f| tui::draw(f, &app));
                                    },
                                )
                                .await;

                                app.is_loading = false;
                                match stream_result {
                                    Ok(()) => {
                                        app.connection_status = ConnectionStatus::Online;
                                        if !streamed_any {
                                            app.messages[assistant_index].content =
                                                String::from("(no response content)");
                                        }
                                        let tool_calls_json = if captured_tool_calls.is_empty() {
                                            None
                                        } else {
                                            Some(serde_json::to_string(&captured_tool_calls)?)
                                        };
                                        app.messages[assistant_index].tool_calls_preview =
                                            tool_calls_preview(&captured_tool_calls);

                                        if let Err(err) = history::append_message_with_meta(
                                            &manager,
                                            session.conversation_id,
                                            "assistant",
                                            &app.messages[assistant_index].content,
                                            None,
                                            tool_calls_json.as_deref(),
                                        ) {
                                            app.push_system(format!("Persist failed: {err}"));
                                            app.set_toast(
                                                ToastLevel::Error,
                                                "Failed to save assistant message",
                                            );
                                        }

                                        if !captured_tool_calls.is_empty() {
                                            let tool_summary =
                                                format_tool_calls(&captured_tool_calls);
                                            app.push_tool(
                                                tool_summary.clone(),
                                                tool_calls_preview(&captured_tool_calls),
                                            );
                                            let tool_calls_json =
                                                serde_json::to_string(&captured_tool_calls)?;
                                            let _ = history::append_message_with_meta(
                                                &manager,
                                                session.conversation_id,
                                                "tool",
                                                &tool_summary,
                                                None,
                                                Some(&tool_calls_json),
                                            );
                                        }

                                        app.status = String::from("Ready");
                                    }
                                    Err(err) => {
                                        app.connection_status = ConnectionStatus::Offline;
                                        app.messages[assistant_index].content =
                                            String::from("(request failed)");
                                        app.push_system(format!("Request failed: {err}"));
                                        app.set_toast(ToastLevel::Error, "Request failed");
                                        app.status = String::from("Error");
                                    }
                                }
                            }
                        }
                        input::InputAction::Noop => {}
                    }
                }
            }
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;

    let ui_state = app.snapshot_ui_state();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    let _ = save_ui_state_to_path(&ui_state_path, &ui_state);

    run_result
}

fn load_conversation_into_app(
    app: &mut App,
    manager: &WorkspaceManager,
    conversation_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    app.messages.clear();
    if conversation_id <= 0 {
        return Ok(());
    }
    let persisted_messages = history::load_conversation(manager, conversation_id)?;
    for persisted in persisted_messages {
        if let Some(message) = App::from_storage_role(
            &persisted.role,
            persisted.content,
            persisted.images,
            persisted.tool_calls,
            persisted.timestamp,
        ) {
            app.messages.push(message);
        }
    }
    Ok(())
}

async fn execute_slash_command(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut crate::workspace::WorkspaceSession,
    default_model: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if prompt == "/models refresh" || prompt == "/model refresh" {
        let client = reqwest::Client::new();
        match ollama::models::list_models(&client, &app.url).await {
            Ok(models) => {
                app.connection_status = ConnectionStatus::Online;
                if !models.is_empty() {
                    app.models = models;
                    if !app.models.iter().any(|m| m == &app.model) {
                        app.model = app.models[0].clone();
                    }
                }
                app.set_toast(ToastLevel::Success, "Model list refreshed");
            }
            Err(err) => {
                app.connection_status = ConnectionStatus::Offline;
                app.push_system(format!("Model refresh failed: {err}"));
                app.set_toast(ToastLevel::Error, "Model refresh failed");
            }
        }
        return Ok(());
    }

    if let Some(raw_ref) = prompt.strip_prefix("/models add ") {
        let candidate = normalize_model_reference(raw_ref);
        if candidate.is_empty() {
            app.push_system(
                "Usage: /models add <model-name-or-ollama-library-url> (pull local or enable cloud)"
                    .to_string(),
            );
            return Ok(());
        }
        let client = reqwest::Client::new();
        match ollama::models::pull_model(&client, &app.url, &candidate).await {
            Ok(()) => match ollama::models::list_models(&client, &app.url).await {
                Ok(models) => {
                    app.connection_status = ConnectionStatus::Online;
                    app.models = models;
                    if !app.models.iter().any(|m| m == &app.model) {
                        if let Some(first) = app.models.first().cloned() {
                            app.model = first;
                        }
                    }
                    app.set_toast(ToastLevel::Success, format!("Model enabled: {candidate}"));
                }
                Err(err) => {
                    app.push_system(format!("Model enabled, but refresh failed: {err}"));
                    app.set_toast(ToastLevel::Info, "Model enabled; refresh failed");
                }
            },
            Err(err) => {
                app.connection_status = ConnectionStatus::Offline;
                app.push_system(format!("Model add failed: {err}"));
                app.set_toast(ToastLevel::Error, "Model add failed");
            }
        }
        return Ok(());
    }

    if let Some(raw_ref) = prompt.strip_prefix("/models remove ") {
        let candidate = normalize_model_reference(raw_ref);
        if candidate.is_empty() {
            app.push_system("Usage: /models remove <model-name>".to_string());
            return Ok(());
        }
        let client = reqwest::Client::new();
        match ollama::models::delete_model(&client, &app.url, &candidate).await {
            Ok(()) => match ollama::models::list_models(&client, &app.url).await {
                Ok(models) => {
                    app.connection_status = ConnectionStatus::Online;
                    app.models = models;
                    if app.model == candidate {
                        if let Some(first) = app.models.first().cloned() {
                            app.model = first;
                        }
                    }
                    app.set_toast(ToastLevel::Success, format!("Model removed: {candidate}"));
                }
                Err(err) => {
                    app.push_system(format!("Model removed, but refresh failed: {err}"));
                    app.set_toast(ToastLevel::Info, "Model removed; refresh failed");
                }
            },
            Err(err) => {
                app.connection_status = ConnectionStatus::Offline;
                app.push_system(format!("Model remove failed: {err}"));
                app.set_toast(ToastLevel::Error, "Model remove failed");
            }
        }
        return Ok(());
    }

    handle_command(app, manager, session, default_model, prompt)?;
    if let Err(err) = app.refresh_workspaces(manager) {
        app.push_system(format!("Workspace refresh failed: {err}"));
        app.set_toast(ToastLevel::Error, "Workspace refresh failed");
    } else {
        app.sync_active_workspace_label(session.workspace_id);
    }
    if let Err(err) = app.refresh_sessions(manager, session.workspace_id) {
        app.push_system(format!("Session refresh failed: {err}"));
        app.set_toast(ToastLevel::Error, "Session refresh failed");
    } else {
        app.focus_active_session(session.workspace_id, session.conversation_id);
    }
    Ok(())
}

fn confirm_pending_workspace_delete(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut crate::workspace::WorkspaceSession,
    default_model: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pending = app.pending_workspace_delete.clone();
    let Some(pending_delete) = pending else {
        return Ok(());
    };

    let workspaces = manager.list_workspaces()?;

    let target = workspaces
        .iter()
        .find(|workspace| workspace.id == pending_delete.id)
        .cloned();
    let Some(target_workspace) = target else {
        app.push_system("Workspace to delete was not found.".to_string());
        app.pending_workspace_delete = None;
        return Ok(());
    };

    manager.delete_workspace(target_workspace.id)?;
    app.pending_workspace_delete = None;
    app.refresh_workspaces(manager)?;

    if target_workspace.id == session.workspace_id {
        if let Some(next_workspace) = app.workspaces.first().cloned() {
            let next_session = manager.open_workspace_session(next_workspace.id, default_model)?;
            session.workspace_id = next_session.workspace_id;
            session.conversation_id = next_session.conversation_id;
            session.preset_id = next_session.preset_id;
            app.active_workspace_id = session.workspace_id;
            app.active_session_id = session.conversation_id;
            app.workspace_name = next_workspace.name;
            app.workspace_root_path = next_workspace.root_path;
            app.refresh_sessions(manager, session.workspace_id)?;
            app.sync_active_workspace_label(session.workspace_id);
            load_conversation_into_app(app, manager, session.conversation_id)?;
            app.focus_active_session(session.workspace_id, session.conversation_id);
            app.set_toast(ToastLevel::Success, "Workspace deleted and switched");
        } else {
            session.workspace_id = 0;
            session.conversation_id = 0;
            session.preset_id = 0;
            app.clear_active_workspace_state();
            app.set_toast(ToastLevel::Success, "Workspace deleted");
        }
    } else {
        app.refresh_sessions(manager, session.workspace_id)?;
        app.sync_active_workspace_label(session.workspace_id);
        app.focus_active_session(session.workspace_id, session.conversation_id);
        app.set_toast(ToastLevel::Success, "Workspace deleted");
    }
    Ok(())
}

fn workspace_help_text() -> String {
    [
        "Workspace commands:",
        "- /workspace list",
        "- /workspace switch <id|name>",
        "- /workspace create <name> --path <folder>",
        "- /workspace create --path <folder> (name defaults to New Workspace)",
        "- /workspace rename <new name> (renames selected/active workspace)",
        "- /workspace delete (opens modal for selected workspace in sidebar)",
        "- /workspace delete <id|name> (optional explicit target)",
        "- Confirm delete modal with Enter, cancel with Esc",
        "- Deleting the last workspace is allowed (you will see the welcome screen)",
        "- /workspace where",
        "- /workspace set-path <id|name> --path <folder>",
        "- /workspace export <path>",
        "- /workspace import <path>",
    ]
    .join("\n")
}

fn models_help_text() -> String {
    [
        "Model commands:",
        "- /models",
        "- /models refresh",
        "- /models add <model-name-or-ollama-library-url> (pull local or enable cloud)",
        "- /models remove <model-name>",
        "- /model <name> (switch active model)",
    ]
    .join("\n")
}

fn session_help_text() -> String {
    [
        "Session commands:",
        "- /session list",
        "- /session new <title>",
        "- /session create <title> (alias of new)",
        "- /session switch <id|title>",
        "- /session rename <id> <new title>",
        "- /session rename <title> --to <new title>",
        "- /session delete [id|title] (defaults to selected/active)",
    ]
    .join("\n")
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .map(|chunk| chunk.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
}

fn next_workspace_name(existing: &[crate::workspace::Workspace], base: &str) -> String {
    if !existing
        .iter()
        .any(|workspace| workspace.name.eq_ignore_ascii_case(base))
    {
        return base.to_string();
    }

    let mut index = 2_u32;
    loop {
        let candidate = format!("{base} {index}");
        if !existing
            .iter()
            .any(|workspace| workspace.name.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
        index += 1;
    }
}

fn normalize_model_reference(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return String::new();
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    let without_suffix = trimmed
        .split(['?', '#'])
        .next()
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if let Some(index) = without_suffix.find("/library/") {
        let tail = &without_suffix[index + "/library/".len()..];
        if let Some(segment) = tail.split('/').find(|part| !part.is_empty()) {
            return segment.to_string();
        }
    }
    trimmed.to_string()
}

fn handle_command(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut crate::workspace::WorkspaceSession,
    default_model: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parts = prompt.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(());
    }

    match parts[0] {
        "/commands" => {
            app.push_system(format!("Commands: {}", COMMAND_SUGGESTIONS.join(", ")));
        }
        "/models" => {
            if parts.len() > 1 {
                app.push_system(models_help_text());
                return Ok(());
            }
            if app.models.is_empty() {
                app.push_system(format!("No models cached yet.\n\n{}", models_help_text()));
            } else {
                let lines = app
                    .models
                    .iter()
                    .map(|model| {
                        if model == &app.model {
                            format!("* {model} (active)")
                        } else {
                            format!("- {model}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                app.push_system(format!("Loaded models:\n{lines}\n\n{}", models_help_text()));
            }
        }
        "/search" => {
            let query = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if query.trim().is_empty() {
                app.push_system("Usage: /search <query>".to_string());
            } else {
                let hits = manager.search_messages(&query)?;
                if hits.is_empty() {
                    app.push_system("No matches found.".to_string());
                } else {
                    let preview = hits
                        .into_iter()
                        .take(8)
                        .map(|h| format!("#{} {}: {}", h.conversation_id, h.role, h.content))
                        .collect::<Vec<_>>()
                        .join("\n");
                    app.push_system(format!("Search results:\n{preview}"));
                }
            }
        }
        "/export" => {
            if !app.has_active_session() || session.conversation_id <= 0 {
                app.push_system(
                    "No active session to export. Create or switch to a workspace first."
                        .to_string(),
                );
                return Ok(());
            }
            if parts.len() < 3 {
                app.push_system("Usage: /export <json|md> <path>".to_string());
            } else {
                let format = parts[1];
                let path = PathBuf::from(parts[2..].join(" "));
                match format {
                    "json" => manager.export_conversation_json(session.conversation_id, &path)?,
                    "md" | "markdown" => {
                        manager.export_conversation_markdown(session.conversation_id, &path)?
                    }
                    _ => {
                        app.push_system("Unknown export format. Use json or md.".to_string());
                        return Ok(());
                    }
                }
                app.set_toast(
                    ToastLevel::Success,
                    format!("Conversation exported to {}", path.display()),
                );
            }
        }
        "/workspaces" => {
            app.push_system("Use `/workspace list`.".to_string());
        }
        "/workspace" => {
            if parts.len() < 2 {
                app.push_system(workspace_help_text());
            } else {
                let sub = parts[1];
                match sub {
                    "list" => {
                        let workspaces = manager.list_workspaces()?;
                        if workspaces.is_empty() {
                            app.push_system("No workspaces found.".to_string());
                        } else {
                            let list = workspaces
                                .into_iter()
                                .map(|workspace| {
                                    let path_label =
                                        workspace.root_path.as_deref().unwrap_or("(path not set)");
                                    if workspace.id == session.workspace_id {
                                        format!(
                                            "* {} (id: {}, active, path: {})",
                                            workspace.name, workspace.id, path_label
                                        )
                                    } else {
                                        format!(
                                            "- {} (id: {}, path: {})",
                                            workspace.name, workspace.id, path_label
                                        )
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            app.push_system(format!(
                                "Workspaces:\n{list}\n\nUse `/workspace switch <id|name>` or `/workspace where`."
                            ));
                        }
                    }
                    "switch" => {
                        if parts.len() < 3 {
                            app.push_system("Usage: /workspace switch <id|name>".to_string());
                            return Ok(());
                        }
                        let selector = parts[2..].join(" ");
                        let workspaces = manager.list_workspaces()?;
                        let target = if let Ok(id) = selector.parse::<i64>() {
                            workspaces.into_iter().find(|w| w.id == id)
                        } else {
                            workspaces
                                .into_iter()
                                .find(|w| w.name.eq_ignore_ascii_case(&selector))
                        };
                        if let Some(workspace) = target {
                            let next =
                                manager.open_workspace_session(workspace.id, default_model)?;
                            session.workspace_id = next.workspace_id;
                            session.conversation_id = next.conversation_id;
                            session.preset_id = next.preset_id;
                            app.active_workspace_id = session.workspace_id;
                            app.active_session_id = session.conversation_id;
                            app.workspace_name = workspace.name;
                            app.workspace_root_path = workspace.root_path;
                            load_conversation_into_app(app, manager, session.conversation_id)?;
                            app.focus_active_session(session.workspace_id, session.conversation_id);
                            app.set_toast(ToastLevel::Success, "Workspace switched");
                        } else {
                            app.push_system(format!(
                                "Workspace not found: {selector}. Use `/workspace list`."
                            ));
                        }
                    }
                    "create" => {
                        let raw_create = prompt
                            .strip_prefix("/workspace create")
                            .map(str::trim)
                            .unwrap_or_default();
                        if raw_create.is_empty() {
                            app.push_system(
                                "Usage: /workspace create <name> --path <folder>\n   or: /workspace create --path <folder>"
                                    .to_string(),
                            );
                            return Ok(());
                        }

                        let (name_part, root_path_raw) =
                            if let Some((left, right)) = raw_create.split_once(" --path ") {
                                (left.trim(), Some(right.trim()))
                            } else {
                                (raw_create, None)
                            };

                        let explicit_path = root_path_raw.is_some();
                        let root_path = if let Some(path_raw) = root_path_raw {
                            if path_raw.is_empty() {
                                app.push_system(
                                    "Workspace path cannot be empty after `--path`.".to_string(),
                                );
                                return Ok(());
                            }
                            PathBuf::from(path_raw)
                        } else {
                            std::env::current_dir()?
                        };

                        let existing_workspaces = manager.list_workspaces()?;
                        let mut name = name_part.trim().to_string();
                        if name.is_empty() {
                            name = next_workspace_name(&existing_workspaces, "New Workspace");
                        }
                        if name.is_empty() {
                            app.push_system("Workspace name cannot be empty.".to_string());
                            return Ok(());
                        }
                        let existing = existing_workspaces
                            .into_iter()
                            .find(|w| w.name.eq_ignore_ascii_case(&name));
                        if existing.is_some() {
                            app.push_system(format!(
                                "Workspace `{name}` already exists. Use `/workspace switch {name}`."
                            ));
                            return Ok(());
                        }
                        let created =
                            manager.create_workspace(&name, default_model, Some(&root_path))?;
                        session.workspace_id = created.workspace_id;
                        session.conversation_id = created.conversation_id;
                        session.preset_id = created.preset_id;
                        app.active_workspace_id = session.workspace_id;
                        app.active_session_id = session.conversation_id;
                        app.workspace_name = name.clone();
                        app.workspace_root_path = Some(root_path.to_string_lossy().into_owned());
                        load_conversation_into_app(app, manager, session.conversation_id)?;
                        app.focus_active_session(session.workspace_id, session.conversation_id);
                        app.refresh_workspaces(manager)?;
                        app.refresh_sessions(manager, session.workspace_id)?;
                        if !explicit_path {
                            app.push_system(format!(
                                "No `--path` provided, using current directory: {}",
                                root_path.display()
                            ));
                        }
                        app.set_toast(ToastLevel::Success, format!("Workspace `{name}` created"));
                    }
                    "rename" => {
                        let new_name_raw = prompt
                            .strip_prefix("/workspace rename")
                            .map(str::trim)
                            .unwrap_or_default();
                        let new_name = new_name_raw
                            .strip_prefix("--to")
                            .map(str::trim)
                            .unwrap_or(new_name_raw)
                            .trim();
                        if new_name.is_empty() {
                            app.push_system(
                                "Usage: /workspace rename <new name>\nRenames selected workspace in sidebar, otherwise active workspace."
                                    .to_string(),
                            );
                            return Ok(());
                        }

                        let workspaces = manager.list_workspaces()?;
                        if workspaces.is_empty() {
                            app.push_system(
                                "No workspaces found. Create one with `/workspace create --path <folder>`."
                                    .to_string(),
                            );
                            return Ok(());
                        }

                        let selected_workspace_id = match app.sidebar_item() {
                            Some(SidebarItem::Workspace(index))
                            | Some(SidebarItem::WorkspaceSessionsLabel(index))
                            | Some(SidebarItem::WorkspaceSession(index, _)) => {
                                app.workspaces.get(index).map(|workspace| workspace.id)
                            }
                            _ => (session.workspace_id > 0).then_some(session.workspace_id),
                        };
                        let target = selected_workspace_id
                            .and_then(|id| workspaces.iter().find(|workspace| workspace.id == id))
                            .cloned();
                        let Some(target_workspace) = target else {
                            app.push_system(
                                "No workspace selected. Select one in sidebar, then run `/workspace rename <new name>`."
                                    .to_string(),
                            );
                            return Ok(());
                        };

                        if target_workspace.name.eq_ignore_ascii_case(new_name) {
                            app.set_toast(ToastLevel::Info, "Workspace name unchanged");
                            return Ok(());
                        }

                        if workspaces.iter().any(|workspace| {
                            workspace.id != target_workspace.id
                                && workspace.name.eq_ignore_ascii_case(new_name)
                        }) {
                            app.push_system(format!(
                                "Workspace `{new_name}` already exists. Choose another name."
                            ));
                            return Ok(());
                        }

                        manager.rename_workspace(target_workspace.id, new_name)?;
                        if target_workspace.id == session.workspace_id {
                            app.workspace_name = new_name.to_string();
                        }
                        app.refresh_workspaces(manager)?;
                        app.sync_active_workspace_label(session.workspace_id);
                        app.set_toast(
                            ToastLevel::Success,
                            format!(
                                "Workspace renamed: {} -> {}",
                                target_workspace.name, new_name
                            ),
                        );
                    }
                    "delete" => {
                        let delete_args = parts.get(2..).unwrap_or(&[]);
                        if matches!(delete_args.first(), Some(&"--cancel")) {
                            if app.pending_workspace_delete.take().is_some() {
                                app.set_toast(ToastLevel::Info, "Workspace delete canceled");
                            } else {
                                app.push_system("No pending workspace delete.".to_string());
                            }
                            return Ok(());
                        }
                        if matches!(delete_args.first(), Some(&"--confirm")) {
                            return confirm_pending_workspace_delete(
                                app,
                                manager,
                                session,
                                default_model,
                            );
                        }

                        let workspaces = manager.list_workspaces()?;
                        if workspaces.is_empty() {
                            app.push_system(
                                "No workspaces to delete. Create one from the sidebar (+ New Workspace)."
                                    .to_string(),
                            );
                            return Ok(());
                        }

                        let target = if delete_args.is_empty() {
                            let selected_workspace_id = match app.sidebar_item() {
                                Some(SidebarItem::Workspace(index))
                                | Some(SidebarItem::WorkspaceSessionsLabel(index))
                                | Some(SidebarItem::WorkspaceSession(index, _)) => {
                                    app.workspaces.get(index).map(|workspace| workspace.id)
                                }
                                _ => Some(session.workspace_id),
                            };
                            selected_workspace_id
                                .and_then(|id| workspaces.iter().find(|w| w.id == id).cloned())
                        } else {
                            let selector = delete_args.join(" ");
                            if let Ok(id) = selector.trim().parse::<i64>() {
                                workspaces.iter().find(|w| w.id == id).cloned()
                            } else {
                                workspaces
                                    .iter()
                                    .find(|w| w.name.eq_ignore_ascii_case(selector.trim()))
                                    .cloned()
                            }
                        };
                        let Some(target_workspace) = target else {
                            app.push_system(
                                "Workspace not found for delete selection.".to_string(),
                            );
                            return Ok(());
                        };

                        app.pending_workspace_delete = Some(PendingWorkspaceDelete {
                            id: target_workspace.id,
                            name: target_workspace.name.clone(),
                            expires_at: Instant::now() + Duration::from_secs(45),
                        });
                        app.set_toast(
                            ToastLevel::Info,
                            "Delete modal opened (Enter confirm, Esc cancel)",
                        );
                    }
                    "where" => {
                        if session.workspace_id <= 0 || app.active_workspace_id <= 0 {
                            app.push_system(
                                "No active workspace. Create one from the sidebar (+ New Workspace)."
                                    .to_string(),
                            );
                            return Ok(());
                        }
                        let path = app
                            .workspace_root_path
                            .as_deref()
                            .unwrap_or("(path not set)");
                        app.push_system(format!(
                            "Active workspace: {} (id: {})\nPath: {}",
                            app.workspace_name, session.workspace_id, path
                        ));
                    }
                    "set-path" => {
                        let raw_set = prompt
                            .strip_prefix("/workspace set-path")
                            .map(str::trim)
                            .unwrap_or_default();
                        if raw_set.is_empty() {
                            app.push_system(
                                "Usage: /workspace set-path <id|name> --path <folder>".to_string(),
                            );
                            return Ok(());
                        }
                        let Some((selector_raw, path_raw)) = raw_set.split_once(" --path ") else {
                            app.push_system(
                                "Usage: /workspace set-path <id|name> --path <folder>".to_string(),
                            );
                            return Ok(());
                        };

                        let selector = selector_raw.trim();
                        let path_value = path_raw.trim();
                        if selector.is_empty() || path_value.is_empty() {
                            app.push_system(
                                "Usage: /workspace set-path <id|name> --path <folder>".to_string(),
                            );
                            return Ok(());
                        }

                        let workspaces = manager.list_workspaces()?;
                        let target = if let Ok(id) = selector.parse::<i64>() {
                            workspaces.into_iter().find(|w| w.id == id)
                        } else {
                            workspaces
                                .into_iter()
                                .find(|w| w.name.eq_ignore_ascii_case(selector))
                        };
                        let Some(workspace) = target else {
                            app.push_system(format!(
                                "Workspace not found: {selector}. Use `/workspace list`."
                            ));
                            return Ok(());
                        };

                        let normalized = manager
                            .set_workspace_root_path(workspace.id, &PathBuf::from(path_value))?;
                        if workspace.id == session.workspace_id {
                            app.workspace_root_path = Some(normalized.clone());
                        }
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Workspace path updated: {normalized}"),
                        );
                    }
                    "export" => {
                        if parts.len() < 3 {
                            app.push_system("Usage: /workspace export <path>".to_string());
                            return Ok(());
                        }
                        let path = PathBuf::from(parts[2..].join(" "));
                        manager.export_workspace_json(session.workspace_id, &path)?;
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Workspace exported to {}", path.display()),
                        );
                    }
                    "import" => {
                        if parts.len() < 3 {
                            app.push_system("Usage: /workspace import <path>".to_string());
                            return Ok(());
                        }
                        let path = PathBuf::from(parts[2..].join(" "));
                        let imported = manager.import_workspace_json(&path, default_model)?;
                        session.workspace_id = imported.workspace_id;
                        session.conversation_id = imported.conversation_id;
                        session.preset_id = imported.preset_id;
                        app.active_workspace_id = session.workspace_id;
                        app.active_session_id = session.conversation_id;
                        if let Some(workspace) = manager
                            .list_workspaces()?
                            .into_iter()
                            .find(|w| w.id == session.workspace_id)
                        {
                            app.workspace_name = workspace.name;
                            app.workspace_root_path = workspace.root_path;
                        }
                        load_conversation_into_app(app, manager, session.conversation_id)?;
                        app.focus_active_session(session.workspace_id, session.conversation_id);
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Workspace imported from {}", path.display()),
                        );
                    }
                    _ => app.push_system(workspace_help_text()),
                }
            }
        }
        "/session" => {
            if session.workspace_id <= 0 {
                app.push_system(
                    "No active workspace. Create one from the sidebar (+ New Workspace) or run `/workspace create ...`."
                        .to_string(),
                );
                return Ok(());
            }
            if parts.len() < 2 {
                app.push_system(session_help_text());
            } else {
                match parts[1] {
                    "list" => {
                        let sessions = manager.list_conversations(session.workspace_id)?;
                        if sessions.is_empty() {
                            app.push_system("No sessions found in this workspace.".to_string());
                        } else {
                            let list = sessions
                                .into_iter()
                                .map(|conversation| {
                                    let title = conversation
                                        .title
                                        .unwrap_or_else(|| format!("Session {}", conversation.id));
                                    if conversation.id == session.conversation_id {
                                        format!("* {} (id: {}, active)", title, conversation.id)
                                    } else {
                                        format!("- {} (id: {})", title, conversation.id)
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            app.push_system(format!(
                                "Sessions in {}:\n{}\n\nUse `/session new <title>`, `/session switch <id|title>`, `/session rename ...`, or `/session delete`.",
                                app.workspace_name, list
                            ));
                        }
                    }
                    "new" | "create" => {
                        let raw_title = parts.get(2..).map(|v| v.join(" ")).unwrap_or_else(|| {
                            format!("Session {}", chrono::Local::now().format("%H:%M"))
                        });
                        let normalized_title = raw_title.trim().to_string();
                        let title_arg = if normalized_title.is_empty() {
                            format!("Session {}", chrono::Local::now().format("%H:%M"))
                        } else {
                            normalized_title
                        };
                        let conversation_id =
                            manager.create_session(session.workspace_id, Some(&title_arg))?;
                        session.conversation_id = conversation_id;
                        app.active_session_id = conversation_id;
                        load_conversation_into_app(app, manager, session.conversation_id)?;
                        app.set_toast(ToastLevel::Success, "New session created");
                    }
                    "switch" => {
                        if parts.len() < 3 {
                            app.push_system("Usage: /session switch <id|title>".to_string());
                            return Ok(());
                        }
                        let selector = parts[2..].join(" ");
                        let sessions = manager.list_conversations(session.workspace_id)?;
                        let target = if let Ok(id) = selector.trim().parse::<i64>() {
                            sessions.into_iter().find(|c| c.id == id)
                        } else {
                            sessions.into_iter().find(|c| {
                                c.title
                                    .as_deref()
                                    .map(|t| t.eq_ignore_ascii_case(selector.trim()))
                                    .unwrap_or(false)
                            })
                        };
                        if let Some(target_session) = target {
                            session.conversation_id = target_session.id;
                            app.active_session_id = target_session.id;
                            load_conversation_into_app(app, manager, session.conversation_id)?;
                            app.set_toast(ToastLevel::Success, "Session switched");
                        } else {
                            app.push_system(format!(
                                "Session not found: {}. Use `/session list`.",
                                selector.trim()
                            ));
                        }
                    }
                    "delete" => {
                        let sessions = manager.list_conversations(session.workspace_id)?;
                        if sessions.is_empty() {
                            app.push_system("No sessions found in this workspace.".to_string());
                            return Ok(());
                        }

                        let selector = if parts.len() >= 3 {
                            parts[2..].join(" ")
                        } else {
                            let from_selection = match app.sidebar_item() {
                                Some(SidebarItem::WorkspaceSession(
                                    workspace_index,
                                    session_index,
                                )) => app.workspaces.get(workspace_index).and_then(|workspace| {
                                    if workspace.id == session.workspace_id {
                                        workspace.sessions.get(session_index).map(|entry| entry.id)
                                    } else {
                                        None
                                    }
                                }),
                                _ => None,
                            };
                            from_selection
                                .or_else(|| {
                                    (app.active_session_id > 0).then_some(app.active_session_id)
                                })
                                .map(|id| id.to_string())
                                .unwrap_or_default()
                        };

                        if selector.trim().is_empty() {
                            app.push_system("Usage: /session delete [id|title]".to_string());
                            return Ok(());
                        }

                        let target = if let Ok(id) = selector.trim().parse::<i64>() {
                            sessions.into_iter().find(|entry| entry.id == id)
                        } else {
                            sessions.into_iter().find(|entry| {
                                entry
                                    .title
                                    .as_deref()
                                    .map(|title| title.eq_ignore_ascii_case(selector.trim()))
                                    .unwrap_or(false)
                            })
                        };
                        let Some(target_session) = target else {
                            app.push_system(format!(
                                "Session not found: {}. Use `/session list`.",
                                selector.trim()
                            ));
                            return Ok(());
                        };

                        manager.delete_session(target_session.id)?;
                        if target_session.id == session.conversation_id {
                            let remaining = manager.list_conversations(session.workspace_id)?;
                            if let Some(next_session) = remaining.last() {
                                session.conversation_id = next_session.id;
                                app.active_session_id = next_session.id;
                                app.active_session_title = next_session
                                    .title
                                    .clone()
                                    .unwrap_or_else(|| format!("Session {}", next_session.id));
                                load_conversation_into_app(app, manager, session.conversation_id)?;
                            } else {
                                session.conversation_id = 0;
                                app.active_session_id = 0;
                                app.active_session_title = String::from("No Session");
                                app.messages.clear();
                                app.status = String::from("No session active");
                            }
                        }
                        app.set_toast(ToastLevel::Success, "Session deleted");
                    }
                    "rename" => {
                        let raw_args = prompt
                            .strip_prefix("/session rename")
                            .map(str::trim)
                            .unwrap_or_default();
                        if raw_args.is_empty() {
                            app.push_system(
                                "Usage: /session rename <id> <new title>\n   or: /session rename <title> --to <new title>"
                                    .to_string(),
                            );
                            return Ok(());
                        }

                        let sessions = manager.list_conversations(session.workspace_id)?;
                        if sessions.is_empty() {
                            app.push_system("No sessions found in this workspace.".to_string());
                            return Ok(());
                        }

                        let (selector, new_title) = if let Some((left, right)) =
                            raw_args.split_once(" --to ")
                        {
                            (left.trim().to_string(), right.trim().to_string())
                        } else if let Some(index) = raw_args.find(char::is_whitespace) {
                            let first = raw_args[..index].trim();
                            let rest = raw_args[index..].trim();
                            if first.parse::<i64>().is_ok() && !rest.is_empty() {
                                (first.to_string(), rest.to_string())
                            } else {
                                let mut titled_sessions = sessions
                                    .iter()
                                    .filter_map(|entry| {
                                        entry.title.as_ref().map(|title| (title.as_str(), entry.id))
                                    })
                                    .collect::<Vec<_>>();
                                titled_sessions
                                    .sort_by_key(|(title, _)| std::cmp::Reverse(title.len()));
                                let resolved = titled_sessions.into_iter().find_map(
                                    |(title, _conversation_id)| {
                                        if starts_with_ignore_ascii_case(raw_args, title) {
                                            let suffix =
                                                raw_args.get(title.len()..).unwrap_or_default();
                                            let candidate = suffix.trim();
                                            if !candidate.is_empty() {
                                                return Some((
                                                    title.to_string(),
                                                    candidate.to_string(),
                                                ));
                                            }
                                        }
                                        None
                                    },
                                );
                                if let Some(pair) = resolved {
                                    pair
                                } else {
                                    app.push_system(
                                        "Usage: /session rename <id> <new title>\nFor multi-word source titles, use: /session rename <title> --to <new title>"
                                            .to_string(),
                                    );
                                    return Ok(());
                                }
                            }
                        } else {
                            app.push_system(
                                "Usage: /session rename <id> <new title>\n   or: /session rename <title> --to <new title>"
                                    .to_string(),
                            );
                            return Ok(());
                        };

                        if selector.trim().is_empty() || new_title.trim().is_empty() {
                            app.push_system(
                                "Usage: /session rename <id> <new title>\n   or: /session rename <title> --to <new title>"
                                    .to_string(),
                            );
                            return Ok(());
                        }

                        let target = if let Ok(id) = selector.trim().parse::<i64>() {
                            sessions.iter().find(|entry| entry.id == id)
                        } else {
                            sessions.iter().find(|entry| {
                                entry
                                    .title
                                    .as_deref()
                                    .map(|title| title.eq_ignore_ascii_case(selector.trim()))
                                    .unwrap_or(false)
                            })
                        };

                        let Some(target_session) = target else {
                            app.push_system(format!(
                                "Session not found: {}. Use `/session list`.",
                                selector.trim()
                            ));
                            return Ok(());
                        };

                        manager.rename_session(target_session.id, new_title.trim())?;
                        if target_session.id == session.conversation_id {
                            app.active_session_title = new_title.trim().to_string();
                        }
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Session renamed to \"{}\"", new_title.trim()),
                        );
                    }
                    _ => app.push_system(session_help_text()),
                }
            }
        }
        "/checkpoint" => {
            if !app.has_active_session() || session.conversation_id <= 0 {
                app.push_system(
                    "No active session for checkpoints. Create or switch to a workspace first."
                        .to_string(),
                );
                return Ok(());
            }
            if parts.len() < 3 {
                app.push_system("Usage: /checkpoint <save|restore> <name>".to_string());
            } else {
                let sub = parts[1];
                let name = parts[2..].join(" ");
                match sub {
                    "save" => {
                        manager.save_checkpoint(session.conversation_id, &name)?;
                        app.set_toast(ToastLevel::Success, format!("Checkpoint saved: {name}"));
                    }
                    "restore" => {
                        let restored = manager.restore_checkpoint(
                            session.workspace_id,
                            session.conversation_id,
                            &name,
                        )?;
                        session.conversation_id = restored;
                        app.active_session_id = restored;
                        load_conversation_into_app(app, manager, session.conversation_id)?;
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Restored checkpoint into new conversation: {name}"),
                        );
                    }
                    _ => app.push_system("Unknown checkpoint subcommand.".to_string()),
                }
            }
        }
        "/attach" => {
            let path = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if path.trim().is_empty() {
                app.push_system("Usage: /attach <image_path>".to_string());
            } else {
                let pathbuf = PathBuf::from(path.clone());
                let ext_ok = pathbuf
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| {
                        matches!(
                            e.to_lowercase().as_str(),
                            "png" | "jpg" | "jpeg" | "webp" | "gif"
                        )
                    })
                    .unwrap_or(false);
                if !ext_ok {
                    app.push_system(
                        "Only png/jpg/jpeg/webp/gif attachments are supported.".to_string(),
                    );
                    return Ok(());
                }
                let bytes = fs::read(&pathbuf)?;
                let encoded = STANDARD.encode(bytes);
                app.pending_images.push(PendingImage {
                    path,
                    base64: encoded,
                });
                app.set_toast(
                    ToastLevel::Info,
                    format!("Attached image ({} pending)", app.pending_images.len()),
                );
            }
        }
        "/clear-attachments" => {
            app.pending_images.clear();
            app.set_toast(ToastLevel::Info, "Pending attachments cleared");
        }
        "/model" => {
            let model = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if model.trim().is_empty() {
                if app.models.is_empty() {
                    app.push_system(
                        "Usage: /model <name>\nNo models cached yet. Try `/models refresh`."
                            .to_string(),
                    );
                } else {
                    let lines = app
                        .models
                        .iter()
                        .take(20)
                        .map(|entry| {
                            if entry == &app.model {
                                format!("* {entry} (active)")
                            } else {
                                format!("- {entry}")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    app.push_system(format!(
                        "Usage: /model <name>\nCurrent: {}\n\nKnown models:\n{}\n\nTip: `/models refresh` to reload.",
                        app.model, lines
                    ));
                }
            } else {
                let requested = model.trim();
                if let Some(exact) = app.models.iter().find(|entry| *entry == requested).cloned() {
                    app.model = exact;
                    app.set_toast(ToastLevel::Success, "Model switched");
                } else if let Some(prefix_match) = app
                    .models
                    .iter()
                    .find(|entry| entry.starts_with(requested))
                    .cloned()
                {
                    app.model = prefix_match;
                    app.set_toast(
                        ToastLevel::Success,
                        format!("Model switched (prefix match): {}", app.model),
                    );
                } else {
                    app.model = requested.to_string();
                    app.set_toast(
                        ToastLevel::Info,
                        "Model set (not in cache). Try `/models refresh` if needed.",
                    );
                }
            }
        }
        "/url" => {
            let url = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if url.trim().is_empty() {
                app.push_system("Usage: /url <http://host:port>".to_string());
            } else {
                app.url = url;
                app.set_toast(ToastLevel::Success, "Server URL updated for this session");
            }
        }
        _ => app.push_system("Unknown command. Use /commands".to_string()),
    }

    Ok(())
}

fn format_tool_calls(tool_calls: &[ChatToolCall]) -> String {
    if tool_calls.is_empty() {
        return String::from("Tool calls: (none)");
    }
    tool_calls
        .iter()
        .enumerate()
        .map(|(index, call)| {
            if let Some(function) = &call.function {
                format!("{}. {}({})", index + 1, function.name, function.arguments)
            } else {
                format!("{}. <unknown tool call>", index + 1)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_calls_preview(tool_calls: &[ChatToolCall]) -> Option<String> {
    if tool_calls.is_empty() {
        return None;
    }
    let names = tool_calls
        .iter()
        .filter_map(|call| call.function.as_ref().map(|f| f.name.clone()))
        .collect::<Vec<_>>();
    if names.is_empty() {
        Some(format!("{} tool call(s)", tool_calls.len()))
    } else {
        Some(format!("Tools: {}", names.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        autocomplete_suggestions, is_cloud_model_name, load_ui_state_from_path,
        next_autocomplete_value, save_ui_state_to_path, App, ModelBucket, SessionEntry,
        SidebarItem, UiRuntimeState, WorkspaceEntry,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn sample_workspaces() -> Vec<WorkspaceEntry> {
        vec![
            WorkspaceEntry {
                id: 1,
                name: String::from("Default Workspace"),
                root_path: Some(String::from("C:/CODING/olly")),
                sessions: Vec::new(),
            },
            WorkspaceEntry {
                id: 2,
                name: String::from("Client Project"),
                root_path: Some(String::from("C:/CODING/client")),
                sessions: Vec::new(),
            },
        ]
    }

    fn sample_sessions() -> Vec<SessionEntry> {
        vec![
            SessionEntry {
                id: 10,
                title: String::from("Main"),
            },
            SessionEntry {
                id: 11,
                title: String::from("Refactor"),
            },
        ]
    }

    #[test]
    fn cloud_model_classifier_handles_common_patterns() {
        assert!(is_cloud_model_name("minimax-m2.7:cloud"));
        assert!(is_cloud_model_name("devstral-2:123b-cloud"));
        assert!(!is_cloud_model_name("qwen2.5-coder:7b"));
    }

    #[test]
    fn sidebar_groups_models_into_cloud_and_local_categories() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("qwen2.5-coder:7b"),
        );
        app.models = vec![
            String::from("minimax-m2.7:cloud"),
            String::from("qwen2.5-coder:7b"),
            String::from("devstral-2:123b-cloud"),
        ];

        assert_eq!(app.model_count_for_bucket(ModelBucket::Cloud), 2);
        assert_eq!(app.model_count_for_bucket(ModelBucket::Local), 1);

        let entries = app.sidebar_entries();
        assert!(entries.contains(&SidebarItem::ModelGroupHeader(ModelBucket::Cloud)));
        assert!(entries.contains(&SidebarItem::ModelGroupHeader(ModelBucket::Local)));
    }

    #[test]
    fn ui_runtime_state_roundtrips_for_sidebar_preferences() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.models_collapsed = true;
        app.cloud_models_collapsed = true;
        app.local_models_collapsed = false;
        app.workspaces_collapsed = true;
        app.collapsed_workspace_ids = [4_i64, 2_i64].into_iter().collect();
        app.sidebar_index = 7;

        let state = app.snapshot_ui_state();
        assert_eq!(
            state,
            UiRuntimeState {
                models_collapsed: true,
                cloud_models_collapsed: true,
                local_models_collapsed: false,
                workspaces_collapsed: true,
                collapsed_workspace_ids: vec![2, 4],
                sidebar_index: 7,
            }
        );

        let mut next = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        next.apply_ui_state(state);
        assert!(next.models_collapsed);
        assert!(next.cloud_models_collapsed);
        assert!(!next.local_models_collapsed);
        assert!(next.workspaces_collapsed);
        assert!(next.collapsed_workspace_ids.contains(&2));
        assert!(next.collapsed_workspace_ids.contains(&4));
        assert_eq!(next.sidebar_index, 7);
    }

    #[test]
    fn ui_state_file_save_and_load_roundtrip() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("olly-ui-state-{unique}"));
        let path = root.join("ui_state.json");
        let state = UiRuntimeState {
            models_collapsed: true,
            cloud_models_collapsed: false,
            local_models_collapsed: true,
            workspaces_collapsed: false,
            collapsed_workspace_ids: vec![10, 3],
            sidebar_index: 5,
        };

        save_ui_state_to_path(&path, &state).expect("save ui state");
        let loaded = load_ui_state_from_path(&path).expect("load ui state");
        assert_eq!(loaded, state);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn autocomplete_promotes_mod_to_model_command() {
        let models = vec![String::from("llama3.2:latest")];
        assert_eq!(
            autocomplete_suggestions("/mod", &models, &sample_workspaces(), &sample_sessions()),
            vec![String::from("/model ")]
        );
    }

    #[test]
    fn autocomplete_fills_model_name_by_prefix() {
        let models = vec![
            String::from("devstral-2:123b-cloud"),
            String::from("llama3.2:latest"),
        ];
        assert_eq!(
            autocomplete_suggestions(
                "/model dev",
                &models,
                &sample_workspaces(),
                &sample_sessions(),
            ),
            vec![String::from("/model devstral-2:123b-cloud")]
        );
    }

    #[test]
    fn autocomplete_expands_models_subcommands() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/models ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/models refresh")));
        assert!(suggestions.contains(&String::from("/models add ")));
        assert!(suggestions.contains(&String::from("/models remove ")));
    }

    #[test]
    fn autocomplete_returns_empty_for_non_command_input() {
        let models = vec![String::from("llama3.2:latest")];
        assert!(autocomplete_suggestions(
            "hello",
            &models,
            &sample_workspaces(),
            &sample_sessions()
        )
        .is_empty());
    }

    #[test]
    fn autocomplete_cycles_to_next_option() {
        let suggestions = vec![String::from("/model one"), String::from("/model two")];
        assert_eq!(
            next_autocomplete_value("/model one", &suggestions),
            Some(String::from("/model two"))
        );
    }

    #[test]
    fn autocomplete_expands_workspace_subcommands() {
        let models = vec![String::from("llama3.2:latest")];
        assert!(autocomplete_suggestions(
            "/workspace ",
            &models,
            &sample_workspaces(),
            &sample_sessions()
        )
        .contains(&String::from("/workspace create ")));
        assert!(autocomplete_suggestions(
            "/workspace ",
            &models,
            &sample_workspaces(),
            &sample_sessions()
        )
        .contains(&String::from("/workspace delete ")));
        assert!(autocomplete_suggestions(
            "/workspace ",
            &models,
            &sample_workspaces(),
            &sample_sessions()
        )
        .contains(&String::from("/workspace rename ")));
    }

    #[test]
    fn autocomplete_workspace_switch_suggests_names_and_ids() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/workspace switch ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/workspace switch 1")));
        assert!(suggestions.contains(&String::from("/workspace switch Client Project")));
    }

    #[test]
    fn autocomplete_workspace_delete_includes_confirm_cancel() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/workspace delete ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/workspace delete")));
        assert!(suggestions.contains(&String::from("/workspace delete --confirm")));
        assert!(suggestions.contains(&String::from("/workspace delete --cancel")));
    }

    #[test]
    fn autocomplete_workspace_create_offers_path_hint() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/workspace create Client",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/workspace create Client --path ")));
    }

    #[test]
    fn autocomplete_workspace_rename_includes_template() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/workspace rename ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/workspace rename ")));
    }

    #[test]
    fn autocomplete_workspace_set_path_suggests_targets() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/workspace set-path ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/workspace set-path 1 --path ")));
        assert!(suggestions.contains(&String::from("/workspace set-path Client Project --path ")));
    }

    #[test]
    fn autocomplete_session_switch_suggests_ids_and_titles() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/session switch ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/session switch 10")));
        assert!(suggestions.contains(&String::from("/session switch Refactor")));
    }

    #[test]
    fn autocomplete_session_subcommands_include_rename() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/session ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/session rename ")));
        assert!(suggestions.contains(&String::from("/session create ")));
        assert!(suggestions.contains(&String::from("/session delete")));
    }

    #[test]
    fn autocomplete_session_rename_suggests_ids_and_titles() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/session rename ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/session rename 10 ")));
        assert!(suggestions.contains(&String::from("/session rename Refactor --to ")));
    }

    #[test]
    fn autocomplete_session_delete_suggests_ids_and_titles() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/session delete ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert!(suggestions.contains(&String::from("/session delete 10")));
        assert!(suggestions.contains(&String::from("/session delete Refactor")));
    }
}
