//! Chat panel rendering - main chat view, footer context, and message display.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{
    state::{FocusPane, Message},
    App,
};

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
        // Rebuild from scratch — track code blocks during this pass
        let (logical_lines, msg_logical_counts) =
            build_chat_logical_lines_with_counts(app, palette);
        let visual_lines = super::manual_reflow(&logical_lines, chat_width);

        // Parse code blocks from raw content (logical level)
        let blocks_logical = parse_code_blocks_logical(&app.messages);

        // Map logical block indices to post-reflow visual ranges
        app.code_blocks = map_code_blocks_to_visual(
            &logical_lines,
            &msg_logical_counts,
            &blocks_logical,
            chat_width,
        );

        let mut styled_lines: Vec<Line<'static>> = visual_lines
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

        // Add [copy] indicators to code block first lines
        for block in &app.code_blocks {
            if block.start_visual_line < styled_lines.len() {
                let line = &mut styled_lines[block.start_visual_line];
                line.spans.push(Span::styled(
                    " [copy]",
                    Style::default()
                        .fg(palette.body_muted)
                        .add_modifier(Modifier::DIM),
                ));
            }
        }

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
            // No selection — Just clone visible lines
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
#[allow(dead_code)]
fn build_chat_logical_lines(app: &App, palette: super::ThemePalette) -> Vec<Line<'static>> {
    build_chat_logical_lines_with_counts(app, palette).0
}

/// Build logical lines from messages for display.
/// Returns (logical_lines, msg_logical_counts) where msg_logical_counts[i] is the
/// number of logical lines produced by message i.
pub(crate) fn build_chat_logical_lines_with_counts(
    app: &App,
    palette: super::ThemePalette,
) -> (Vec<Line<'static>>, Vec<usize>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut msg_line_counts: Vec<usize> = Vec::new();

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

        let mut count = 0;

        // 1. Header line (with optional tool_calls_preview — matches production exactly)
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
        count += 1;

        // 2. Optional image badge line
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
            count += 1;
        }

        // 3. Markdown content lines
        let message_lines = super::markdown_lines(
            &message.content,
            base_style,
            app.syntax_highlighting,
            palette,
        );
        for spans in message_lines {
            lines.push(Line::from(spans).alignment(align));
            count += 1;
        }

        // 4. Empty separator line
        lines.push(Line::default());
        count += 1;

        msg_line_counts.push(count);
    }

    (lines, msg_line_counts)
}

/// Convert a Line to a plain string.
fn line_to_string(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

// ─── Code block detection and mapping ───────────────────────────────────────

/// A code block identified from raw message content, expressed in logical line indices.
#[derive(Debug, Clone)]
pub struct CodeBlockLogical {
    /// Index within the message's logical lines where the block starts (0-based).
    pub logical_start_in_msg: usize,
    /// Number of code content lines (before reflow; fences excluded).
    pub code_line_count: usize,
    pub raw_content: String,
    pub language: String,
}

/// Parse code blocks from all messages, returning logical-level block info.
pub(crate) fn parse_code_blocks_logical(messages: &[Message]) -> Vec<(usize, CodeBlockLogical)> {
    let mut result = Vec::new();

    for (msg_idx, msg) in messages.iter().enumerate() {
        // Count logical lines before content (header + optional image badge)
        let pre_content_lines = 1 + if msg.image_count > 0 { 1 } else { 0 };

        let mut in_code_block = false;
        let mut code_start_logical = 0;
        let mut code_lines = Vec::new();
        let mut code_lang = String::new();
        let mut logical_line_idx = 0;

        for raw_line in msg.content.lines() {
            let trimmed = raw_line.trim();

            if trimmed.starts_with("```") {
                if in_code_block {
                    if !code_lines.is_empty() {
                        result.push((
                            msg_idx,
                            CodeBlockLogical {
                                logical_start_in_msg: pre_content_lines + code_start_logical,
                                code_line_count: code_lines.len(),
                                raw_content: code_lines.join("\n"),
                                language: code_lang.clone(),
                            },
                        ));
                    }
                    in_code_block = false;
                    code_lines.clear();
                } else {
                    in_code_block = true;
                    code_start_logical = logical_line_idx;
                    let fence_len = trimmed.chars().take_while(|&c| c == '`').count();
                    code_lang = trimmed[fence_len..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_string();
                }
            } else if in_code_block {
                code_lines.push(raw_line.to_string());
            }

            // Count non-fence lines (markdown_lines skips fences)
            if !trimmed.starts_with("```") {
                logical_line_idx += 1;
            }
        }

        // Flush unclosed block
        if in_code_block && !code_lines.is_empty() {
            result.push((
                msg_idx,
                CodeBlockLogical {
                    logical_start_in_msg: pre_content_lines + code_start_logical,
                    code_line_count: code_lines.len(),
                    raw_content: code_lines.join("\n"),
                    language: code_lang.clone(),
                },
            ));
        }
    }

    result
}

/// Convert logical-level code blocks to post-reflow visual line ranges.
fn map_code_blocks_to_visual(
    logical_lines: &[Line<'_>],
    msg_logical_counts: &[usize],
    blocks_logical: &[(usize, CodeBlockLogical)],
    chat_width: usize,
) -> Vec<crate::app::state::CodeBlockRange> {
    // Build mapping: for each logical line index, what is the first visual line index it produced?
    let mut logical_to_visual_start = vec![0usize; logical_lines.len().saturating_add(1)];

    let mut visual_idx = 0;
    for (_li, logical) in logical_lines.iter().enumerate() {
        logical_to_visual_start[_li] = visual_idx;
        visual_idx += super::reflow_row_count_for_line(logical, chat_width);
    }
    logical_to_visual_start[logical_lines.len()] = visual_idx;

    // Convert each logical block to visual range
    blocks_logical
        .iter()
        .filter(|(_, block)| block.code_line_count > 0)
        .filter_map(|(msg_idx, block)| {
            let global_offset = compute_global_logical_offset(*msg_idx, msg_logical_counts);
            let start_logical = global_offset + block.logical_start_in_msg;
            let end_logical = (start_logical + block.code_line_count).min(logical_lines.len());

            if start_logical >= logical_to_visual_start.len() {
                return None;
            }

            let visual_start = logical_to_visual_start[start_logical];
            let visual_end_exclusive = logical_to_visual_start[end_logical];

            if visual_end_exclusive <= visual_start {
                return None;
            }

            Some(crate::app::state::CodeBlockRange {
                start_visual_line: visual_start,
                end_visual_line: visual_end_exclusive.saturating_sub(1),
                raw_content: block.raw_content.clone(),
                language: block.language.clone(),
            })
        })
        .collect()
}

fn compute_global_logical_offset(msg_idx: usize, msg_logical_counts: &[usize]) -> usize {
    msg_logical_counts.iter().take(msg_idx).sum()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_blocks_logical_detects_triple_backtick_block() {
        let msg = Message {
            role: "Assistant".into(),
            content: "Here is code:\n```rust\nfn main() {}\n```\nDone.".into(),
            timestamp: "12:00".into(),
            image_count: 0,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        };
        let blocks = parse_code_blocks_logical(&[msg]);
        assert_eq!(blocks.len(), 1);
        let (_, block) = &blocks[0];
        assert_eq!(block.language, "rust");
        assert_eq!(block.raw_content, "fn main() {}");
    }

    #[test]
    fn parse_code_blocks_logical_handles_quadruple_backticks_as_content() {
        let msg = Message {
            role: "Assistant".into(),
            content: "````\nsome code\n````".into(),
            timestamp: "12:00".into(),
            image_count: 0,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        };
        let blocks = parse_code_blocks_logical(&[msg]);
        assert_eq!(blocks.len(), 1);
        let (_, block) = &blocks[0];
        // Fixed behavior: strips all leading backticks, so language is empty.
        assert!(block.language.is_empty());
        assert_eq!(block.raw_content, "some code");
    }

    #[test]
    fn map_code_blocks_to_visual_handles_wrapping() {
        let long_line: String = "a".repeat(50);
        let logical_lines: Vec<Line<'static>> = vec![
            Line::from("header"),
            Line::from(long_line),
            Line::from("fn main() {}"),
            Line::from("let x = 1;"),
            Line::default(),
        ];
        let msg_logical_counts = vec![logical_lines.len()];
        let blocks_logical = vec![(
            0usize,
            CodeBlockLogical {
                logical_start_in_msg: 2,
                code_line_count: 2,
                raw_content: "fn main() {}\nlet x = 1;".into(),
                language: "rust".into(),
            },
        )];
        let chat_width = 10;

        let result = map_code_blocks_to_visual(
            &logical_lines,
            &msg_logical_counts,
            &blocks_logical,
            chat_width,
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_visual_line, 6);
        assert_eq!(result[0].end_visual_line, 8); // "fn main() {}" wraps at width 10
        assert_eq!(result[0].language, "rust");
    }

    #[test]
    fn map_code_blocks_to_visual_filters_empty_blocks() {
        let logical_lines: Vec<Line<'static>> = vec![Line::from("header"), Line::from("code")];
        let msg_logical_counts = vec![2];
        let blocks_logical = vec![(
            0usize,
            CodeBlockLogical {
                logical_start_in_msg: 1,
                code_line_count: 0,
                raw_content: String::new(),
                language: "rust".into(),
            },
        )];

        let result =
            map_code_blocks_to_visual(&logical_lines, &msg_logical_counts, &blocks_logical, 80);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn map_code_blocks_to_visual_handles_multiple_messages() {
        let logical_lines: Vec<Line<'static>> = vec![
            Line::from("header0"),
            Line::from("code line"),
            Line::default(),
            Line::from("header1"),
            Line::from("code line"),
            Line::default(),
        ];
        let msg_logical_counts = vec![3, 3];
        let blocks_logical = vec![
            (
                0usize,
                CodeBlockLogical {
                    logical_start_in_msg: 1,
                    code_line_count: 1,
                    raw_content: "line1".into(),
                    language: "rust".into(),
                },
            ),
            (
                1usize,
                CodeBlockLogical {
                    logical_start_in_msg: 1,
                    code_line_count: 1,
                    raw_content: "line2".into(),
                    language: "python".into(),
                },
            ),
        ];

        let result =
            map_code_blocks_to_visual(&logical_lines, &msg_logical_counts, &blocks_logical, 80);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].start_visual_line, 1);
        assert_eq!(result[1].start_visual_line, 4);
    }

    #[test]
    fn build_chat_logical_lines_with_counts_single_message_no_image() {
        use crate::app::UiTheme;

        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.messages.push(Message {
            role: "You".into(),
            content: "line1\nline2".into(),
            timestamp: String::from("12:00"),
            image_count: 0,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        });

        let palette = super::super::palette::theme_palette(UiTheme::EmeraldDusk);
        let (lines, counts) = build_chat_logical_lines_with_counts(&app, palette);

        assert_eq!(counts, vec![4]);
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn build_chat_logical_lines_with_counts_message_with_image() {
        use crate::app::UiTheme;

        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.messages.push(Message {
            role: "You".into(),
            content: "hello".into(),
            timestamp: String::from("12:00"),
            image_count: 2,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        });

        let palette = super::super::palette::theme_palette(UiTheme::EmeraldDusk);
        let (lines, counts) = build_chat_logical_lines_with_counts(&app, palette);

        assert_eq!(counts, vec![4]);
        assert_eq!(lines.len(), 4);
        assert!(lines[1].spans.iter().any(|s| s.content.contains("images")));
    }

    #[test]
    fn copy_indicator_appended_to_first_line_of_code_block() {
        let mut styled_lines: Vec<Line<'static>> =
            (0..10).map(|i| Line::from(format!("line {i}"))).collect();
        let code_blocks = vec![crate::app::state::CodeBlockRange {
            start_visual_line: 3,
            end_visual_line: 5,
            raw_content: "fn main() {}".into(),
            language: "rust".into(),
        }];

        for block in &code_blocks {
            if block.start_visual_line < styled_lines.len() {
                let line = &mut styled_lines[block.start_visual_line];
                line.spans.push(Span::styled(
                    " [copy]",
                    Style::default()
                        .fg(ratatui::style::Color::Gray)
                        .add_modifier(Modifier::DIM),
                ));
            }
        }

        let line3_text: String = styled_lines[3]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(line3_text.contains(" [copy]"));

        let line0_text: String = styled_lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(!line0_text.contains(" [copy]"));
    }

    #[test]
    fn parse_code_blocks_logical_flushes_unclosed_block() {
        let messages = vec![Message {
            role: "Assistant".into(),
            content: "```rust\nfn main() {}\n".into(),
            timestamp: "12:00".into(),
            image_count: 0,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        }];

        let result = parse_code_blocks_logical(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1.language, "rust");
        assert_eq!(result[0].1.raw_content, "fn main() {}");
        assert_eq!(result[0].1.code_line_count, 1);
    }

    #[test]
    fn parse_code_blocks_logical_ignores_empty_block() {
        let messages = vec![Message {
            role: "Assistant".into(),
            content: "```\n```".into(),
            timestamp: "12:00".into(),
            image_count: 0,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        }];

        let result = parse_code_blocks_logical(&messages);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn parse_code_blocks_logical_language_extraction_edges() {
        let messages = vec![Message {
            role: "Assistant".into(),
            content:
                "```  python\npass\n```\n```\ncode\n```\n```typescript-react\nconst x = 1;\n```"
                    .into(),
            timestamp: "12:00".into(),
            image_count: 0,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        }];

        let result = parse_code_blocks_logical(&messages);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].1.language, "python");
        assert_eq!(result[1].1.language, "");
        assert_eq!(result[2].1.language, "typescript-react");
    }

    #[test]
    fn diff_with_nested_fences_does_not_break_parsing() {
        let diff_output = "````\nSome code with ```rust inside\n````";

        let messages = vec![Message {
            role: "System".into(),
            content: diff_output.into(),
            timestamp: "12:00".into(),
            image_count: 0,
            tool_calls_preview: None,
            tool_name: None,
            tool_calls: None,
        }];

        let result = parse_code_blocks_logical(&messages);

        assert_eq!(result.len(), 1);
        assert!(result[0].1.raw_content.contains("```rust"));
    }
}
