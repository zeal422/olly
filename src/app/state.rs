use std::{
    cell::Cell,
    collections::HashSet,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ratatui::layout::Alignment;
use serde::{Deserialize, Serialize};

use crate::{
    app::tools::ToolsState,
    config::AppConfig,
    ollama::chat::{ChatMessage, ChatToolCall},
    workspace::manager::WorkspaceManager,
};

use super::{
    autocomplete_suggestions,
    domain::{ModelBucket, SidebarItem, SidebarSection},
    load_conversation_into_app, tool_calls_preview,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum UiTheme {
    #[default]
    EmeraldDusk,
    NeonCircuit,
    SunsetEmber,
    OceanMidnight,
    MonoAmber,
    SakuraNight,
    PaperMint,
    DaybreakCanvas,
    Dracula,
    Nord,
    GruvboxDark,
    TokyoNight,
    CatppuccinMocha,
    SolarizedDark,
    MonokaiPro,
    OneDark,
    ArcticFrost,
    WarmSand,
    LavenderDream,
    RoseGarden,
}

impl UiTheme {
    pub const ALL: [UiTheme; 20] = [
        UiTheme::EmeraldDusk,
        UiTheme::NeonCircuit,
        UiTheme::SunsetEmber,
        UiTheme::OceanMidnight,
        UiTheme::MonoAmber,
        UiTheme::SakuraNight,
        UiTheme::PaperMint,
        UiTheme::DaybreakCanvas,
        UiTheme::Dracula,
        UiTheme::Nord,
        UiTheme::GruvboxDark,
        UiTheme::TokyoNight,
        UiTheme::CatppuccinMocha,
        UiTheme::SolarizedDark,
        UiTheme::MonokaiPro,
        UiTheme::OneDark,
        UiTheme::ArcticFrost,
        UiTheme::WarmSand,
        UiTheme::LavenderDream,
        UiTheme::RoseGarden,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UiTheme::EmeraldDusk => "Emerald Dusk",
            UiTheme::NeonCircuit => "Neon Circuit",
            UiTheme::SunsetEmber => "Sunset Ember",
            UiTheme::OceanMidnight => "Ocean Midnight",
            UiTheme::MonoAmber => "Mono Amber",
            UiTheme::SakuraNight => "Sakura Night",
            UiTheme::PaperMint => "Paper Mint",
            UiTheme::DaybreakCanvas => "Daybreak Canvas",
            UiTheme::Dracula => "Dracula",
            UiTheme::Nord => "Nord",
            UiTheme::GruvboxDark => "Gruvbox Dark",
            UiTheme::TokyoNight => "Tokyo Night",
            UiTheme::CatppuccinMocha => "Catppuccin Mocha",
            UiTheme::SolarizedDark => "Solarized Dark",
            UiTheme::MonokaiPro => "Monokai Pro",
            UiTheme::OneDark => "One Dark",
            UiTheme::ArcticFrost => "Arctic Frost",
            UiTheme::WarmSand => "Warm Sand",
            UiTheme::LavenderDream => "Lavender Dream",
            UiTheme::RoseGarden => "Rose Garden",
        }
    }

    pub fn is_light(self) -> bool {
        matches!(
            self,
            UiTheme::PaperMint
                | UiTheme::DaybreakCanvas
                | UiTheme::ArcticFrost
                | UiTheme::WarmSand
                | UiTheme::LavenderDream
                | UiTheme::RoseGarden
        )
    }
}

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
pub struct PendingSessionDelete {
    pub workspace_id: i64,
    pub id: i64,
    pub name: String,
    pub expires_at: Instant,
}

/// Pending tool confirmation for policy=confirm mode
#[derive(Debug, Clone)]
pub struct PendingToolConfirmation {
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_args: String,
    pub expires_at: Instant,
    pub step: usize,
    pub total_steps: usize,
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
    pub tool_name: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct SessionContextSnapshot {
    pub workspace_id: i64,
    pub session_id: i64,
    pub model: String,
    pub model_context_window: Option<u32>,
    pub prompt_tokens_used: Option<u32>,
    pub last_completion_tokens: Option<u32>,
    pub peak_prompt_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct UiRuntimeState {
    pub model: Option<String>,
    pub theme: UiTheme,
    pub models_collapsed: bool,
    pub cloud_models_collapsed: bool,
    pub local_models_collapsed: bool,
    pub workspaces_collapsed: bool,
    pub collapsed_workspace_ids: Vec<i64>,
    pub sidebar_index: usize,
    pub session_context: Option<SessionContextSnapshot>,
    pub tools_state: ToolsState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePickerState {
    pub selected: usize,
    pub original: UiTheme,
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
    pub model_context_window: Option<u32>,
    pub prompt_tokens_used: Option<u32>,
    pub last_completion_tokens: Option<u32>,
    pub peak_prompt_tokens: u32,
    pub status: String,
    pub connection_status: ConnectionStatus,
    pub focus: FocusPane,
    pub show_help: bool,
    pub toast: Option<Toast>,
    pub chat_scroll: u16,
    pub chat_max_scroll_hint: Cell<u16>,
    pub chat_selection_anchor: Option<(usize, usize)>,
    pub chat_selection_current: Option<(usize, usize)>,
    pub is_selecting: bool,
    pub chat_area: Option<ratatui::layout::Rect>,
    pub chat_visual_lines: Vec<(String, Option<Alignment>)>,
    pub chat_visual_lines_styled: Vec<ratatui::text::Line<'static>>,
    pub chat_cached_version: u64,
    pub chat_content_version: u64,
    pub chat_last_render_width: u16,
    pub chat_last_render_height: u16,
    pub auto_scroll_direction: Option<i16>,
    pub auto_scroll_speed: u16,
    pub is_loading: bool,
    pub spinner_frame: usize,
    pub syntax_highlighting: bool,
    pub pending_images: Vec<PendingImage>,
    pub pending_workspace_delete: Option<PendingWorkspaceDelete>,
    pub pending_session_delete: Option<PendingSessionDelete>,
    pub theme: UiTheme,
    pub theme_picker: Option<ThemePickerState>,
    pub tools_state: ToolsState,
    pub pending_tool_confirmation: Option<PendingToolConfirmation>,
    pub command_suggestion_index: usize,
    pub input_cursor_char_index: usize,
    pub input_cursor_preferred_column: Option<usize>,
    pub quit_armed_until: Option<Instant>,
    restored_session_context: Option<SessionContextSnapshot>,
    should_quit: bool,
}

#[allow(dead_code)]
pub(crate) struct AppUiStateView<'a> {
    pub(crate) focus: FocusPane,
    pub(crate) show_help: bool,
    pub(crate) toast: Option<&'a Toast>,
    pub(crate) theme: UiTheme,
    pub(crate) sidebar_index: usize,
}

#[allow(dead_code)]
pub(crate) struct AppSessionStateView<'a> {
    pub(crate) active_workspace_id: i64,
    pub(crate) active_session_id: i64,
    pub(crate) workspace_name: &'a str,
    pub(crate) active_session_title: &'a str,
    pub(crate) workspace_root_path: Option<&'a str>,
}

#[allow(dead_code)]
pub(crate) struct AppTelemetryStateView {
    pub(crate) model_context_window: Option<u32>,
    pub(crate) prompt_tokens_used: Option<u32>,
    pub(crate) last_completion_tokens: Option<u32>,
    pub(crate) peak_prompt_tokens: u32,
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
            model_context_window: None,
            prompt_tokens_used: None,
            last_completion_tokens: None,
            peak_prompt_tokens: 0,
            status: String::from("Ready"),
            connection_status: ConnectionStatus::Unknown,
            focus: FocusPane::Chat,
            show_help: false,
            toast: None,
            chat_scroll: 0,
            chat_max_scroll_hint: Cell::new(0),
            chat_selection_anchor: None,
            chat_selection_current: None,
            is_selecting: false,
            chat_area: None,
            chat_visual_lines: Vec::new(),
            chat_visual_lines_styled: Vec::new(),
            chat_cached_version: 0,
            chat_content_version: 0,
            chat_last_render_width: 0,
            chat_last_render_height: 0,
            auto_scroll_direction: None,
            auto_scroll_speed: 3,
            is_loading: false,
            spinner_frame: 0,
            syntax_highlighting: true,
            pending_images: Vec::new(),
            pending_workspace_delete: None,
            pending_session_delete: None,
            theme: UiTheme::default(),
            theme_picker: None,
            tools_state: ToolsState::default(),
            pending_tool_confirmation: None,
            command_suggestion_index: 0,
            input_cursor_char_index: 0,
            input_cursor_preferred_column: None,
            quit_armed_until: None,
            restored_session_context: None,
            should_quit: false,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    #[allow(dead_code)]
    pub(crate) fn ui_state_view(&self) -> AppUiStateView<'_> {
        AppUiStateView {
            focus: self.focus,
            show_help: self.show_help,
            toast: self.toast.as_ref(),
            theme: self.theme,
            sidebar_index: self.sidebar_index,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn session_state_view(&self) -> AppSessionStateView<'_> {
        AppSessionStateView {
            active_workspace_id: self.active_workspace_id,
            active_session_id: self.active_session_id,
            workspace_name: &self.workspace_name,
            active_session_title: &self.active_session_title,
            workspace_root_path: self.workspace_root_path.as_deref(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn telemetry_state_view(&self) -> AppTelemetryStateView {
        AppTelemetryStateView {
            model_context_window: self.model_context_window,
            prompt_tokens_used: self.prompt_tokens_used,
            last_completion_tokens: self.last_completion_tokens,
            peak_prompt_tokens: self.peak_prompt_tokens,
        }
    }

    pub(crate) fn snapshot_ui_state(&self) -> UiRuntimeState {
        let mut collapsed_workspace_ids = self
            .collapsed_workspace_ids
            .iter()
            .copied()
            .collect::<Vec<_>>();
        collapsed_workspace_ids.sort_unstable();
        UiRuntimeState {
            model: (!self.model.is_empty()).then_some(self.model.clone()),
            theme: self.theme,
            models_collapsed: self.models_collapsed,
            cloud_models_collapsed: self.cloud_models_collapsed,
            local_models_collapsed: self.local_models_collapsed,
            workspaces_collapsed: self.workspaces_collapsed,
            collapsed_workspace_ids,
            sidebar_index: self.sidebar_index,
            session_context: self.capture_active_session_context_snapshot(),
            tools_state: self.tools_state.clone(),
        }
    }

    pub(crate) fn apply_ui_state(&mut self, state: UiRuntimeState) {
        let UiRuntimeState {
            model,
            theme,
            models_collapsed,
            cloud_models_collapsed,
            local_models_collapsed,
            workspaces_collapsed,
            collapsed_workspace_ids,
            sidebar_index,
            session_context,
            tools_state,
        } = state;
        if let Some(saved_model) = model {
            if !saved_model.trim().is_empty() {
                self.model = saved_model;
            }
        }
        self.theme = theme;
        self.models_collapsed = models_collapsed;
        self.cloud_models_collapsed = cloud_models_collapsed;
        self.local_models_collapsed = local_models_collapsed;
        self.workspaces_collapsed = workspaces_collapsed;
        self.collapsed_workspace_ids = collapsed_workspace_ids.into_iter().collect();
        self.sidebar_index = sidebar_index;
        self.restored_session_context = session_context;
        self.tools_state = tools_state;
    }

    fn capture_active_session_context_snapshot(&self) -> Option<SessionContextSnapshot> {
        if self.active_workspace_id <= 0 || self.active_session_id <= 0 || self.model.is_empty() {
            return None;
        }
        Some(SessionContextSnapshot {
            workspace_id: self.active_workspace_id,
            session_id: self.active_session_id,
            model: self.model.clone(),
            model_context_window: self.model_context_window,
            prompt_tokens_used: self.prompt_tokens_used,
            last_completion_tokens: self.last_completion_tokens,
            peak_prompt_tokens: self.peak_prompt_tokens,
        })
    }

    pub(crate) fn persist_active_session_context_snapshot(&mut self) {
        self.restored_session_context = self.capture_active_session_context_snapshot();
    }

    pub(crate) fn restore_session_context_snapshot_if_match(&mut self) {
        let Some(snapshot) = self.restored_session_context.as_ref() else {
            return;
        };
        if snapshot.workspace_id != self.active_workspace_id
            || snapshot.session_id != self.active_session_id
            || snapshot.model != self.model
        {
            return;
        }
        self.model_context_window = snapshot.model_context_window;
        self.prompt_tokens_used = snapshot.prompt_tokens_used;
        self.last_completion_tokens = snapshot.last_completion_tokens;
        self.peak_prompt_tokens = snapshot.peak_prompt_tokens;
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    pub(crate) fn request_quit(&mut self) {
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

    pub(crate) fn switch_focus(&mut self) {
        self.clear_chat_selection();
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

    pub(crate) fn take_input_for_send(&mut self) -> Option<String> {
        let trimmed = self.input.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        self.input.clear();
        self.input_cursor_char_index = 0;
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
        Some(trimmed)
    }

    pub(crate) fn reset_command_selection(&mut self) {
        self.command_suggestion_index = 0;
    }

    pub(crate) fn to_ollama_messages(&self) -> Vec<ChatMessage> {
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
                if role == "tool" {
                    Some(ChatMessage::new_tool(
                        m.tool_name.clone().unwrap_or_else(|| "unknown".to_string()),
                        m.content.clone(),
                    ))
                } else {
                    let mut msg = ChatMessage::new(role, m.content.clone());
                    if role == "assistant" {
                        msg.tool_calls = m.tool_calls.clone();
                    }
                    Some(msg)
                }
            })
            .collect()
    }

    pub(crate) fn from_storage_role(
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
        let parsed_tool_calls = tool_calls
            .as_ref()
            .and_then(|raw| serde_json::from_str::<Vec<ChatToolCall>>(raw).ok());
        let tool_preview = parsed_tool_calls
            .as_ref()
            .and_then(|calls| tool_calls_preview(calls));
        let tool_name = if role == "tool" {
            parsed_tool_calls
                .as_ref()
                .and_then(|calls| calls.first())
                .and_then(|call| call.function.as_ref())
                .map(|function| function.name.clone())
        } else {
            None
        };
        let tool_calls = parsed_tool_calls.clone();
        Some(Message {
            role: display_role.to_string(),
            content,
            timestamp: Self::normalize_storage_timestamp(&timestamp),
            image_count,
            tool_calls_preview: tool_preview,
            tool_name,
            tool_calls,
        })
    }

    pub(crate) fn refresh_workspaces(
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

    pub(crate) fn sync_active_workspace_label(&mut self, workspace_id: i64) {
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

    pub(crate) fn has_active_session(&self) -> bool {
        self.active_workspace_id > 0 && self.active_session_id > 0
    }

    pub(crate) fn clear_active_workspace_state(&mut self) {
        self.active_workspace_id = 0;
        self.workspace_name = String::from("No Workspace");
        self.workspace_root_path = None;
        self.active_session_id = 0;
        self.active_session_title = String::from("No Session");
        self.pending_session_delete = None;
        self.prompt_tokens_used = None;
        self.last_completion_tokens = None;
        self.peak_prompt_tokens = 0;
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

    pub(crate) fn refresh_sessions(
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

    pub(crate) fn sync_active_session_label(&mut self) {
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

    pub(crate) fn clamp_sidebar_index(&mut self) {
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

    pub(crate) fn sidebar_up(&mut self, amount: u16) {
        if self.sidebar_item_count() == 0 {
            return;
        }
        self.sidebar_index = self.sidebar_index.saturating_sub(amount as usize);
    }

    pub(crate) fn sidebar_down(&mut self, amount: u16) {
        if self.sidebar_item_count() == 0 {
            return;
        }
        let max = self.sidebar_item_count() - 1;
        self.sidebar_index = (self.sidebar_index + amount as usize).min(max);
    }

    pub(crate) fn focus_active_session(&mut self, workspace_id: i64, conversation_id: i64) {
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

    pub(crate) fn activate_sidebar_selection(
        &mut self,
        manager: &WorkspaceManager,
        session: &mut crate::workspace::WorkspaceSession,
        default_model: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.clear_chat_selection();
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
                self.set_input_text(String::from("/workspace create New Workspace --path "));
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

    pub(crate) fn autocomplete_command(&mut self) {
        let suggestions =
            autocomplete_suggestions(&self.input, &self.models, &self.workspaces, &self.sessions);
        if suggestions.is_empty() {
            return;
        }
        let index = self
            .command_suggestion_index
            .min(suggestions.len().saturating_sub(1));
        if let Some(suggestion) = suggestions.get(index) {
            self.set_input_text(suggestion.clone());
            self.command_suggestion_index = (index + 1) % suggestions.len();
        }
    }

    pub(crate) fn autocomplete_command_prev(&mut self) {
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

    pub(crate) fn autocomplete_command_next(&mut self) {
        let suggestions =
            autocomplete_suggestions(&self.input, &self.models, &self.workspaces, &self.sessions);
        if suggestions.is_empty() {
            return;
        }
        self.command_suggestion_index = (self.command_suggestion_index + 1) % suggestions.len();
    }

    pub(crate) fn is_command_suggestion_mode(&self) -> bool {
        self.is_slash_command_input() && !self.autocomplete_options().is_empty()
    }

    pub(crate) fn is_slash_command_input(&self) -> bool {
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

pub fn is_cloud_model_name(model: &str) -> bool {
    let value = model.trim().to_ascii_lowercase();
    value.contains(":cloud") || value.ends_with("-cloud") || value.contains("-cloud:")
}

pub fn ui_state_path_for_config(config: &AppConfig) -> PathBuf {
    Path::new(&config.storage.db_path)
        .parent()
        .map(|dir| dir.join("ui_state.json"))
        .unwrap_or_else(|| crate::config::default_data_dir().join("ui_state.json"))
}

pub fn load_ui_state_from_path(path: &std::path::Path) -> Option<UiRuntimeState> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<UiRuntimeState>(&raw).ok()
}

pub fn save_ui_state_to_path(
    path: &std::path::Path,
    state: &UiRuntimeState,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(state)
        .map_err(|err| std::io::Error::other(format!("serialize ui state: {err}")))?;
    std::fs::write(path, raw)
}

pub fn apply_saved_ui_state_with_startup_precedence(
    app: &mut App,
    saved_ui_state: UiRuntimeState,
    startup_model: &str,
    model_was_overridden: bool,
) {
    app.apply_ui_state(saved_ui_state);
    if model_was_overridden {
        app.model = startup_model.to_string();
    }
}
