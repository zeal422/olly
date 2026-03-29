pub mod context;
pub mod input;
pub mod tui;

use std::{
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
pub struct WorkspaceEntry {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub image_count: usize,
    pub tool_calls_preview: Option<String>,
}

#[derive(Debug, Clone)]
pub struct App {
    pub input: String,
    pub messages: Vec<Message>,
    pub models: Vec<String>,
    pub workspaces: Vec<WorkspaceEntry>,
    pub sidebar_index: usize,
    pub model: String,
    pub url: String,
    pub workspace_name: String,
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
            sidebar_index: 0,
            model,
            url,
            workspace_name: String::from("Default Workspace"),
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
            quit_armed_until: None,
            should_quit: false,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        if let Some(toast) = &self.toast {
            if Instant::now() > toast.expires_at {
                self.toast = None;
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
        Some(trimmed)
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
            .map(|w| WorkspaceEntry {
                id: w.id,
                name: w.name,
            })
            .collect();
        self.clamp_sidebar_index();
        Ok(())
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
        self.models.len() + self.workspaces.len()
    }

    pub fn sidebar_item(&self) -> Option<SidebarItem> {
        if self.sidebar_item_count() == 0 {
            return None;
        }
        if self.sidebar_index < self.models.len() {
            return Some(SidebarItem::Model(self.sidebar_index));
        }
        let workspace_idx = self.sidebar_index.saturating_sub(self.models.len());
        if workspace_idx < self.workspaces.len() {
            Some(SidebarItem::Workspace(workspace_idx))
        } else {
            None
        }
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

    fn focus_active_workspace(&mut self, workspace_id: i64) {
        if let Some(index) = self.workspaces.iter().position(|w| w.id == workspace_id) {
            self.sidebar_index = self.models.len() + index;
        } else {
            self.clamp_sidebar_index();
        }
    }

    fn focus_active_model(&mut self) {
        if let Some(index) = self.models.iter().position(|m| m == &self.model) {
            self.sidebar_index = index;
        } else {
            self.clamp_sidebar_index();
        }
    }

    fn activate_sidebar_selection(
        &mut self,
        manager: &WorkspaceManager,
        session: &mut crate::workspace::WorkspaceSession,
        default_model: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.sidebar_item() {
            Some(SidebarItem::Model(model_index)) => {
                if let Some(model) = self.models.get(model_index).cloned() {
                    self.model = model;
                    self.status = String::from("Model switched from sidebar");
                    self.set_toast(
                        ToastLevel::Success,
                        format!("Model switched: {}", self.model),
                    );
                }
            }
            Some(SidebarItem::Workspace(workspace_index)) => {
                if let Some(workspace) = self.workspaces.get(workspace_index).cloned() {
                    if workspace.id == session.workspace_id {
                        self.set_toast(ToastLevel::Info, "Workspace already active");
                        return Ok(());
                    }

                    let next_session =
                        manager.open_workspace_session(workspace.id, default_model)?;
                    session.workspace_id = next_session.workspace_id;
                    session.conversation_id = next_session.conversation_id;
                    session.preset_id = next_session.preset_id;
                    self.workspace_name = workspace.name;
                    load_conversation_into_app(self, manager, session.conversation_id)?;
                    self.status = String::from("Workspace switched from sidebar");
                    self.set_toast(ToastLevel::Success, "Workspace switched");
                }
            }
            None => {}
        }
        Ok(())
    }

    fn autocomplete_command(&mut self) {
        let suggestions = autocomplete_suggestions(&self.input, &self.models);
        if let Some(suggestion) = next_autocomplete_value(&self.input, &suggestions) {
            self.input = suggestion;
        }
    }

    pub fn autocomplete_preview(&self) -> Option<String> {
        let suggestions = autocomplete_suggestions(&self.input, &self.models);
        suggestions
            .into_iter()
            .find(|candidate| candidate.trim() != self.input.trim())
    }

    pub fn autocomplete_options(&self) -> Vec<String> {
        autocomplete_suggestions(&self.input, &self.models)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItem {
    Model(usize),
    Workspace(usize),
}

fn autocomplete_suggestions(input: &str, models: &[String]) -> Vec<String> {
    let token = input.trim();
    if !token.starts_with('/') || token.contains('\n') {
        return Vec::new();
    }

    if token == "/model" || token == "/mod" {
        return vec![String::from("/model ")];
    }

    if let Some(partial) = token
        .strip_prefix("/model ")
        .or_else(|| token.strip_prefix("/mod "))
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

    if token.contains(' ') {
        return Vec::new();
    }

    COMMAND_SUGGESTIONS
        .iter()
        .find(|candidate| candidate.starts_with(token))
        .map(|found| vec![format!("{found} ")])
        .unwrap_or_default()
}

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

pub async fn run(
    url_override: Option<String>,
    model_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_or_create()?;
    let url = url_override.unwrap_or_else(|| config.ollama.url.clone());
    let model = model_override.unwrap_or_else(|| config.defaults.model.clone());
    let mut app = App::new(url, model);
    app.workspace_name = config.defaults.workspace.clone();
    app.syntax_highlighting = config.ui.syntax_highlighting;
    let input_bindings = input::InputBindings::from_config(&config.keybindings);

    let manager = WorkspaceManager::open(Path::new(&config.storage.db_path))?;
    let mut session = manager.bootstrap_default(&config.defaults.workspace, &app.model)?;

    load_conversation_into_app(&mut app, &manager, session.conversation_id)?;
    app.refresh_workspaces(&manager)?;
    if let Some(workspace) = app.workspaces.iter().find(|w| w.id == session.workspace_id) {
        app.workspace_name = workspace.name.clone();
    }
    app.focus_active_workspace(session.workspace_id);

    let client = reqwest::Client::new();
    match ollama::models::list_models(&client, &app.url).await {
        Ok(models) => {
            app.connection_status = ConnectionStatus::Online;
            if !models.is_empty() {
                if !models.iter().any(|m| m == &app.model) {
                    app.model = models[0].clone();
                }
                app.models = models;
                app.focus_active_model();
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

    let run_result = async {
        while !app.should_quit() {
            app.tick();
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

                    match action {
                        input::InputAction::Quit => app.request_quit(),
                        input::InputAction::ToggleHelp => app.toggle_help(),
                        input::InputAction::DismissOverlay => app.dismiss_overlay(),
                        input::InputAction::SwitchFocus => app.switch_focus(),
                        input::InputAction::AutoCompleteCommand => app.autocomplete_command(),
                        input::InputAction::ScrollUp(amount) => {
                            if app.focus == FocusPane::Workspaces {
                                app.sidebar_up(amount);
                            } else {
                                app.scroll_up(amount);
                            }
                        }
                        input::InputAction::ScrollDown(amount) => {
                            if app.focus == FocusPane::Workspaces {
                                app.sidebar_down(amount);
                            } else {
                                app.scroll_down(amount);
                            }
                        }
                        input::InputAction::Insert(c) => app.input.push(c),
                        input::InputAction::Backspace => {
                            app.input.pop();
                        }
                        input::InputAction::NewLine => {
                            if app.focus == FocusPane::Workspaces {
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
                                app.input.push('\n')
                            }
                        }
                        input::InputAction::Send => {
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

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    run_result
}

fn load_conversation_into_app(
    app: &mut App,
    manager: &WorkspaceManager,
    conversation_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    app.messages.clear();
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
                    app.focus_active_model();
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

    handle_command(app, manager, session, default_model, prompt)?;
    if let Err(err) = app.refresh_workspaces(manager) {
        app.push_system(format!("Workspace refresh failed: {err}"));
        app.set_toast(ToastLevel::Error, "Workspace refresh failed");
    }
    Ok(())
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
            if app.models.is_empty() {
                app.push_system("No models cached yet. Try `/models refresh`.".to_string());
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
                app.push_system(format!(
                    "Loaded models:\n{lines}\n\nUse `/model <name>` or `/models refresh`."
                ));
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
        "/workspace" => {
            if parts.len() < 2 {
                app.push_system("Usage: /workspace <list|switch|export|import> ...".to_string());
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
                                    if workspace.id == session.workspace_id {
                                        format!(
                                            "* {} (id: {}, active)",
                                            workspace.name, workspace.id
                                        )
                                    } else {
                                        format!("- {} (id: {})", workspace.name, workspace.id)
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            app.push_system(format!(
                                "Workspaces:\n{list}\n\nUse `/workspace switch <id|name>`."
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
                            app.workspace_name = workspace.name;
                            load_conversation_into_app(app, manager, session.conversation_id)?;
                            app.focus_active_workspace(session.workspace_id);
                            app.set_toast(ToastLevel::Success, "Workspace switched");
                        } else {
                            app.push_system(format!(
                                "Workspace not found: {selector}. Use `/workspace list`."
                            ));
                        }
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
                        if let Some(workspace) = manager
                            .list_workspaces()?
                            .into_iter()
                            .find(|w| w.id == session.workspace_id)
                        {
                            app.workspace_name = workspace.name;
                        }
                        load_conversation_into_app(app, manager, session.conversation_id)?;
                        app.focus_active_workspace(session.workspace_id);
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Workspace imported from {}", path.display()),
                        );
                    }
                    _ => app.push_system("Unknown workspace subcommand.".to_string()),
                }
            }
        }
        "/checkpoint" => {
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
                    app.focus_active_model();
                    app.set_toast(ToastLevel::Success, "Model switched");
                } else if let Some(prefix_match) = app
                    .models
                    .iter()
                    .find(|entry| entry.starts_with(requested))
                    .cloned()
                {
                    app.model = prefix_match;
                    app.focus_active_model();
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
    use super::{autocomplete_suggestions, next_autocomplete_value};

    #[test]
    fn autocomplete_promotes_mod_to_model_command() {
        let models = vec![String::from("llama3.2:latest")];
        assert_eq!(
            autocomplete_suggestions("/mod", &models),
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
            autocomplete_suggestions("/model dev", &models),
            vec![String::from("/model devstral-2:123b-cloud")]
        );
    }

    #[test]
    fn autocomplete_returns_empty_for_non_command_input() {
        let models = vec![String::from("llama3.2:latest")];
        assert!(autocomplete_suggestions("hello", &models).is_empty());
    }

    #[test]
    fn autocomplete_cycles_to_next_option() {
        let suggestions = vec![String::from("/model one"), String::from("/model two")];
        assert_eq!(
            next_autocomplete_value("/model one", &suggestions),
            Some(String::from("/model two"))
        );
    }
}
