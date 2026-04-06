use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{
    domain::{ModelBucket, SidebarItem, SidebarSection},
    state::ToastLevel,
    App,
};

mod chat;
mod input;
mod markdown;
mod modals;
mod palette;
mod preloader;
mod sidebar;

// Re-export palette and markdown types/functions for use by submodules.
pub(crate) use markdown::markdown_lines;
pub(crate) use palette::ThemePalette;
pub(crate) use preloader::render_preloader;

/// Calculate scroll position from bottom.
fn calculate_scroll_position(chat_scroll: u16, max_scroll: u16) -> u16 {
    let from_bottom = chat_scroll.min(max_scroll);
    max_scroll.saturating_sub(from_bottom)
}

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    if frame.area().width < 2 || frame.area().height < 2 {
        return;
    }
    let palette = palette::theme_palette(app.theme);
    frame.render_widget(
        Block::default().style(Style::default().bg(palette::app_background(app.theme))),
        frame.area(),
    );

    let input_height = input::input_panel_height(app, frame.area());
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(input_height),
        ])
        .split(frame.area());

    sidebar::render_status_bar(frame, app, root[0], palette);

    let body = if frame.area().width < 115 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Min(8)])
            .split(root[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(20)])
            .split(root[1])
    };

    sidebar::render_models_panel(frame, app, body[0], palette);
    chat::render_chat_panel(frame, app, body[1], palette);
    input::render_input_panel(frame, app, root[2], palette);

    if let Some(toast) = &app.toast {
        let frame_width = frame.area().width;
        if frame_width < 4 {
            return;
        }
        let max_toast_width = frame_width.saturating_sub(2).max(1);
        let width = (toast.message.len().min(56) as u16 + 4).min(max_toast_width);
        let x = frame.area().width.saturating_sub(width.saturating_add(2));
        let area = Rect {
            x,
            y: 2,
            width,
            height: 3,
        };
        frame.render_widget(Clear, area);
        let style = match toast.level {
            ToastLevel::Info => Style::default()
                .bg(palette.toast_info_bg)
                .fg(palette.toast_fg),
            ToastLevel::Success => Style::default()
                .bg(palette.toast_success_bg)
                .fg(palette.toast_fg),
            ToastLevel::Error => Style::default()
                .bg(palette.toast_error_bg)
                .fg(palette.toast_fg),
        };
        frame.render_widget(
            Paragraph::new(toast.message.as_str()).style(style).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Toast")
                    .style(Style::default().fg(palette.border_active)),
            ),
            area,
        );
    }

    modals::render_help_modal(frame, app, palette);
    modals::render_workspace_delete_modal(frame, app, palette);
    modals::render_session_delete_modal(frame, app, palette);
    modals::render_theme_picker_modal(frame, app, palette);
    modals::render_tool_confirmation_modal(frame, app, palette);
}

pub(super) fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

fn manual_reflow<'a>(logical_lines: &[Line<'a>], width: usize) -> Vec<Line<'a>> {
    if width == 0 {
        return logical_lines.to_vec();
    }

    let mut result = Vec::new();

    for line in logical_lines {
        let lw = line_width(line);
        if lw <= width || line.spans.is_empty() {
            result.push(line.clone());
            continue;
        }

        let alignment = line.alignment;

        let mut remaining: Vec<Span<'a>> = line.spans.clone();
        while !remaining.is_empty() {
            let mut current_line_spans: Vec<Span<'a>> = Vec::new();
            let mut current_width = 0;
            let mut next_remaining: Vec<Span<'a>> = Vec::new();
            let mut broke_mid_span = false;

            for span in &remaining {
                let span_chars: Vec<char> = span.content.chars().collect();
                let span_len = span_chars.len();

                if current_width + span_len <= width {
                    current_line_spans.push(span.clone());
                    current_width += span_len;
                } else {
                    let space_left = width - current_width;
                    if space_left > 0 && current_line_spans.is_empty() {
                        let split_text: String = span_chars[..space_left].iter().collect();
                        let remainder_text: String = span_chars[space_left..].iter().collect();
                        current_line_spans.push(Span {
                            content: std::borrow::Cow::Owned(split_text),
                            style: span.style,
                        });
                        if !remainder_text.is_empty() {
                            next_remaining.push(Span {
                                content: std::borrow::Cow::Owned(remainder_text),
                                style: span.style,
                            });
                        }
                    } else if !current_line_spans.is_empty() {
                        next_remaining.push(span.clone());
                    } else {
                        let split_text: String = span_chars[..width].iter().collect();
                        let remainder_text: String = span_chars[width..].iter().collect();
                        current_line_spans.push(Span {
                            content: std::borrow::Cow::Owned(split_text),
                            style: span.style,
                        });
                        if !remainder_text.is_empty() {
                            next_remaining.push(Span {
                                content: std::borrow::Cow::Owned(remainder_text),
                                style: span.style,
                            });
                        }
                    }
                    broke_mid_span = true;
                    break;
                }
            }

            if !broke_mid_span && current_line_spans.is_empty() && !remaining.is_empty() {
                let span = &remaining[0];
                let span_chars: Vec<char> = span.content.chars().collect();
                let split_text: String = span_chars[..width.min(span_chars.len())].iter().collect();
                let remainder_text: String =
                    span_chars[width.min(span_chars.len())..].iter().collect();
                current_line_spans.push(Span {
                    content: std::borrow::Cow::Owned(split_text),
                    style: span.style,
                });
                if !remainder_text.is_empty() {
                    next_remaining.push(Span {
                        content: std::borrow::Cow::Owned(remainder_text),
                        style: span.style,
                    });
                }
            }

            result.push(Line {
                spans: current_line_spans,
                alignment,
                style: line.style,
            });
            remaining = next_remaining;
        }
    }

    result
}

/// Visual rows count for a single logical line at `width`.
/// Uses `manual_reflow` on a single-line slice so the result matches production exactly.
pub(super) fn reflow_row_count_for_line(line: &Line<'_>, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    manual_reflow(std::slice::from_ref(line), width).len()
}

pub(super) fn split_line_at_selection(
    line: &Line<'_>,
    sel_start: usize,
    sel_end: usize,
    selection_style: Style,
) -> Vec<Span<'static>> {
    if sel_start >= sel_end {
        return line
            .spans
            .iter()
            .map(|s| Span {
                content: std::borrow::Cow::Owned(s.content.to_string()),
                style: s.style,
            })
            .collect();
    }

    let mut result = Vec::new();
    let mut char_offset = 0;

    for span in &line.spans {
        let span_text: String = span.content.to_string();
        let span_len = span_text.chars().count();
        let span_start = char_offset;
        let span_end = char_offset + span_len;

        if span_end <= sel_start || span_start >= sel_end {
            result.push(Span {
                content: std::borrow::Cow::Owned(span_text),
                style: span.style,
            });
        } else {
            let chars: Vec<char> = span_text.chars().collect();
            let overlap_start = sel_start.saturating_sub(span_start);
            let overlap_end = sel_end.min(span_end) - span_start;

            if overlap_start > 0 {
                let before: String = chars[..overlap_start].iter().collect();
                result.push(Span {
                    content: std::borrow::Cow::Owned(before),
                    style: span.style,
                });
            }

            let selected: String = chars[overlap_start..overlap_end].iter().collect();
            result.push(Span {
                content: std::borrow::Cow::Owned(selected),
                style: span.style.patch(selection_style),
            });

            if overlap_end < span_len {
                let after: String = chars[overlap_end..].iter().collect();
                result.push(Span {
                    content: std::borrow::Cow::Owned(after),
                    style: span.style,
                });
            }
        }

        char_offset += span_len;
    }

    result
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    if max_chars < 5 {
        return value.chars().take(max_chars).collect();
    }

    let head_count = (max_chars - 3) / 2;
    let tail_count = max_chars - head_count - 3;
    let head = value.chars().take(head_count).collect::<String>();
    let tail = value
        .chars()
        .rev()
        .take(tail_count)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{head}...{tail}")
}

fn truncate_end(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    chars.into_iter().take(max_chars - 3).collect::<String>() + "..."
}

#[allow(dead_code)]
fn sidebar_line_with_marquee(
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

#[allow(dead_code)]
fn scroll_window(chars: &[char], width: usize, offset: usize) -> String {
    if chars.is_empty() || width == 0 {
        return String::new();
    }
    (0..width)
        .map(|idx| chars[(offset + idx) % chars.len()])
        .collect::<String>()
}

fn sidebar_action_hint(app: &App) -> String {
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

#[cfg(test)]
mod tests {
    use super::reflow_row_count_for_line;
    use super::{
        calculate_scroll_position, manual_reflow, scroll_window, sidebar_action_hint,
        sidebar_line_with_marquee, split_line_at_selection, truncate_end, truncate_middle, Style,
    };
    use crate::app::tui::input::{
        estimate_input_visual_lines, input_cursor_screen_position, input_cursor_visual_position,
        input_panel_height,
    };
    use crate::app::{
        domain::{ModelBucket, SidebarItem},
        state::{SessionEntry, WorkspaceEntry},
        App,
    };
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use ratatui::text::Line;

    fn app_for_sidebar_tests() -> App {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("qwen2.5-coder:7b"),
        );
        app.models = vec![
            String::from("qwen2.5-coder:7b"),
            String::from("gemini-3-flash-preview:cloud"),
        ];
        app.workspaces = vec![WorkspaceEntry {
            id: 1,
            name: String::from("Workspace A"),
            root_path: Some(String::from("C:/work/a")),
            sessions: vec![SessionEntry {
                id: 10,
                title: String::from("Main Conversation"),
            }],
        }];
        app
    }

    #[test]
    fn truncate_middle_preserves_expected_shape() {
        let value = "abcdefghijklmnopqrstuvwxyz";
        let truncated = truncate_middle(value, 12);
        assert_eq!(truncated.chars().count(), 12);
        assert!(truncated.starts_with("abcd"));
        assert!(truncated.ends_with("wxyz"));
        assert!(truncated.contains("..."));
    }

    #[test]
    fn truncate_end_preserves_expected_shape() {
        assert_eq!(truncate_end("abcdef", 0), "");
        assert_eq!(truncate_end("abcdef", 3), "...");
        assert_eq!(truncate_end("abcdefghij", 8), "abcde...");
    }

    #[test]
    fn scroll_window_rotates_content_by_offset() {
        let chars = "abcdef".chars().collect::<Vec<_>>();
        assert_eq!(scroll_window(&chars, 4, 0), "abcd");
        assert_eq!(scroll_window(&chars, 4, 2), "cdef");
        assert_eq!(scroll_window(&chars, 4, 5), "fabc");
    }

    #[test]
    fn sidebar_marquee_truncates_unselected_and_scrolls_selected() {
        let line = "> Workspace: VeryLongWorkspaceNameForSmallScreens";
        let unselected = sidebar_line_with_marquee(line, false, 16, 0);
        let selected_a = sidebar_line_with_marquee(line, true, 16, 0);
        let selected_b = sidebar_line_with_marquee(line, true, 16, 40);

        assert_eq!(unselected.chars().count(), 16);
        assert!(unselected.ends_with("..."));
        assert_eq!(selected_a.chars().count(), 16);
        assert_eq!(selected_b.chars().count(), 16);
        assert_ne!(selected_a, selected_b);
    }

    #[test]
    fn sidebar_action_hint_matches_selected_item_type() {
        let mut app = app_for_sidebar_tests();

        let entries = app.sidebar_entries();
        let cloud_group_index = entries
            .iter()
            .position(|entry| matches!(entry, SidebarItem::ModelGroupHeader(ModelBucket::Cloud)))
            .expect("cloud model group should exist");
        app.sidebar_index = cloud_group_index;
        assert_eq!(
            sidebar_action_hint(&app),
            "Enter collapse/expand cloud models"
        );

        let entries = app.sidebar_entries();
        let session_index = entries
            .iter()
            .position(|entry| matches!(entry, SidebarItem::WorkspaceSession(_, _)))
            .expect("workspace session item should exist");
        app.sidebar_index = session_index;
        assert_eq!(
            sidebar_action_hint(&app),
            "Enter open session | /session rename <id> <title> | /session delete"
        );
    }

    #[test]
    fn estimate_input_visual_lines_counts_newlines_and_wrapped_lines() {
        assert_eq!(estimate_input_visual_lines("", 20), 1);
        assert_eq!(estimate_input_visual_lines("hello", 20), 1);
        assert_eq!(estimate_input_visual_lines("line1\nline2", 20), 2);
        assert_eq!(estimate_input_visual_lines("abcdefghij", 4), 3);
        assert_eq!(estimate_input_visual_lines("abc\n\ndef", 10), 3);
    }

    #[test]
    fn input_panel_height_expands_for_multiline_and_clamps() {
        let mut app = app_for_sidebar_tests();
        let frame = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 40,
        };

        app.input = String::new();
        assert_eq!(input_panel_height(&app, frame), 3);

        app.input = "a\nb\nc\n".repeat(3);
        assert_eq!(input_panel_height(&app, frame), 8);

        let tiny_frame = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 5,
        };
        assert_eq!(input_panel_height(&app, tiny_frame), 3);
    }

    #[test]
    fn input_cursor_visual_position_handles_wrapping_and_newlines() {
        assert_eq!(input_cursor_visual_position("", 0, 10), (0, 0));
        assert_eq!(input_cursor_visual_position("hello", 5, 10), (0, 5));
        assert_eq!(input_cursor_visual_position("line1\nline2", 11, 10), (1, 5));
        assert_eq!(input_cursor_visual_position("abcd", 4, 4), (1, 0));
        assert_eq!(input_cursor_visual_position("abcdefghij", 10, 4), (2, 2));
        assert_eq!(input_cursor_visual_position("abc\n\n", 5, 8), (2, 0));
        assert_eq!(input_cursor_visual_position("a\n\nb", 2, 8), (1, 0));
    }

    #[test]
    fn input_cursor_screen_position_respects_visible_scroll_window() {
        let area = Rect {
            x: 5,
            y: 10,
            width: 12,
            height: 5,
        };
        let inner_width = area.width.saturating_sub(2).max(1);
        let inner_height = area.height.saturating_sub(2) as usize;
        let input = "one\ntwo\nthree\nfour";
        let cursor = input.chars().count();

        let hidden =
            input_cursor_screen_position(input, cursor, area, inner_width, inner_height, 0);
        assert!(hidden.is_none());

        let visible =
            input_cursor_screen_position(input, cursor, area, inner_width, inner_height, 1);
        assert!(visible.is_some());
    }

    #[test]
    fn calculate_scroll_position_at_bottom_returns_max() {
        assert_eq!(calculate_scroll_position(0, 500), 500);
    }

    #[test]
    fn calculate_scroll_position_at_top_returns_zero() {
        assert_eq!(calculate_scroll_position(500, 500), 0);
    }

    #[test]
    fn calculate_scroll_position_in_middle_returns_correct() {
        assert_eq!(calculate_scroll_position(250, 500), 250);
    }

    #[test]
    fn calculate_scroll_position_zero_max_returns_zero() {
        assert_eq!(calculate_scroll_position(0, 0), 0);
    }

    #[test]
    fn calculate_scroll_position_stale_scroll_caps_at_zero() {
        assert_eq!(calculate_scroll_position(600, 500), 0);
    }

    #[test]
    fn calculate_scroll_position_chat_scroll_caps_at_max() {
        assert_eq!(calculate_scroll_position(400, 300), 0);
    }

    #[test]
    fn manual_reflow_short_line_unchanged() {
        let line = ratatui::text::Line::from(vec![ratatui::text::Span::raw("Hi")]);
        let result = manual_reflow(&[line], 80);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spans[0].content, "Hi");
    }

    #[test]
    fn manual_reflow_breaks_long_word() {
        let line =
            ratatui::text::Line::from(vec![ratatui::text::Span::raw("abcdefghijklmnopqrstuvwxyz")]);
        let result = manual_reflow(&[line], 10);
        assert!(result.len() > 1);
        for line in &result {
            let width = line
                .spans
                .iter()
                .map(|s| s.content.chars().count())
                .sum::<usize>();
            assert!(width <= 10);
        }
    }

    #[test]
    fn manual_reflow_wraps_long_line_at_spaces() {
        let line = ratatui::text::Line::from(vec![
            ratatui::text::Span::raw("Hello "),
            ratatui::text::Span::raw("world"),
            ratatui::text::Span::raw(" this"),
            ratatui::text::Span::raw(" is"),
            ratatui::text::Span::raw(" a"),
            ratatui::text::Span::raw(" test"),
        ]);
        let result = manual_reflow(&[line], 10);
        // The total width should be <= 10 per line
        for line in &result {
            let width = line
                .spans
                .iter()
                .map(|s| s.content.chars().count())
                .sum::<usize>();
            assert!(width <= 10);
        }
    }

    #[test]
    fn split_line_at_selection_highlights_partial_line() {
        use ratatui::style::Modifier;
        let line = ratatui::text::Line::from(vec![ratatui::text::Span::raw("Hello world")]);
        let selection_style = Style::default().add_modifier(Modifier::REVERSED);
        let result = split_line_at_selection(&line, 6, 10, selection_style);
        // The selected part "wor" should have reversed style
        let has_reversed = result.iter().any(|s| s.style == selection_style);
        assert!(has_reversed);
    }

    #[test]
    fn split_line_at_selection_full_line() {
        let line = ratatui::text::Line::from(vec![ratatui::text::Span::raw("Hello")]);
        let selection_style = Style::default().add_modifier(Modifier::REVERSED);
        let result = split_line_at_selection(&line, 0, 5, selection_style);
        // Entire line should be reversed
        let all_reversed = result.iter().all(|s| s.style == selection_style);
        assert!(all_reversed);
    }

    #[test]
    fn reflow_row_count_for_line_short_line_returns_one() {
        let line = Line::from("hello");
        assert_eq!(reflow_row_count_for_line(&line, 20), 1);
    }

    #[test]
    fn reflow_row_count_for_line_wrapping_returns_correct_rows() {
        let line = Line::from("a".repeat(30));
        assert_eq!(reflow_row_count_for_line(&line, 10), 3);
    }

    #[test]
    fn reflow_row_count_for_line_empty_returns_one() {
        let line = Line::from("");
        assert_eq!(reflow_row_count_for_line(&line, 20), 1);
    }

    #[test]
    fn reflow_row_count_for_line_width_zero_returns_one() {
        let line = Line::from("hello");
        assert_eq!(reflow_row_count_for_line(&line, 0), 1);
    }
}
