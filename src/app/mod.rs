pub mod autocomplete;
pub mod chat_pipeline;
pub mod commands;
pub mod context;
pub mod domain;
pub mod input;
pub mod platform;
pub mod runtime;
pub mod tools;
pub mod tui;

use std::{
    cell::Cell,
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::Local;
use ratatui::layout::Alignment;
use serde::{Deserialize, Serialize};

use crate::{
    app::tools::ToolsState,
    config::AppConfig,
    history,
    ollama::{
        self,
        chat::{ChatMessage, ChatToolCall},
    },
    workspace::manager::WorkspaceManager,
};

pub(crate) const COMMAND_SUGGESTIONS: &[&str] = &[
    "/commands",
    "/models",
    "/themes",
    "/session",
    "/search",
    "/export",
    "/workspace",
    "/checkpoint",
    "/attach",
    "/clear-attachments",
    "/model",
    "/url",
    "/tools",
];
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
}

impl UiTheme {
    pub const ALL: [UiTheme; 8] = [
        UiTheme::EmeraldDusk,
        UiTheme::NeonCircuit,
        UiTheme::SunsetEmber,
        UiTheme::OceanMidnight,
        UiTheme::MonoAmber,
        UiTheme::SakuraNight,
        UiTheme::PaperMint,
        UiTheme::DaybreakCanvas,
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
        }
    }

    pub fn is_light(self) -> bool {
        matches!(self, UiTheme::PaperMint | UiTheme::DaybreakCanvas)
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
struct SessionContextSnapshot {
    workspace_id: i64,
    session_id: i64,
    model: String,
    model_context_window: Option<u32>,
    prompt_tokens_used: Option<u32>,
    last_completion_tokens: Option<u32>,
    peak_prompt_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct UiRuntimeState {
    model: Option<String>,
    theme: UiTheme,
    models_collapsed: bool,
    cloud_models_collapsed: bool,
    local_models_collapsed: bool,
    workspaces_collapsed: bool,
    collapsed_workspace_ids: Vec<i64>,
    sidebar_index: usize,
    session_context: Option<SessionContextSnapshot>,
    tools_state: ToolsState,
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
    chat_max_scroll_hint: Cell<u16>,
    pub chat_selection_anchor: Option<(usize, usize)>,
    pub chat_selection_current: Option<(usize, usize)>,
    pub is_selecting: bool,
    pub chat_area: Option<ratatui::layout::Rect>,
    pub chat_visual_lines: Vec<(String, Option<Alignment>)>,
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

    fn apply_ui_state(&mut self, state: UiRuntimeState) {
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
        if let Some(pending_delete) = &self.pending_session_delete {
            if Instant::now() > pending_delete.expires_at {
                self.pending_session_delete = None;
            }
        }
        if let Some(pending_tool) = &self.pending_tool_confirmation {
            if Instant::now() > pending_tool.expires_at {
                self.pending_tool_confirmation = None;
            }
        }

        // Apply auto-scroll if active during selection drag
        if let Some(direction) = self.auto_scroll_direction {
            if direction < 0 {
                self.scroll_up_preserve_selection(self.auto_scroll_speed);
            } else {
                self.scroll_down_preserve_selection(self.auto_scroll_speed);
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

    pub(crate) fn set_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(Toast {
            message: message.into(),
            level,
            expires_at: Instant::now() + Duration::from_secs(4),
        });
    }

    pub(crate) fn dismiss_overlay(&mut self) {
        if self.show_help {
            self.show_help = false;
            return;
        }
        self.toast = None;
    }

    pub(crate) fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
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

    pub fn theme_picker_entries(&self) -> &'static [UiTheme] {
        &UiTheme::ALL
    }

    fn open_theme_picker(&mut self) {
        let selected = UiTheme::ALL
            .iter()
            .position(|theme| *theme == self.theme)
            .unwrap_or(0);
        self.theme_picker = Some(ThemePickerState {
            selected,
            original: self.theme,
        });
        self.set_toast(ToastLevel::Info, "Theme picker opened (live preview)");
    }

    pub(crate) fn theme_picker_next(&mut self) {
        if let Some(state) = &mut self.theme_picker {
            state.selected = (state.selected + 1) % UiTheme::ALL.len();
            self.theme = UiTheme::ALL[state.selected];
        }
    }

    pub(crate) fn theme_picker_prev(&mut self) {
        if let Some(state) = &mut self.theme_picker {
            if state.selected == 0 {
                state.selected = UiTheme::ALL.len() - 1;
            } else {
                state.selected -= 1;
            }
            self.theme = UiTheme::ALL[state.selected];
        }
    }

    pub(crate) fn theme_picker_confirm(&mut self) {
        if self.theme_picker.take().is_some() {
            self.set_toast(
                ToastLevel::Success,
                format!("Theme set: {}", self.theme.label()),
            );
        }
    }

    pub(crate) fn theme_picker_cancel(&mut self) {
        if let Some(state) = self.theme_picker.take() {
            self.theme = state.original;
            self.set_toast(ToastLevel::Info, "Theme picker canceled");
        }
    }

    fn scroll_up(&mut self, amount: u16) {
        let max_scroll = self.chat_max_scroll_hint.get();
        self.chat_scroll = self
            .chat_scroll
            .min(max_scroll)
            .saturating_add(amount)
            .min(max_scroll);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.chat_scroll = self
            .chat_scroll
            .min(self.chat_max_scroll_hint.get())
            .saturating_sub(amount);
    }

    fn scroll_up_preserve_selection(&mut self, amount: u16) {
        let max_scroll = self.chat_max_scroll_hint.get();
        self.chat_scroll = self
            .chat_scroll
            .min(max_scroll)
            .saturating_add(amount)
            .min(max_scroll);
    }

    fn scroll_down_preserve_selection(&mut self, amount: u16) {
        self.chat_scroll = self
            .chat_scroll
            .min(self.chat_max_scroll_hint.get())
            .saturating_sub(amount);
    }

    pub(crate) fn update_chat_max_scroll_hint(&self, max_scroll: u16) {
        self.chat_max_scroll_hint.set(max_scroll);
    }

    pub(crate) fn clamp_chat_scroll_to_hint(&mut self) {
        self.chat_scroll = self.chat_scroll.min(self.chat_max_scroll_hint.get());
    }

    pub(crate) fn clear_chat_selection(&mut self) {
        self.chat_selection_anchor = None;
        self.chat_selection_current = None;
        self.is_selecting = false;
    }

    pub(crate) fn update_chat_area_metadata(
        &mut self,
        area: ratatui::layout::Rect,
        visual_lines: Vec<(String, Option<Alignment>)>,
        max_scroll: u16,
    ) {
        self.chat_area = Some(area);
        self.chat_visual_lines = visual_lines;
        self.update_chat_max_scroll_hint(max_scroll);
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

    fn input_char_len(&self) -> usize {
        self.input.chars().count()
    }

    pub(crate) fn clear_input_editor_state(&mut self) {
        self.input_cursor_char_index = self.input_char_len();
        self.input_cursor_preferred_column = None;
    }

    fn set_input_text(&mut self, value: String) {
        self.input = value;
        self.clear_input_editor_state();
    }

    fn byte_index_for_char(input: &str, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        let total_chars = input.chars().count();
        if char_index >= total_chars {
            return input.len();
        }
        input
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(input.len())
    }

    fn clamp_input_cursor(&mut self) {
        self.input_cursor_char_index = self.input_cursor_char_index.min(self.input_char_len());
    }

    pub(crate) fn move_input_cursor_left(&mut self) {
        self.clamp_input_cursor();
        self.input_cursor_char_index = self.input_cursor_char_index.saturating_sub(1);
        self.input_cursor_preferred_column = None;
    }

    pub(crate) fn move_input_cursor_right(&mut self) {
        self.clamp_input_cursor();
        self.input_cursor_char_index =
            (self.input_cursor_char_index + 1).min(self.input_char_len());
        self.input_cursor_preferred_column = None;
    }

    fn input_line_count(&self) -> usize {
        self.input.split('\n').count().max(1)
    }

    fn input_line_col_for_cursor(&self) -> (usize, usize) {
        let cursor = self.input_cursor_char_index.min(self.input_char_len());
        let mut line = 0_usize;
        let mut col = 0_usize;
        for (idx, ch) in self.input.chars().enumerate() {
            if idx >= cursor {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn input_char_index_for_line_col(&self, target_line: usize, target_col: usize) -> usize {
        let lines = self.input.split('\n').collect::<Vec<_>>();
        let line_index = target_line.min(lines.len().saturating_sub(1));
        let column = target_col.min(lines[line_index].chars().count());
        let mut char_index = 0_usize;
        for line in lines.iter().take(line_index) {
            char_index += line.chars().count() + 1;
        }
        char_index + column
    }

    pub(crate) fn move_input_cursor_up(&mut self) {
        let (line, col) = self.input_line_col_for_cursor();
        if line == 0 {
            return;
        }
        let target_col = self.input_cursor_preferred_column.unwrap_or(col);
        self.input_cursor_char_index = self.input_char_index_for_line_col(line - 1, target_col);
        self.input_cursor_preferred_column = Some(target_col);
    }

    pub(crate) fn move_input_cursor_down(&mut self) {
        let (line, col) = self.input_line_col_for_cursor();
        if line + 1 >= self.input_line_count() {
            return;
        }
        let target_col = self.input_cursor_preferred_column.unwrap_or(col);
        self.input_cursor_char_index = self.input_char_index_for_line_col(line + 1, target_col);
        self.input_cursor_preferred_column = Some(target_col);
    }

    pub(crate) fn insert_input_char(&mut self, ch: char) {
        self.clear_chat_selection();
        self.clamp_input_cursor();
        let insert_at = Self::byte_index_for_char(&self.input, self.input_cursor_char_index);
        self.input.insert(insert_at, ch);
        self.input_cursor_char_index += 1;
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
    }

    pub(crate) fn insert_input_string(&mut self, text: String) {
        self.clear_chat_selection();
        self.clamp_input_cursor();
        let insert_at = Self::byte_index_for_char(&self.input, self.input_cursor_char_index);
        self.input.insert_str(insert_at, &text);
        self.input_cursor_char_index += text.chars().count();
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
    }

    pub(crate) fn backspace_input_char(&mut self) {
        self.clear_chat_selection();
        self.clamp_input_cursor();
        if self.input_cursor_char_index == 0 {
            return;
        }
        let remove_start = Self::byte_index_for_char(&self.input, self.input_cursor_char_index - 1);
        let remove_end = Self::byte_index_for_char(&self.input, self.input_cursor_char_index);
        self.input.replace_range(remove_start..remove_end, "");
        self.input_cursor_char_index -= 1;
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
    }

    pub(crate) fn insert_newline_at_cursor(&mut self) {
        self.insert_input_char('\n');
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
                    Some(ChatMessage::new(role, m.content.clone()))
                }
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
        Some(Message {
            role: display_role.to_string(),
            content,
            timestamp: Self::normalize_storage_timestamp(&timestamp),
            image_count,
            tool_calls_preview: tool_preview,
            tool_name,
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
    autocomplete::core::suggestions(input, models, workspaces, sessions)
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

pub(crate) fn ui_state_path_for_config(config: &AppConfig) -> PathBuf {
    Path::new(&config.storage.db_path)
        .parent()
        .map(|dir| dir.join("ui_state.json"))
        .unwrap_or_else(|| crate::config::default_data_dir().join("ui_state.json"))
}

pub(crate) fn load_ui_state_from_path(path: &Path) -> Option<UiRuntimeState> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<UiRuntimeState>(&raw).ok()
}

pub(crate) fn save_ui_state_to_path(path: &Path, state: &UiRuntimeState) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(state)
        .map_err(|err| io::Error::other(format!("serialize ui state: {err}")))?;
    fs::write(path, raw)
}

pub(crate) fn apply_saved_ui_state_with_startup_precedence(
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

pub async fn run(
    url_override: Option<String>,
    model_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = runtime::startup::bootstrap(url_override, model_override).await?;
    let mut terminal = runtime::startup::init_terminal()?;
    let mut last_saved_ui_state =
        runtime::startup::persist_initial_ui_state(&runtime.app, &runtime.ui_state_path);

    let run_result = runtime::event_loop::run(
        &mut runtime.app,
        &runtime.manager,
        &mut runtime.session,
        &runtime.config.defaults.model,
        &runtime.client,
        &runtime.input_bindings,
        &mut terminal,
        &runtime.ui_state_path,
        &mut last_saved_ui_state,
    )
    .await;

    runtime::shutdown::teardown_terminal(&mut terminal, &runtime.app, &runtime.ui_state_path)?;

    run_result
}
pub(crate) fn load_conversation_into_app(
    app: &mut App,
    manager: &WorkspaceManager,
    conversation_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    app.messages.clear();
    app.prompt_tokens_used = None;
    app.last_completion_tokens = None;
    app.peak_prompt_tokens = 0;
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
    app.restore_session_context_snapshot_if_match();
    Ok(())
}

pub(crate) fn command_may_change_model_context(prompt: &str) -> bool {
    let trimmed = prompt.trim_start();
    trimmed == "/model"
        || trimmed.starts_with("/model ")
        || trimmed == "/models"
        || trimmed.starts_with("/models ")
        || trimmed == "/url"
        || trimmed.starts_with("/url ")
}

pub(crate) async fn refresh_model_context_window(app: &mut App, client: &reqwest::Client) {
    if let Ok(Some(window)) =
        ollama::models::model_context_window(client, &app.url, &app.model).await
    {
        app.model_context_window = Some(window);
    }
    app.persist_active_session_context_snapshot();
}

pub(crate) fn workspace_help_text() -> String {
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

pub(crate) fn models_help_text() -> String {
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

pub(crate) fn session_help_text() -> String {
    [
        "Session commands:",
        "- /session list",
        "- /session new <title>",
        "- /session create <title> (alias of new)",
        "- /session switch <id|title>",
        "- /session rename <id> <new title>",
        "- /session rename <title> --to <new title>",
        "- /session delete (opens modal for selected session in sidebar)",
        "- /session delete <id|title> (optional explicit target)",
        "- Confirm delete modal with Enter, cancel with Esc",
    ]
    .join("\n")
}

pub(crate) fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .map(|chunk| chunk.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
}

pub(crate) fn unquote_command_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.as_bytes()[0];
        let last = trimmed.as_bytes()[trimmed.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
    }
    trimmed.to_string()
}

pub(crate) fn next_workspace_name(existing: &[crate::workspace::Workspace], base: &str) -> String {
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

pub(crate) fn handle_command(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut crate::workspace::WorkspaceSession,
    default_model: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = commands::parse_command(prompt);
    if parsed.is_empty() {
        return Ok(());
    }

    let parts = parsed.parts();
    let route = commands::route_command(parts[0]);

    if commands::handlers::utility::handle(app, route, parts)? {
        return Ok(());
    }
    if commands::handlers::models::handle(app, manager, route, parts)? {
        return Ok(());
    }
    if commands::handlers::misc::handle(app, manager, session, route, parts)? {
        return Ok(());
    }

    match route {
        commands::CommandRoute::Workspace => {
            if commands::handlers::workspace::handle(
                app,
                manager,
                session,
                default_model,
                prompt,
                parts,
            )? {
                return Ok(());
            }
        }
        commands::CommandRoute::Session => {
            if commands::handlers::session::handle(app, manager, session, prompt, parts)? {
                return Ok(());
            }
        }
        commands::CommandRoute::Unknown => {
            app.push_system("Unknown command. Use /commands".to_string());
            return Ok(());
        }
        _ => {
            app.push_system("Unknown command. Use /commands".to_string());
            return Ok(());
        }
    }

    Ok(())
}
pub(crate) fn tool_calls_preview(tool_calls: &[ChatToolCall]) -> Option<String> {
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
mod mod_tests;
