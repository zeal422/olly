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
use super::{line_width, split_line_at_selection};

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
        let mut welcome_lines = super::preloader::animated_logo_lines_welcome();
        welcome_lines.extend([
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
        ]);
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
        app.chat_visual_lines_styled.clear();
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
        app.chat_visual_lines_styled.clear();
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

    // Check if cache is valid (version matches, not empty, same dimensions)
    let cache_valid = !app.chat_visual_lines_styled.is_empty()
        && app.chat_content_version == app.chat_cached_version
        && app.chat_last_render_width == split[0].width
        && app.chat_last_render_height == split[0].height;

    let max_scroll = if cache_valid {
        // Recompute max_scroll for current viewport height
        let max_scroll = app
            .chat_visual_lines_styled
            .len()
            .saturating_sub(viewport_height as usize) as u16;
        app.chat_max_scroll_hint.set(max_scroll);
        max_scroll
    } else {
        // Rebuild from scratch
        let logical_lines = build_chat_logical_lines(app, palette);
        let visual_lines = super::manual_reflow(&logical_lines, chat_width);
        let styled_lines: Vec<Line<'static>> = visual_lines
            .into_iter()
            .map(|line| {
                let mut l = Line::from(
                    line.spans
                        .into_iter()
                        .map(|s| Span::styled(s.content.into_owned(), s.style))
                        .collect::<Vec<_>>(),
                );
                if let Some(align) = line.alignment {
                    l = l.alignment(align);
                }
                l
            })
            .collect();
        let plain_lines: Vec<(String, Option<Alignment>)> = styled_lines
            .iter()
            .map(|line| (line_to_string(line), line.alignment))
            .collect();
        let max_scroll = styled_lines.len().saturating_sub(viewport_height as usize) as u16;

        // Update cache
        app.chat_visual_lines_styled = styled_lines;
        app.chat_cached_version = app.chat_content_version;
        app.chat_last_render_width = split[0].width;
        app.chat_last_render_height = split[0].height;
        app.chat_area = Some(split[0]);
        app.chat_visual_lines = plain_lines;
        app.chat_max_scroll_hint.set(max_scroll);

        max_scroll
    };

    let scroll_y = super::calculate_scroll_position(app.chat_scroll, max_scroll);

    // Only process visible viewport lines (O(viewport) instead of O(n))
    let total_lines = app.chat_visual_lines_styled.len();
    let visible_start = scroll_y as usize;
    let visible_end = (scroll_y as usize + viewport_height as usize).min(total_lines);

    let display_lines: Vec<Line<'static>> = if visible_start < total_lines {
        let visible_lines = &app.chat_visual_lines_styled[visible_start..visible_end];

        if let (Some(anchor), Some(current)) =
            (app.chat_selection_anchor, app.chat_selection_current)
        {
            // Apply selection only to visible lines
            let (mut start_line, mut start_col) = anchor;
            let (mut end_line, mut end_col) = current;
            if start_line > end_line || (start_line == end_line && start_col > end_col) {
                std::mem::swap(&mut start_line, &mut end_line);
                std::mem::swap(&mut start_col, &mut end_col);
            }

            let selection_style = Style::default().add_modifier(Modifier::REVERSED);
            visible_lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let global_idx = visible_start + i;
                    if global_idx < start_line || global_idx > end_line {
                        // Not selected — clone as-is
                        Line {
                            spans: line
                                .spans
                                .iter()
                                .map(|s| Span {
                                    content: std::borrow::Cow::Owned(s.content.to_string()),
                                    style: s.style,
                                })
                                .collect(),
                            alignment: line.alignment,
                            style: Style::default(),
                        }
                    } else {
                        // Within selection range
                        let line_len = line_width(line);
                        let sel_start = if global_idx == start_line {
                            start_col
                        } else {
                            0
                        };
                        let sel_end_excl = if global_idx == end_line {
                            end_col.saturating_add(1).min(line_len)
                        } else {
                            line_len
                        };

                        if global_idx == start_line && global_idx == end_line {
                            let new_spans = split_line_at_selection(
                                line,
                                sel_start,
                                sel_end_excl,
                                selection_style,
                            );
                            Line {
                                spans: new_spans,
                                alignment: line.alignment,
                                style: Style::default(),
                            }
                        } else if global_idx == start_line {
                            let new_spans =
                                split_line_at_selection(line, sel_start, line_len, selection_style);
                            Line {
                                spans: new_spans,
                                alignment: line.alignment,
                                style: Style::default(),
                            }
                        } else if global_idx == end_line {
                            let new_spans =
                                split_line_at_selection(line, 0, sel_end_excl, selection_style);
                            Line {
                                spans: new_spans,
                                alignment: line.alignment,
                                style: Style::default(),
                            }
                        } else {
                            // Fully selected line in the middle
                            Line {
                                spans: line
                                    .spans
                                    .iter()
                                    .map(|s| Span {
                                        content: std::borrow::Cow::Owned(s.content.to_string()),
                                        style: s.style.patch(selection_style),
                                    })
                                    .collect(),
                                alignment: line.alignment,
                                style: Style::default(),
                            }
                        }
                    }
                })
                .collect()
        } else {
            // No selection — just clone visible lines
            visible_lines
                .iter()
                .map(|line| Line {
                    spans: line
                        .spans
                        .iter()
                        .map(|s| Span {
                            content: std::borrow::Cow::Owned(s.content.to_string()),
                            style: s.style,
                        })
                        .collect(),
                    alignment: line.alignment,
                    style: Style::default(),
                })
                .collect()
        }
    } else {
        Vec::new()
    };

    frame.render_widget(
        Paragraph::new(Text::from(display_lines)).style(
            Style::default()
                .fg(palette.chat_text)
                .bg(chat_background(app.theme)),
        ),
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
