//! Chat panel rendering - main chat view, footer context, and message display.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{state::FocusPane, App};

use super::palette::chat_background;

/// Render the main chat panel with messages and footer context strip.
pub(super) fn render_chat_panel(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: Rect,
    palette: super::ThemePalette,
) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let border_style = if app.focus == FocusPane::Chat {
        Style::default()
            .fg(palette.border_active)
            .bg(chat_background(app.theme))
    } else {
        Style::default()
            .fg(palette.border_inactive)
            .bg(chat_background(app.theme))
    };
    let block = Block::default()
        .title("Chat")
        .borders(Borders::ALL)
        .style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 2 || inner.height < 2 {
        return;
    }

    if app.active_workspace_id <= 0 {
        let welcome_lines = vec![
            Line::from(Span::styled(
                "======================================================",
                Style::default()
                    .fg(palette.border_active)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "        OLLY",
                Style::default()
                    .fg(palette.border_active)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "======================================================",
                Style::default()
                    .fg(palette.border_active)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Terminal workspace chat for local/cloud Ollama models."),
            Line::from(""),
            Line::from(Span::styled(
                "Requirements:",
                Style::default()
                    .fg(palette.chat_text)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("- Install Ollama"),
            Line::from("- Start Ollama server (`ollama serve`)"),
            Line::from("- If needed, set URL with `/url <http://host:port>`"),
            Line::from(""),
            Line::from(Span::styled(
                "Start here:",
                Style::default()
                    .fg(palette.chat_text)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("1. Press Tab to focus sidebar"),
            Line::from("2. Select `+ New Workspace` and press Enter"),
            Line::from("3. Add a folder path and press Enter"),
            Line::from("4. Type a prompt and send with Ctrl+Enter"),
            Line::from(""),
            Line::from("Tip: You can also run `/workspace create New Workspace --path <folder>`."),
        ];
        let welcome_height = welcome_lines.len() as u16;
        let top_padding = inner
            .height
            .saturating_sub(welcome_height)
            .saturating_div(2);
        let welcome_area = Rect {
            x: inner.x,
            y: inner.y.saturating_add(top_padding),
            width: inner.width,
            height: inner.height.saturating_sub(top_padding),
        };
        frame.render_widget(
            Paragraph::new(Text::from(welcome_lines))
                .style(Style::default().fg(palette.body_muted))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            welcome_area,
        );
        // Clear stale chat metadata when not in chat view
        app.chat_area = None;
        app.chat_visual_lines.clear();
        app.clear_chat_selection();
        app.chat_scroll = 0;
        app.chat_max_scroll_hint.set(0);
        return;
    }

    if app.active_session_id <= 0 {
        let empty_session = [
            "No Session In This Workspace",
            "",
            "Create one to start chatting:",
            "- /session new <title>",
            "- /session create <title>",
            "- /session switch <id|title>",
        ]
        .join("\n");
        frame.render_widget(
            Paragraph::new(empty_session)
                .style(Style::default().fg(palette.body_muted))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            inner,
        );
        // Clear stale chat metadata when not in chat view
        app.chat_area = None;
        app.chat_visual_lines.clear();
        app.clear_chat_selection();
        app.chat_scroll = 0;
        app.chat_max_scroll_hint.set(0);
        return;
    }

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let viewport_height = split[0].height;
    let chat_width = split[0].width as usize;
    let logical_lines = build_chat_logical_lines(app, palette);
    let visual_lines = super::manual_reflow(&logical_lines, chat_width);
    let plain_lines: Vec<(String, Option<Alignment>)> = visual_lines
        .iter()
        .map(|line| (line_to_string(line), line.alignment))
        .collect();
    let max_scroll = visual_lines.len().saturating_sub(viewport_height as usize) as u16;
    app.update_chat_area_metadata(split[0], plain_lines, max_scroll);

    let scroll_y = super::calculate_scroll_position(app.chat_scroll, max_scroll);

    let display_lines = if let (Some(anchor), Some(current)) =
        (app.chat_selection_anchor, app.chat_selection_current)
    {
        super::apply_selection_to_text(&visual_lines, anchor, current)
    } else {
        visual_lines
    };

    frame.render_widget(
        Paragraph::new(Text::from(display_lines))
            .style(
                Style::default()
                    .fg(palette.chat_text)
                    .bg(chat_background(app.theme)),
            )
            .scroll((scroll_y, 0)),
        split[0],
    );

    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(split[1]);

    let action_hint = super::sidebar_action_hint(app);
    frame.render_widget(
        Paragraph::new(super::truncate_end(
            &action_hint,
            footer[0].width.saturating_sub(1) as usize,
        ))
        .style(
            Style::default()
                .fg(palette.footer_hint)
                .bg(chat_background(app.theme)),
        )
        .alignment(Alignment::Left),
        footer[0],
    );

    let path = app
        .workspace_root_path
        .as_deref()
        .unwrap_or("(path not set)");
    let context_usage = format!(
        "ctx:{}/{}",
        token_label(app.prompt_tokens_used),
        token_label(app.model_context_window)
    );
    let context_line = format!(
        "session:{}#{} | msgs:{} | {} | ws:{} | path:{}",
        app.active_session_title,
        app.active_session_id,
        app.messages.len(),
        context_usage,
        app.workspace_name,
        super::truncate_middle(path, 28)
    );
    frame.render_widget(
        Paragraph::new(super::truncate_middle(
            &context_line,
            footer[1].width.saturating_sub(1) as usize,
        ))
        .style(
            Style::default()
                .fg(palette.footer_context)
                .bg(chat_background(app.theme)),
        )
        .alignment(Alignment::Right),
        footer[1],
    );
}

/// Build logical lines from messages for display.
fn build_chat_logical_lines(app: &App, palette: super::ThemePalette) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for message in &app.messages {
        let (avatar, align, base_style, header_style) = match message.role.as_str() {
            "You" => (
                "YOU",
                Alignment::Right,
                Style::default().fg(palette.you_fg).bg(palette.you_bg),
                Style::default()
                    .fg(palette.you_header)
                    .add_modifier(Modifier::BOLD),
            ),
            "Assistant" => (
                "AI",
                Alignment::Left,
                Style::default()
                    .fg(palette.assistant_fg)
                    .bg(palette.assistant_bg),
                Style::default()
                    .fg(palette.assistant_header)
                    .add_modifier(Modifier::BOLD),
            ),
            "Tool" => (
                "TOOL",
                Alignment::Left,
                Style::default().fg(palette.tool_fg).bg(palette.tool_bg),
                Style::default()
                    .fg(palette.tool_header)
                    .add_modifier(Modifier::BOLD),
            ),
            _ => (
                "SYS",
                Alignment::Center,
                Style::default().fg(palette.system_fg).bg(palette.system_bg),
                Style::default()
                    .fg(palette.system_header)
                    .add_modifier(Modifier::ITALIC),
            ),
        };

        lines.push(
            Line::from(vec![Span::styled(
                match &message.tool_calls_preview {
                    Some(preview) => format!("{avatar} {} | {preview}", message.timestamp),
                    None => format!("{avatar} {}", message.timestamp),
                },
                header_style,
            )])
            .alignment(align),
        );

        if message.image_count > 0 {
            lines.push(
                Line::from(vec![Span::styled(
                    format!(" [attached images: {}] ", message.image_count),
                    base_style
                        .fg(palette.image_badge_fg)
                        .bg(palette.image_badge_bg)
                        .add_modifier(Modifier::BOLD),
                )])
                .alignment(align),
            );
        }

        let message_lines = super::markdown_lines(
            &message.content,
            base_style,
            app.syntax_highlighting,
            palette,
        );
        for spans in message_lines {
            lines.push(Line::from(spans).alignment(align));
        }
        lines.push(Line::default());
    }

    lines
}

/// Convert a Line to a plain string.
fn line_to_string(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

/// Format token count with k/m suffixes.
fn format_tokens(value: u32) -> String {
    if value >= 1_000_000 {
        format!("{:.1}m", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

/// Format token count with k/m suffixes.
fn token_label(value: Option<u32>) -> String {
    value.map(format_tokens).unwrap_or_else(|| "?".to_string())
}
