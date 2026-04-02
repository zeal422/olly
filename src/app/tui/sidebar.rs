//! Sidebar rendering - status bar and models panel.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{
    domain::{ModelBucket, SidebarItem, SidebarSection},
    state::{ConnectionStatus, FocusPane},
    App,
};

use super::palette::{sidebar_background, ThemePalette};
use super::{scroll_window, truncate_end, truncate_middle};

const SPINNER_FRAMES: &[&str] = &["|", "/", "-", "\\"];

/// Render the status bar at the top of the UI.
pub(super) fn render_status_bar(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    palette: ThemePalette,
) {
    let connection = match app.connection_status {
        ConnectionStatus::Unknown => "Unknown",
        ConnectionStatus::Online => "Online",
        ConnectionStatus::Offline => "Offline",
    };
    let spinner = if app.is_loading {
        SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()]
    } else {
        " "
    };
    let hl_mode = if app.syntax_highlighting {
        "hl:on"
    } else {
        "hl:off"
    };
    let tools_status = if app.tools_state.enabled {
        format!("tools:on/{}", app.tools_state.policy.as_str())
    } else {
        "tools:off".to_string()
    };
    let workspace_path = app
        .workspace_root_path
        .as_deref()
        .unwrap_or("(path not set)");
    let status = format!(
        "{spinner} ws={} | path={} | session={}#{} | msgs={} | model={} | conn={} | imgs={} | {hl_mode} | {} | {} | F1 help",
        app.workspace_name,
        truncate_middle(workspace_path, 36),
        app.active_session_title,
        app.active_session_id,
        app.messages.len(),
        app.model,
        connection,
        app.pending_images.len(),
        tools_status,
        app.status
    );
    frame.render_widget(
        Paragraph::new(status).style(
            Style::default()
                .fg(palette.status_fg)
                .bg(palette.status_bg)
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
}

/// Render the models panel in the sidebar.
pub(super) fn render_models_panel(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    palette: ThemePalette,
) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let selected = app.sidebar_item();
    let mut lines = Vec::new();
    let line_width = area.width.saturating_sub(2) as usize;
    let mut push_sidebar_line = |raw: String, is_selected: bool| {
        lines.push(sidebar_line_with_marquee(
            &raw,
            is_selected,
            line_width,
            app.spinner_frame,
        ));
    };
    for entry in app.sidebar_entries() {
        match entry {
            SidebarItem::SectionHeader(SidebarSection::Models) => {
                let marker = if selected == Some(entry) { ">" } else { " " };
                let fold = if app.models_collapsed { "+" } else { "-" };
                push_sidebar_line(format!("{marker}[{fold}] Models"), selected == Some(entry));
            }
            SidebarItem::ModelGroupHeader(bucket) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                let (label, fold, count) = match bucket {
                    ModelBucket::Cloud => (
                        "Cloud",
                        if app.cloud_models_collapsed { "+" } else { "-" },
                        app.model_count_for_bucket(ModelBucket::Cloud),
                    ),
                    ModelBucket::Local => (
                        "Local",
                        if app.local_models_collapsed { "+" } else { "-" },
                        app.model_count_for_bucket(ModelBucket::Local),
                    ),
                };
                push_sidebar_line(
                    format!("{marker}   [{fold}] {label} ({count})"),
                    is_selected,
                );
            }
            SidebarItem::SectionHeader(SidebarSection::Workspaces) => {
                let marker = if selected == Some(entry) { ">" } else { " " };
                let fold = if app.workspaces_collapsed { "+" } else { "-" };
                push_sidebar_line(
                    format!("{marker}[{fold}] Workspaces ({})", app.workspaces.len()),
                    selected == Some(entry),
                );
            }
            SidebarItem::Model(index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(model) = app.models.get(index) {
                    let active = if model == &app.model { "*" } else { " " };
                    push_sidebar_line(format!("{marker}     {active} {model}"), is_selected);
                }
            }
            SidebarItem::WorkspaceCreate => {
                let marker = if selected == Some(entry) { ">" } else { " " };
                push_sidebar_line(format!("{marker} + New Workspace"), selected == Some(entry));
            }
            SidebarItem::Workspace(index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(workspace) = app.workspaces.get(index) {
                    let active = if workspace.name == app.workspace_name {
                        "*"
                    } else {
                        " "
                    };
                    let fold = if app.collapsed_workspace_ids.contains(&workspace.id) {
                        "+"
                    } else {
                        "-"
                    };
                    push_sidebar_line(
                        format!("{marker} {active} [{fold}] {}", workspace.name),
                        is_selected,
                    );
                }
            }
            SidebarItem::WorkspaceSessionsLabel(workspace_index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(workspace) = app.workspaces.get(workspace_index) {
                    push_sidebar_line(
                        format!("{marker}   Sessions ({})", workspace.sessions.len()),
                        is_selected,
                    );
                }
            }
            SidebarItem::WorkspaceSession(workspace_index, session_index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(session) = app
                    .workspaces
                    .get(workspace_index)
                    .and_then(|workspace| workspace.sessions.get(session_index))
                {
                    let active = if session.id == app.active_session_id {
                        "*"
                    } else {
                        " "
                    };
                    push_sidebar_line(
                        format!("{marker}     {active} Session: {}", session.title),
                        is_selected,
                    );
                }
            }
        }
    }

    let border_style = if app.focus == FocusPane::Workspaces {
        Style::default()
            .fg(palette.border_active)
            .bg(sidebar_background(app.theme))
    } else {
        Style::default()
            .fg(palette.border_inactive)
            .bg(sidebar_background(app.theme))
    };

    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .style(
                Style::default()
                    .fg(palette.sidebar_text)
                    .bg(sidebar_background(app.theme)),
            )
            .block(
                Block::default()
                    .title("Sidebar")
                    .borders(Borders::ALL)
                    .style(border_style),
            ),
        area,
    );
}

/// Get the action hint for the current sidebar selection.
#[allow(dead_code)]
pub(super) fn sidebar_action_hint(app: &App) -> String {
    match app.sidebar_item() {
        Some(SidebarItem::SectionHeader(SidebarSection::Models)) => String::from(
            "Enter toggle all models | /models refresh | /models add <name-or-url> | /themes",
        ),
        Some(SidebarItem::ModelGroupHeader(ModelBucket::Cloud)) => {
            String::from("Enter collapse/expand cloud models")
        }
        Some(SidebarItem::ModelGroupHeader(ModelBucket::Local)) => {
            String::from("Enter collapse/expand local models")
        }
        Some(SidebarItem::Model(_)) => String::from("Enter switch model | /models remove <name>"),
        Some(SidebarItem::SectionHeader(SidebarSection::Workspaces)) => {
            String::from("Enter toggle | /workspace create --path <folder>")
        }
        Some(SidebarItem::WorkspaceCreate) => {
            String::from("Enter starts workspace creation command")
        }
        Some(SidebarItem::Workspace(_)) => String::from(
            "Enter switch/toggle sessions | /workspace rename <new name> | /workspace delete",
        ),
        Some(SidebarItem::WorkspaceSessionsLabel(_)) => {
            String::from("Enter collapse/expand sessions | /session new <title>")
        }
        Some(SidebarItem::WorkspaceSession(_, _)) => {
            String::from("Enter open session | /session rename <id> <title> | /session delete")
        }
        None => String::from("Tab focus | Enter select"),
    }
}

/// Render a sidebar line with marquee scrolling effect for selected items.
pub(super) fn sidebar_line_with_marquee(
    line: &str,
    is_selected: bool,
    max_width: usize,
    frame: usize,
) -> String {
    if max_width == 0 {
        return String::new();
    }

    let chars = line.chars().collect::<Vec<_>>();
    if chars.len() <= max_width {
        return line.to_string();
    }

    let static_prefix_len = chars.len().min(4);
    let prefix = chars.iter().take(static_prefix_len).collect::<String>();
    let dynamic = chars.iter().skip(static_prefix_len).collect::<String>();
    let dynamic_width = max_width.saturating_sub(static_prefix_len);
    if dynamic_width == 0 {
        return truncate_end(line, max_width);
    }

    let dynamic_chars = dynamic.chars().collect::<Vec<_>>();
    if dynamic_chars.len() <= dynamic_width {
        return truncate_end(line, max_width);
    }

    if !is_selected {
        let tail = dynamic_chars
            .iter()
            .take(dynamic_width.saturating_sub(3))
            .collect::<String>();
        return format!("{prefix}{tail}...");
    }

    let separator = "   ";
    let mut marquee = dynamic.clone();
    marquee.push_str(separator);
    marquee.push_str(&dynamic);
    let window = scroll_window(
        &marquee.chars().collect::<Vec<_>>(),
        dynamic_width,
        (frame / 2) % (dynamic_chars.len() + separator.chars().count()),
    );
    format!("{prefix}{window}")
}
