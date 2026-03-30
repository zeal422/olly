pub mod autocomplete;
pub mod chat_pipeline;
pub mod commands;
pub mod context;
pub mod domain;
pub mod input;
pub mod runtime;
pub mod tui;

use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::{
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
];
pub(crate) const AUTO_SCROLL_TARGET: u16 = 30_000;

#[cfg(windows)]
pub(crate) fn maximize_console_window() {
    #[link(name = "Kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }

    #[link(name = "User32")]
    extern "system" {
        fn ShowWindow(hwnd: *mut core::ffi::c_void, command: i32) -> i32;
    }

    const SW_MAXIMIZE: i32 = 3;
    // SAFETY: We only call Win32 API on this process console window handle.
    // If no console exists, hwnd is null and this function becomes a no-op.
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
    }
}

#[cfg(windows)]
pub(crate) fn hide_console_scrollbars() {
    #[link(name = "Kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }

    #[link(name = "User32")]
    extern "system" {
        fn ShowScrollBar(hwnd: *mut core::ffi::c_void, w_bar: i32, b_show: i32) -> i32;
    }

    const SB_BOTH: i32 = 3;

    // SAFETY: This only targets the current process console window. Null handles are ignored.
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            let _ = ShowScrollBar(hwnd, SB_BOTH, 0);
        }
    }
}

#[cfg(windows)]
pub(crate) fn snap_console_window_to_cell_grid() {
    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct ConsoleFontInfoEx {
        cb_size: u32,
        n_font: u32,
        dw_font_size: Coord,
        font_family: u32,
        font_weight: u32,
        face_name: [u16; 32],
    }

    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
        fn GetStdHandle(std_handle: i32) -> *mut core::ffi::c_void;
        fn GetCurrentConsoleFontEx(
            console_output: *mut core::ffi::c_void,
            maximum_window: i32,
            console_current_font_ex: *mut ConsoleFontInfoEx,
        ) -> i32;
    }

    #[link(name = "User32")]
    extern "system" {
        fn GetClientRect(hwnd: *mut core::ffi::c_void, rect: *mut WinRect) -> i32;
        fn GetWindowRect(hwnd: *mut core::ffi::c_void, rect: *mut WinRect) -> i32;
        fn SetWindowPos(
            hwnd: *mut core::ffi::c_void,
            hwnd_insert_after: *mut core::ffi::c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    const STD_OUTPUT_HANDLE: i32 = -11;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;

    // SAFETY: Uses current process console window/handle and clamps values to valid ranges.
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd.is_null() {
            return;
        }

        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout_handle.is_null() || (stdout_handle as isize) == -1 {
            return;
        }

        let mut font = ConsoleFontInfoEx {
            cb_size: std::mem::size_of::<ConsoleFontInfoEx>() as u32,
            n_font: 0,
            dw_font_size: Coord { x: 0, y: 0 },
            font_family: 0,
            font_weight: 0,
            face_name: [0; 32],
        };
        if GetCurrentConsoleFontEx(stdout_handle, 0, &mut font as *mut ConsoleFontInfoEx) == 0 {
            return;
        }
        let font_w = i32::from(font.dw_font_size.x.max(1));
        let font_h = i32::from(font.dw_font_size.y.max(1));

        let mut client_rect = WinRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let mut window_rect = WinRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(hwnd, &mut client_rect as *mut WinRect) == 0
            || GetWindowRect(hwnd, &mut window_rect as *mut WinRect) == 0
        {
            return;
        }

        let client_w = (client_rect.right - client_rect.left).max(1);
        let client_h = (client_rect.bottom - client_rect.top).max(1);
        let window_w = (window_rect.right - window_rect.left).max(1);
        let window_h = (window_rect.bottom - window_rect.top).max(1);
        let non_client_w = window_w.saturating_sub(client_w);
        let non_client_h = window_h.saturating_sub(client_h);

        let snapped_client_w = ((client_w / font_w).max(1)).saturating_mul(font_w);
        let snapped_client_h = ((client_h / font_h).max(1)).saturating_mul(font_h);
        let target_w = snapped_client_w.saturating_add(non_client_w).max(100);
        let target_h = snapped_client_h.saturating_add(non_client_h).max(100);

        if target_w != window_w || target_h != window_h {
            let _ = SetWindowPos(
                hwnd,
                core::ptr::null_mut(),
                0,
                0,
                target_w,
                target_h,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(windows)]
pub(crate) fn sync_console_buffer_to_window(normalize_origin: bool) {
    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    struct ConsoleScreenBufferInfo {
        dw_size: Coord,
        dw_cursor_position: Coord,
        w_attributes: u16,
        sr_window: SmallRect,
        dw_maximum_window_size: Coord,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetStdHandle(std_handle: i32) -> *mut core::ffi::c_void;
        fn GetConsoleScreenBufferInfo(
            console_output: *mut core::ffi::c_void,
            console_screen_buffer_info: *mut ConsoleScreenBufferInfo,
        ) -> i32;
        fn SetConsoleWindowInfo(
            console_output: *mut core::ffi::c_void,
            absolute: i32,
            console_window: *const SmallRect,
        ) -> i32;
        fn SetConsoleScreenBufferSize(console_output: *mut core::ffi::c_void, size: Coord) -> i32;
    }

    const STD_OUTPUT_HANDLE: i32 = -11;

    // SAFETY: Uses this process console output handle and validates dimensions before applying.
    unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout_handle.is_null() || (stdout_handle as isize) == -1 {
            return;
        }
        let mut info = std::mem::MaybeUninit::<ConsoleScreenBufferInfo>::uninit();
        if GetConsoleScreenBufferInfo(stdout_handle, info.as_mut_ptr()) == 0 {
            return;
        }
        let info = info.assume_init();
        let window = info.sr_window;
        let width = window.right - window.left + 1;
        let height = window.bottom - window.top + 1;
        if width <= 0 || height <= 0 {
            return;
        }

        // Ensure buffer is never smaller than viewport before normalization.
        let needed_w = info.dw_size.x.max(width);
        let needed_h = info.dw_size.y.max(height);
        if needed_w != info.dw_size.x || needed_h != info.dw_size.y {
            let _ = SetConsoleScreenBufferSize(
                stdout_handle,
                Coord {
                    x: needed_w,
                    y: needed_h,
                },
            );
        }

        if normalize_origin && (window.left != 0 || window.top != 0) {
            let normalized_window = SmallRect {
                left: 0,
                top: 0,
                right: width - 1,
                bottom: height - 1,
            };
            let _ = SetConsoleWindowInfo(stdout_handle, 1, &normalized_window);
        }

        // Lock buffer to viewport to prevent native scrollbars in alternate screen.
        if info.dw_size.x != width || info.dw_size.y != height {
            let _ = SetConsoleScreenBufferSize(
                stdout_handle,
                Coord {
                    x: width,
                    y: height,
                },
            );
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn maximize_console_window() {}

#[cfg(not(windows))]
pub(crate) fn hide_console_scrollbars() {}

#[cfg(not(windows))]
pub(crate) fn snap_console_window_to_cell_grid() {}

#[cfg(not(windows))]
pub(crate) fn sync_console_buffer_to_window(_normalize_origin: bool) {}

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
    pub is_loading: bool,
    pub spinner_frame: usize,
    pub syntax_highlighting: bool,
    pub pending_images: Vec<PendingImage>,
    pub pending_workspace_delete: Option<PendingWorkspaceDelete>,
    pub pending_session_delete: Option<PendingSessionDelete>,
    pub theme: UiTheme,
    pub theme_picker: Option<ThemePickerState>,
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
            chat_scroll: AUTO_SCROLL_TARGET,
            is_loading: false,
            spinner_frame: 0,
            syntax_highlighting: true,
            pending_images: Vec::new(),
            pending_workspace_delete: None,
            pending_session_delete: None,
            theme: UiTheme::default(),
            theme_picker: None,
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
        self.messages.push(Message {
            role: role.into(),
            content: content.into(),
            timestamp: Self::now_label(),
            image_count,
            tool_calls_preview,
        });
        self.chat_scroll = AUTO_SCROLL_TARGET;
    }

    pub(crate) fn push_user(&mut self, content: String, image_count: usize) {
        self.push_message("You", content, image_count, None);
    }

    pub(crate) fn push_assistant(&mut self, content: String, tool_calls_preview: Option<String>) {
        self.push_message("Assistant", content, 0, tool_calls_preview);
    }

    pub(crate) fn push_tool(&mut self, content: String, tool_calls_preview: Option<String>) {
        self.push_message("Tool", content, 0, tool_calls_preview);
    }

    pub(crate) fn push_system(&mut self, content: String) {
        self.push_message("System", content, 0, None);
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
        self.chat_scroll = self.chat_scroll.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.chat_scroll = self
            .chat_scroll
            .saturating_add(amount)
            .min(AUTO_SCROLL_TARGET);
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
        self.clamp_input_cursor();
        let insert_at = Self::byte_index_for_char(&self.input, self.input_cursor_char_index);
        self.input.insert(insert_at, ch);
        self.input_cursor_char_index += 1;
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
    }

    pub(crate) fn backspace_input_char(&mut self) {
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

pub(crate) async fn execute_slash_command(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut crate::workspace::WorkspaceSession,
    default_model: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if prompt.trim() == "/themes" {
        app.open_theme_picker();
        return Ok(());
    }

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

pub(crate) fn confirm_pending_workspace_delete(
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

pub(crate) fn confirm_pending_session_delete(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut crate::workspace::WorkspaceSession,
) -> Result<(), Box<dyn std::error::Error>> {
    let pending = app.pending_session_delete.clone();
    let Some(pending_delete) = pending else {
        return Ok(());
    };

    if pending_delete.workspace_id <= 0 {
        app.pending_session_delete = None;
        return Ok(());
    }

    let sessions = manager.list_conversations(pending_delete.workspace_id)?;
    let target = sessions
        .iter()
        .find(|conversation| conversation.id == pending_delete.id)
        .cloned();
    let Some(target_session) = target else {
        app.pending_session_delete = None;
        app.push_system("Session to delete was not found.".to_string());
        return Ok(());
    };

    manager.delete_session(target_session.id)?;
    app.pending_session_delete = None;

    if pending_delete.workspace_id == session.workspace_id
        && target_session.id == session.conversation_id
    {
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
    app.refresh_workspaces(manager)?;
    app.sync_active_workspace_label(session.workspace_id);
    app.focus_active_session(session.workspace_id, session.conversation_id);
    app.set_toast(ToastLevel::Success, "Session deleted");
    Ok(())
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
pub(crate) fn format_tool_calls(tool_calls: &[ChatToolCall]) -> String {
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
mod tests {
    use super::{
        apply_saved_ui_state_with_startup_precedence, autocomplete_suggestions,
        confirm_pending_session_delete, confirm_pending_workspace_delete, handle_command,
        is_cloud_model_name, load_ui_state_from_path, models_help_text, next_autocomplete_value,
        save_ui_state_to_path, session_help_text, workspace_help_text, App, FocusPane, ModelBucket,
        SessionContextSnapshot, SessionEntry, SidebarItem, UiRuntimeState, UiTheme, WorkspaceEntry,
    };
    use crate::workspace::{manager::WorkspaceManager, WorkspaceSession};
    use std::{
        fs,
        path::PathBuf,
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

    fn unique_temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "olly-app-tests-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create test root");
        root
    }

    fn setup_app_state(label: &str) -> (PathBuf, WorkspaceManager, WorkspaceSession, App) {
        let root = unique_temp_root(label);
        let db_path = root.join("workspaces.db");
        let manager = WorkspaceManager::open(&db_path).expect("open manager");
        let session = manager
            .bootstrap_default("Default Workspace", "llama3.2:latest")
            .expect("bootstrap default workspace");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.active_workspace_id = session.workspace_id;
        app.active_session_id = session.conversation_id;
        app.workspace_name = String::from("Default Workspace");
        app.refresh_workspaces(&manager)
            .expect("refresh workspaces after bootstrap");
        app.refresh_sessions(&manager, session.workspace_id)
            .expect("refresh sessions after bootstrap");
        app.sync_active_workspace_label(session.workspace_id);
        app.focus_active_session(session.workspace_id, session.conversation_id);
        (root, manager, session, app)
    }

    fn select_workspace_in_sidebar(app: &mut App, workspace_id: i64) {
        let entries = app.sidebar_entries();
        let target_index = entries
            .iter()
            .position(|entry| match entry {
                SidebarItem::Workspace(index) => app
                    .workspaces
                    .get(*index)
                    .map(|workspace| workspace.id == workspace_id)
                    .unwrap_or(false),
                _ => false,
            })
            .expect("workspace should exist in sidebar");
        app.sidebar_index = target_index;
    }

    fn select_session_in_sidebar(app: &mut App, workspace_id: i64, session_id: i64) {
        let entries = app.sidebar_entries();
        let target_index = entries
            .iter()
            .position(|entry| match entry {
                SidebarItem::WorkspaceSession(workspace_index, session_index) => app
                    .workspaces
                    .get(*workspace_index)
                    .and_then(|workspace| {
                        workspace
                            .sessions
                            .get(*session_index)
                            .map(|session| (workspace.id, session.id))
                    })
                    .map(|(wid, sid)| wid == workspace_id && sid == session_id)
                    .unwrap_or(false),
                _ => false,
            })
            .expect("session should exist in sidebar");
        app.sidebar_index = target_index;
    }

    fn latest_system_message(app: &App) -> Option<&str> {
        app.messages
            .iter()
            .rev()
            .find(|message| message.role == "System")
            .map(|message| message.content.as_str())
    }

    #[test]
    fn cloud_model_classifier_handles_common_patterns() {
        assert!(is_cloud_model_name("minimax-m2.7:cloud"));
        assert!(is_cloud_model_name("devstral-2:123b-cloud"));
        assert!(!is_cloud_model_name("qwen2.5-coder:7b"));
    }

    #[test]
    fn input_editor_inserts_on_empty_line_after_vertical_move() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.set_input_text(String::from("one\n\nthree"));
        app.move_input_cursor_up();
        app.insert_input_char('x');

        assert_eq!(app.input, "one\nx\nthree");
        let (line, col) = app.input_line_col_for_cursor();
        assert_eq!((line, col), (1, 1));
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
                model: Some(String::from("llama3.2:latest")),
                theme: UiTheme::EmeraldDusk,
                models_collapsed: true,
                cloud_models_collapsed: true,
                local_models_collapsed: false,
                workspaces_collapsed: true,
                collapsed_workspace_ids: vec![2, 4],
                sidebar_index: 7,
                session_context: None,
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
        assert_eq!(next.theme, UiTheme::EmeraldDusk);
        assert_eq!(next.model, "llama3.2:latest");
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
            model: Some(String::from("qwen3.5:cloud")),
            theme: UiTheme::SakuraNight,
            models_collapsed: true,
            cloud_models_collapsed: false,
            local_models_collapsed: true,
            workspaces_collapsed: false,
            collapsed_workspace_ids: vec![10, 3],
            sidebar_index: 5,
            session_context: Some(SessionContextSnapshot {
                workspace_id: 10,
                session_id: 11,
                model: String::from("qwen3.5:cloud"),
                model_context_window: Some(262_144),
                prompt_tokens_used: Some(4_096),
                last_completion_tokens: Some(128),
                peak_prompt_tokens: 4_096,
            }),
        };

        save_ui_state_to_path(&path, &state).expect("save ui state");
        let loaded = load_ui_state_from_path(&path).expect("load ui state");
        assert_eq!(loaded, state);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ui_state_backward_compat_missing_new_fields_deserializes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("olly-ui-state-compat-{unique}"));
        let path = root.join("ui_state.json");
        fs::create_dir_all(&root).expect("create compat ui-state root");
        let legacy_raw = r#"{
  "theme": "emerald-dusk",
  "models_collapsed": true,
  "cloud_models_collapsed": false,
  "local_models_collapsed": false,
  "workspaces_collapsed": false,
  "collapsed_workspace_ids": [1, 2],
  "sidebar_index": 3
}"#;
        fs::write(&path, legacy_raw).expect("write legacy ui_state json");

        let loaded = load_ui_state_from_path(&path).expect("load legacy ui_state");
        assert_eq!(loaded.model, None);
        assert_eq!(loaded.session_context, None);
        assert_eq!(loaded.theme, UiTheme::EmeraldDusk);
        assert_eq!(loaded.sidebar_index, 3);
        assert_eq!(loaded.collapsed_workspace_ids, vec![1, 2]);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn startup_model_precedence_cli_override_beats_saved_ui_model() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("qwen3.5:cloud"),
        );
        let saved = UiRuntimeState {
            model: Some(String::from("llama3.2:latest")),
            theme: UiTheme::EmeraldDusk,
            models_collapsed: false,
            cloud_models_collapsed: false,
            local_models_collapsed: false,
            workspaces_collapsed: false,
            collapsed_workspace_ids: Vec::new(),
            sidebar_index: 0,
            session_context: None,
        };

        apply_saved_ui_state_with_startup_precedence(&mut app, saved, "qwen3.5:cloud", true);

        assert_eq!(app.model, "qwen3.5:cloud");
    }

    #[test]
    fn startup_model_precedence_uses_saved_ui_model_without_cli_override() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        let saved = UiRuntimeState {
            model: Some(String::from("qwen3.5:cloud")),
            theme: UiTheme::EmeraldDusk,
            models_collapsed: false,
            cloud_models_collapsed: false,
            local_models_collapsed: false,
            workspaces_collapsed: false,
            collapsed_workspace_ids: Vec::new(),
            sidebar_index: 0,
            session_context: None,
        };

        apply_saved_ui_state_with_startup_precedence(&mut app, saved, "llama3.2:latest", false);

        assert_eq!(app.model, "qwen3.5:cloud");
    }

    #[test]
    fn restore_session_context_snapshot_applies_when_model_and_session_match() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("qwen3.5:cloud"),
        );
        let state = UiRuntimeState {
            model: Some(String::from("qwen3.5:cloud")),
            theme: UiTheme::EmeraldDusk,
            models_collapsed: false,
            cloud_models_collapsed: false,
            local_models_collapsed: false,
            workspaces_collapsed: false,
            collapsed_workspace_ids: Vec::new(),
            sidebar_index: 0,
            session_context: Some(SessionContextSnapshot {
                workspace_id: 3,
                session_id: 9,
                model: String::from("qwen3.5:cloud"),
                model_context_window: Some(262_144),
                prompt_tokens_used: Some(3_210),
                last_completion_tokens: Some(78),
                peak_prompt_tokens: 3_210,
            }),
        };
        app.apply_ui_state(state);
        app.active_workspace_id = 3;
        app.active_session_id = 9;
        app.model_context_window = None;
        app.prompt_tokens_used = None;
        app.last_completion_tokens = None;
        app.peak_prompt_tokens = 0;

        app.restore_session_context_snapshot_if_match();

        assert_eq!(app.model_context_window, Some(262_144));
        assert_eq!(app.prompt_tokens_used, Some(3_210));
        assert_eq!(app.last_completion_tokens, Some(78));
        assert_eq!(app.peak_prompt_tokens, 3_210);
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
    fn autocomplete_includes_themes_command() {
        let models = vec![String::from("llama3.2:latest")];
        assert_eq!(
            autocomplete_suggestions("/the", &models, &sample_workspaces(), &sample_sessions()),
            vec![String::from("/themes ")]
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
    fn autocomplete_session_delete_suggests_single_command() {
        let models = vec![String::from("llama3.2:latest")];
        let suggestions = autocomplete_suggestions(
            "/session delete ",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        );
        assert_eq!(suggestions, vec![String::from("/session delete")]);
    }

    #[test]
    fn workspace_create_without_name_uses_incremented_default_name() {
        let (root, manager, mut session, mut app) =
            setup_app_state("workspace-create-default-name");
        let create_root = root.join("workspace-create-root");
        fs::create_dir_all(&create_root).expect("create workspace root");
        manager
            .create_workspace("New Workspace", "llama3.2:latest", Some(&create_root))
            .expect("create existing New Workspace");
        app.refresh_workspaces(&manager)
            .expect("refresh workspaces before create");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!("/workspace create --path {}", create_root.display()),
        )
        .expect("workspace create command");

        let names = manager
            .list_workspaces()
            .expect("list workspaces")
            .into_iter()
            .map(|workspace| workspace.name)
            .collect::<Vec<_>>();
        assert!(names.iter().any(|name| name == "New Workspace"));
        assert!(names
            .iter()
            .any(|name| name.starts_with("New Workspace ") && name != "New Workspace"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_rename_rejects_duplicate_name() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-rename-duplicate");
        let taken_root = root.join("taken-workspace-root");
        fs::create_dir_all(&taken_root).expect("create taken workspace root");
        manager
            .create_workspace("Taken Name", "llama3.2:latest", Some(&taken_root))
            .expect("create workspace with taken name");
        app.refresh_workspaces(&manager)
            .expect("refresh workspaces before rename");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/workspace rename Taken Name",
        )
        .expect("workspace rename command");

        let names = manager
            .list_workspaces()
            .expect("list workspaces")
            .into_iter()
            .map(|workspace| workspace.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names
                .iter()
                .filter(|name| name.as_str() == "Taken Name")
                .count(),
            1
        );
        assert!(names.iter().any(|name| name == "Default Workspace"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_delete_uses_sidebar_selection_and_confirmation_flow() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-delete-selection");
        let delete_root = root.join("delete-workspace-root");
        fs::create_dir_all(&delete_root).expect("create delete workspace root");
        let deletable = manager
            .create_workspace("Delete Me", "llama3.2:latest", Some(&delete_root))
            .expect("create deletable workspace");

        app.refresh_workspaces(&manager)
            .expect("refresh workspaces before delete");
        app.refresh_sessions(&manager, session.workspace_id)
            .expect("refresh sessions before delete");
        select_workspace_in_sidebar(&mut app, deletable.workspace_id);

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/workspace delete",
        )
        .expect("open workspace delete confirmation");
        assert_eq!(
            app.pending_workspace_delete
                .as_ref()
                .map(|pending| pending.id),
            Some(deletable.workspace_id)
        );

        confirm_pending_workspace_delete(&mut app, &manager, &mut session, "llama3.2:latest")
            .expect("confirm delete");
        assert!(app.pending_workspace_delete.is_none());
        assert!(!manager
            .list_workspaces()
            .expect("list workspaces after delete")
            .iter()
            .any(|workspace| workspace.id == deletable.workspace_id));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_delete_uses_sidebar_selection_and_confirmation_flow() {
        let (root, manager, mut session, mut app) = setup_app_state("session-delete-selection");
        let target = manager
            .create_session(session.workspace_id, Some("Delete This Session"))
            .expect("create target session");

        app.refresh_workspaces(&manager)
            .expect("refresh workspaces before delete");
        app.refresh_sessions(&manager, session.workspace_id)
            .expect("refresh sessions before delete");
        select_session_in_sidebar(&mut app, session.workspace_id, target);

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session delete",
        )
        .expect("open session delete confirmation");
        assert_eq!(
            app.pending_session_delete
                .as_ref()
                .map(|pending| pending.id),
            Some(target)
        );

        confirm_pending_session_delete(&mut app, &manager, &mut session).expect("confirm delete");
        assert!(app.pending_session_delete.is_none());
        assert!(!manager
            .list_conversations(session.workspace_id)
            .expect("list sessions after delete")
            .iter()
            .any(|conversation| conversation.id == target));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_delete_targets_sidebar_selection_from_other_workspace() {
        let (root, manager, mut session, mut app) =
            setup_app_state("session-delete-selection-cross-workspace");
        let original_workspace_id = session.workspace_id;
        let original_session_id = session.conversation_id;

        let other_root = root.join("other-workspace-root");
        fs::create_dir_all(&other_root).expect("create other workspace root");
        let other_workspace = manager
            .create_workspace(
                "Delete Target Workspace",
                "llama3.2:latest",
                Some(&other_root),
            )
            .expect("create other workspace");

        app.refresh_workspaces(&manager)
            .expect("refresh workspaces before cross-workspace delete");
        app.refresh_sessions(&manager, session.workspace_id)
            .expect("refresh active workspace sessions before cross-workspace delete");
        select_session_in_sidebar(
            &mut app,
            other_workspace.workspace_id,
            other_workspace.conversation_id,
        );

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session delete",
        )
        .expect("open cross-workspace session delete confirmation");
        assert_eq!(
            app.pending_session_delete
                .as_ref()
                .map(|pending| (pending.workspace_id, pending.id)),
            Some((
                other_workspace.workspace_id,
                other_workspace.conversation_id
            ))
        );

        confirm_pending_session_delete(&mut app, &manager, &mut session)
            .expect("confirm cross-workspace delete");

        assert_eq!(session.workspace_id, original_workspace_id);
        assert_eq!(session.conversation_id, original_session_id);
        assert_eq!(app.active_workspace_id, original_workspace_id);
        assert_eq!(app.active_session_id, original_session_id);
        assert!(!manager
            .list_conversations(other_workspace.workspace_id)
            .expect("list conversations in other workspace after delete")
            .iter()
            .any(|conversation| conversation.id == other_workspace.conversation_id));
        assert!(!app
            .workspaces
            .iter()
            .find(|workspace| workspace.id == other_workspace.workspace_id)
            .expect("other workspace should remain")
            .sessions
            .iter()
            .any(|entry| entry.id == other_workspace.conversation_id));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_create_sets_active_session_and_loads_empty_messages() {
        let (root, manager, mut session, mut app) = setup_app_state("session-create-active");
        let previous = session.conversation_id;

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session create Planning",
        )
        .expect("session create command");

        assert_ne!(session.conversation_id, previous);
        assert_eq!(app.active_session_id, session.conversation_id);
        assert!(manager
            .load_messages(session.conversation_id)
            .expect("load new session messages")
            .is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_delete_falls_back_then_enters_zero_session_state() {
        let (root, manager, mut session, mut app) = setup_app_state("session-delete-fallback");
        let main_session = session.conversation_id;
        let secondary = manager
            .create_session(session.workspace_id, Some("Secondary"))
            .expect("create secondary session");

        app.refresh_sessions(&manager, session.workspace_id)
            .expect("refresh sessions after create");
        session.conversation_id = secondary;
        app.active_session_id = secondary;

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!("/session delete {secondary}"),
        )
        .expect("open secondary delete modal");
        assert_eq!(
            app.pending_session_delete
                .as_ref()
                .map(|pending| pending.id),
            Some(secondary)
        );
        confirm_pending_session_delete(&mut app, &manager, &mut session)
            .expect("confirm secondary delete");

        assert_eq!(session.conversation_id, main_session);
        assert_eq!(app.active_session_id, main_session);

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!("/session delete {main_session}"),
        )
        .expect("open final delete modal");
        assert_eq!(
            app.pending_session_delete
                .as_ref()
                .map(|pending| pending.id),
            Some(main_session)
        );
        confirm_pending_session_delete(&mut app, &manager, &mut session)
            .expect("confirm final delete");

        assert_eq!(session.conversation_id, 0);
        assert_eq!(app.active_session_id, 0);
        assert_eq!(app.active_session_title, "No Session");
        assert_eq!(app.status, "No session active");
        assert!(app.messages.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_rename_supports_id_and_title_with_to_syntax() {
        let (root, manager, mut session, mut app) = setup_app_state("session-rename-paths");
        let target_id = manager
            .create_session(session.workspace_id, Some("Research Notes"))
            .expect("create target session");
        app.refresh_sessions(&manager, session.workspace_id)
            .expect("refresh sessions before rename");
        session.conversation_id = target_id;
        app.active_session_id = target_id;

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!("/session rename {target_id} Renamed By Id"),
        )
        .expect("rename by id");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session rename Renamed By Id --to Final Session Name",
        )
        .expect("rename by title --to");

        let renamed = manager
            .list_conversations(session.workspace_id)
            .expect("list conversations after rename")
            .into_iter()
            .find(|conversation| conversation.id == target_id)
            .expect("target session should exist");
        assert_eq!(renamed.title.as_deref(), Some("Final Session Name"));
        assert_eq!(app.active_session_title, "Final Session Name");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn model_command_handles_exact_prefix_and_unknown_values() {
        let (root, manager, mut session, mut app) = setup_app_state("model-command-cases");
        app.models = vec![
            String::from("qwen2.5-coder:7b"),
            String::from("gemini-3-flash-preview:cloud"),
        ];

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/model gemini-3-flash-preview:cloud",
        )
        .expect("exact model command");
        assert_eq!(app.model, "gemini-3-flash-preview:cloud");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/model qwen2.5",
        )
        .expect("prefix model command");
        assert_eq!(app.model, "qwen2.5-coder:7b");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/model my-custom-model:cloud",
        )
        .expect("unknown model command");
        assert_eq!(app.model, "my-custom-model:cloud");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn url_command_updates_runtime_url() {
        let (root, manager, mut session, mut app) = setup_app_state("url-command-update");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/url http://127.0.0.1:11435",
        )
        .expect("url command");
        assert_eq!(app.url, "http://127.0.0.1:11435");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_create_without_args_shows_usage() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-create-usage");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/workspace create",
        )
        .expect("workspace create usage branch");

        let message = latest_system_message(&app).expect("expected system usage message");
        assert!(message.contains("Usage: /workspace create <name> --path <folder>"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_set_path_missing_path_shows_usage() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-set-path-usage");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/workspace set-path Default Workspace",
        )
        .expect("workspace set-path usage branch");

        assert_eq!(
            latest_system_message(&app),
            Some("Usage: /workspace set-path <id|name> --path <folder>")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_create_supports_quoted_name_and_path() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-create-quoted");
        let quoted_path = root.join("quoted workspace path");
        fs::create_dir_all(&quoted_path).expect("create quoted workspace path");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!(
                "/workspace create \"Quoted Workspace\" --path \"{}\"",
                quoted_path.display()
            ),
        )
        .expect("workspace create with quoted args");

        let created = manager
            .list_workspaces()
            .expect("list workspaces after create")
            .into_iter()
            .find(|workspace| workspace.name == "Quoted Workspace")
            .expect("quoted workspace should exist");
        assert_eq!(session.workspace_id, created.id);
        assert_eq!(app.workspace_name, "Quoted Workspace");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_set_path_supports_quoted_selector_and_path() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-set-path-quoted");
        let quoted_path = root.join("path with spaces");
        fs::create_dir_all(&quoted_path).expect("create quoted path");
        let canonical = fs::canonicalize(&quoted_path)
            .expect("canonicalize quoted path")
            .to_string_lossy()
            .into_owned();

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!(
                "/workspace set-path \"Default Workspace\" --path \"{}\"",
                quoted_path.display()
            ),
        )
        .expect("workspace set-path with quoted args");

        let updated = manager
            .list_workspaces()
            .expect("list workspaces after set-path")
            .into_iter()
            .find(|workspace| workspace.id == session.workspace_id)
            .expect("active workspace should exist");
        assert_eq!(updated.root_path.as_deref(), Some(canonical.as_str()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_markdown_supports_quoted_output_path() {
        let (root, manager, mut session, mut app) = setup_app_state("export-md-quoted");
        manager
            .save_message(
                session.conversation_id,
                "assistant",
                "quoted export payload",
            )
            .expect("save message for export");
        let output = root.join("exports folder").join("conversation file.md");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!("/export md \"{}\"", output.display()),
        )
        .expect("export markdown with quoted path");

        assert!(output.exists());
        let raw = fs::read_to_string(&output).expect("read exported markdown");
        assert!(raw.contains("quoted export payload"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_rename_supports_quoted_titles_with_to_syntax() {
        let (root, manager, mut session, mut app) = setup_app_state("session-rename-quoted");
        let target_id = manager
            .create_session(session.workspace_id, Some("Old Multi Word Title"))
            .expect("create target session");
        app.refresh_sessions(&manager, session.workspace_id)
            .expect("refresh sessions");
        session.conversation_id = target_id;
        app.active_session_id = target_id;

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session rename \"Old Multi Word Title\" --to \"New Multi Word Title\"",
        )
        .expect("session rename with quoted titles");

        let renamed = manager
            .list_conversations(session.workspace_id)
            .expect("list sessions after rename")
            .into_iter()
            .find(|entry| entry.id == target_id)
            .expect("target session should exist");
        assert_eq!(renamed.title.as_deref(), Some("New Multi Word Title"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_switch_unknown_keeps_active_workspace() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-switch-unknown");
        let before_id = session.workspace_id;
        let before_name = app.workspace_name.clone();

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/workspace switch Missing Workspace",
        )
        .expect("workspace switch unknown branch");

        assert_eq!(session.workspace_id, before_id);
        assert_eq!(app.active_workspace_id, before_id);
        assert_eq!(app.workspace_name, before_name);
        let message = latest_system_message(&app).expect("expected not-found message");
        assert!(message.contains("Workspace not found: Missing Workspace"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_switch_unknown_keeps_active_session() {
        let (root, manager, mut session, mut app) = setup_app_state("session-switch-unknown");
        let before_id = session.conversation_id;

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session switch 999999",
        )
        .expect("session switch unknown branch");

        assert_eq!(session.conversation_id, before_id);
        assert_eq!(app.active_session_id, before_id);
        let message = latest_system_message(&app).expect("expected not-found message");
        assert!(message.contains("Session not found: 999999"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_rename_invalid_syntax_shows_usage() {
        let (root, manager, mut session, mut app) = setup_app_state("session-rename-usage");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session rename onlyone",
        )
        .expect("session rename usage branch");

        let message = latest_system_message(&app).expect("expected usage message");
        assert!(message.contains("Usage: /session rename <id> <new title>"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_delete_unknown_id_does_not_mutate_active_session() {
        let (root, manager, mut session, mut app) = setup_app_state("session-delete-unknown");
        let before_id = session.conversation_id;

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session delete 777777",
        )
        .expect("session delete unknown branch");

        assert_eq!(session.conversation_id, before_id);
        assert_eq!(app.active_session_id, before_id);
        let message = latest_system_message(&app).expect("expected not-found message");
        assert!(message.contains("Session not found: 777777"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_delete_without_selection_and_without_active_shows_fallback_message() {
        let (root, manager, mut session, mut app) = setup_app_state("session-delete-fallback-msg");
        app.active_session_id = 0;
        session.workspace_id = 0;
        app.focus = FocusPane::Chat;

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/session delete",
        )
        .expect("session delete fallback branch");

        let message = latest_system_message(&app).expect("expected fallback system message");
        assert!(message.contains("No active workspace."));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_delete_unknown_selector_shows_not_found_message() {
        let (root, manager, mut session, mut app) = setup_app_state("workspace-delete-not-found");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/workspace delete missing-workspace",
        )
        .expect("workspace delete unknown selector");

        let message = latest_system_message(&app).expect("expected not-found message");
        assert!(message.contains("Workspace not found for delete selection."));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_unknown_format_shows_explicit_error() {
        let (root, manager, mut session, mut app) = setup_app_state("export-unknown-format");
        let export_path = root.join("conversation.txt");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            &format!("/export txt {}", export_path.display()),
        )
        .expect("export unknown format branch");

        assert_eq!(
            latest_system_message(&app),
            Some("Unknown export format. Use json or md.")
        );
        assert!(!export_path.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn model_command_without_arg_shows_known_models_help() {
        let (root, manager, mut session, mut app) = setup_app_state("model-command-help");
        app.models = vec![
            String::from("qwen2.5-coder:7b"),
            String::from("gemini-3-flash-preview:cloud"),
        ];
        app.model = String::from("qwen2.5-coder:7b");

        handle_command(
            &mut app,
            &manager,
            &mut session,
            "llama3.2:latest",
            "/model",
        )
        .expect("model help branch");

        let message = latest_system_message(&app).expect("expected model help message");
        assert!(message.contains("Usage: /model <name>"));
        assert!(message.contains("Known models:"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn clamp_sidebar_index_clamps_to_last_valid_sidebar_entry() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.sidebar_index = 42;
        app.models_collapsed = true;
        app.cloud_models_collapsed = true;
        app.local_models_collapsed = true;
        app.workspaces_collapsed = true;
        app.workspaces.clear();
        app.models.clear();
        app.clamp_sidebar_index();

        assert_eq!(app.sidebar_entries().len(), 2);
        assert_eq!(app.sidebar_index, 1);
    }

    #[test]
    fn sync_active_session_label_falls_back_to_latest_when_stale() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.sessions = vec![
            SessionEntry {
                id: 21,
                title: String::from("First"),
            },
            SessionEntry {
                id: 22,
                title: String::from("Latest"),
            },
        ];
        app.active_session_id = 999;

        app.sync_active_session_label();

        assert_eq!(app.active_session_id, 22);
        assert_eq!(app.active_session_title, "Latest");
    }

    #[test]
    fn clear_active_workspace_state_resets_all_related_fields() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.active_workspace_id = 5;
        app.workspace_name = String::from("Some Workspace");
        app.workspace_root_path = Some(String::from("C:/tmp/workspace"));
        app.active_session_id = 77;
        app.active_session_title = String::from("Custom Session");
        app.sessions = vec![SessionEntry {
            id: 77,
            title: String::from("Custom Session"),
        }];
        app.messages.push(super::Message {
            role: String::from("System"),
            content: String::from("stale"),
            timestamp: String::from("00:00"),
            image_count: 0,
            tool_calls_preview: None,
        });
        app.status = String::from("busy");

        app.clear_active_workspace_state();

        assert_eq!(app.active_workspace_id, 0);
        assert_eq!(app.workspace_name, "No Workspace");
        assert_eq!(app.workspace_root_path, None);
        assert_eq!(app.active_session_id, 0);
        assert_eq!(app.active_session_title, "No Session");
        assert!(app.sessions.is_empty());
        assert!(app.messages.is_empty());
        assert_eq!(app.status, "No workspace active");
    }

    #[test]
    fn app_state_views_reflect_current_values() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.focus = FocusPane::Workspaces;
        app.show_help = true;
        app.sidebar_index = 3;
        app.active_workspace_id = 11;
        app.active_session_id = 22;
        app.workspace_name = String::from("Refactor WS");
        app.active_session_title = String::from("Main");
        app.workspace_root_path = Some(String::from("C:/CODING/olly"));
        app.model_context_window = Some(262_144);
        app.prompt_tokens_used = Some(1024);
        app.last_completion_tokens = Some(256);
        app.peak_prompt_tokens = 4096;

        let ui = app.ui_state_view();
        assert_eq!(ui.focus, FocusPane::Workspaces);
        assert!(ui.show_help);
        assert_eq!(ui.sidebar_index, 3);

        let session = app.session_state_view();
        assert_eq!(session.active_workspace_id, 11);
        assert_eq!(session.active_session_id, 22);
        assert_eq!(session.workspace_name, "Refactor WS");
        assert_eq!(session.active_session_title, "Main");
        assert_eq!(session.workspace_root_path, Some("C:/CODING/olly"));

        let telemetry = app.telemetry_state_view();
        assert_eq!(telemetry.model_context_window, Some(262_144));
        assert_eq!(telemetry.prompt_tokens_used, Some(1024));
        assert_eq!(telemetry.last_completion_tokens, Some(256));
        assert_eq!(telemetry.peak_prompt_tokens, 4096);
    }

    #[test]
    fn workspace_help_text_snapshot_stable() {
        let expected = [
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
        .join("\n");
        assert_eq!(workspace_help_text(), expected);
    }

    #[test]
    fn session_help_text_snapshot_stable() {
        let expected = [
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
        .join("\n");
        assert_eq!(session_help_text(), expected);
    }

    #[test]
    fn models_help_text_snapshot_stable() {
        let expected = [
            "Model commands:",
            "- /models",
            "- /models refresh",
            "- /models add <model-name-or-ollama-library-url> (pull local or enable cloud)",
            "- /models remove <model-name>",
            "- /model <name> (switch active model)",
        ]
        .join("\n");
        assert_eq!(models_help_text(), expected);
    }
}
