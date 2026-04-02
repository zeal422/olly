//! Input panel rendering - input field, cursor placement, and autocomplete popup.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::{
    state::{FocusPane, UiTheme},
    App,
};

/// Render the input panel with cursor and autocomplete popup.
pub(super) fn render_input_panel(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    palette: super::ThemePalette,
) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let inner_width = area.width.saturating_sub(2).max(1);
    let inner_height = area.height.saturating_sub(2) as usize;
    let total_visual_lines = estimate_input_visual_lines(&app.input, inner_width);
    let (cursor_line, _) =
        input_cursor_visual_position(&app.input, app.input_cursor_char_index, inner_width);
    let max_scroll_y = total_visual_lines.saturating_sub(inner_height) as u16;
    let scroll_y =
        (cursor_line.saturating_add(1).saturating_sub(inner_height) as u16).min(max_scroll_y);

    frame.render_widget(
        Paragraph::new(app.input.as_str())
            .style(
                Style::default()
                    .fg(palette.input_text)
                    .bg(input_background(app.theme)),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0))
            .block(
                Block::default()
                    .title("Input (Ctrl+Enter send, arrows move caret)")
                    .borders(Borders::ALL)
                    .style(
                        Style::default()
                            .fg(palette.input_border)
                            .bg(input_background(app.theme)),
                    ),
            ),
        area,
    );

    let show_input_cursor = should_show_input_cursor(app);

    if show_input_cursor {
        if let Some((cursor_x, cursor_y)) = input_cursor_screen_position(
            &app.input,
            app.input_cursor_char_index,
            area,
            inner_width,
            inner_height,
            scroll_y,
        ) {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    render_autocomplete_popup(frame, app, area, palette);
}

fn should_show_input_cursor(app: &App) -> bool {
    // Hide cursor on welcome screen (no active workspace) to avoid distracting blink.
    app.active_workspace_id > 0 && (app.focus == FocusPane::Chat || app.is_slash_command_input())
}

/// Calculate the required height for the input panel.
pub(super) fn input_panel_height(app: &App, frame_area: Rect) -> u16 {
    let min_total_height = 3;
    let max_content_lines = 6_usize;
    let inner_width = frame_area.width.saturating_sub(2).max(1);
    let visual_lines = estimate_input_visual_lines(&app.input, inner_width);
    let desired_total = visual_lines.min(max_content_lines) as u16 + 2;

    // Reserve room for status + at least one body row.
    let max_allowed = frame_area.height.saturating_sub(2).max(min_total_height);
    desired_total.clamp(min_total_height, max_allowed)
}

/// Estimate the number of visual lines for the input text.
pub(super) fn estimate_input_visual_lines(input: &str, inner_width: u16) -> usize {
    let width = inner_width.max(1) as usize;
    if input.is_empty() {
        return 1;
    }

    input
        .split('\n')
        .map(|line| {
            let chars = line.chars().count();
            chars.max(1).div_ceil(width)
        })
        .sum::<usize>()
        .max(1)
}

/// Calculate the visual position (line, column) of the cursor.
pub(super) fn input_cursor_visual_position(
    input: &str,
    cursor_char_index: usize,
    inner_width: u16,
) -> (usize, usize) {
    let width = inner_width.max(1) as usize;
    if input.is_empty() || cursor_char_index == 0 {
        return (0, 0);
    }

    let clamped_cursor = cursor_char_index.min(input.chars().count());
    let mut line = 0_usize;
    let mut col = 0_usize;
    for (index, ch) in input.chars().enumerate() {
        if index >= clamped_cursor {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
            continue;
        }
        if col >= width {
            line += 1;
            col = 0;
        }
        col += 1;
    }

    // Place cursor at the start of the next wrapped row when the current row is full.
    if col >= width {
        line += 1;
        col = 0;
    }

    (line, col)
}

/// Calculate the screen position of the cursor.
pub(super) fn input_cursor_screen_position(
    input: &str,
    cursor_char_index: usize,
    area: Rect,
    inner_width: u16,
    inner_height: usize,
    scroll_y: u16,
) -> Option<(u16, u16)> {
    if inner_width == 0 || inner_height == 0 {
        return None;
    }

    let (cursor_line, cursor_col) =
        input_cursor_visual_position(input, cursor_char_index, inner_width);
    let scroll_top = scroll_y as usize;
    if cursor_line < scroll_top || cursor_line >= scroll_top + inner_height {
        return None;
    }

    let visible_line = (cursor_line - scroll_top) as u16;
    let max_x_offset = inner_width.saturating_sub(1) as usize;
    let x_offset = cursor_col.min(max_x_offset) as u16;
    let cursor_x = area.x.saturating_add(1).saturating_add(x_offset);
    let cursor_y = area.y.saturating_add(1).saturating_add(visible_line);
    Some((cursor_x, cursor_y))
}

/// Render the autocomplete popup above the input.
fn render_autocomplete_popup(
    frame: &mut Frame<'_>,
    app: &App,
    input_area: Rect,
    palette: super::ThemePalette,
) {
    if app.focus != FocusPane::Chat && !app.input.trim_start().starts_with('/') {
        return;
    }
    let suggestions = app.autocomplete_options();
    if suggestions.is_empty() {
        return;
    }
    if input_area.width < 4 {
        return;
    }

    let visible = suggestions.into_iter().take(6).collect::<Vec<_>>();
    let max_len = visible
        .iter()
        .map(|entry| entry.trim_end().len())
        .max()
        .unwrap_or(20) as u16;
    let width = (max_len + 6).min(input_area.width.saturating_sub(1)).max(8);
    let height = (visible.len() as u16 + 2).min(8);
    let y = input_area.y.saturating_sub(height);
    let x = input_area.x;

    let popup_area = Rect {
        x,
        y,
        width,
        height,
    };

    let preview = app.autocomplete_preview();
    let lines = visible
        .into_iter()
        .map(|entry| {
            if preview.as_deref() == Some(entry.as_str()) {
                format!("> {}", entry.trim_end())
            } else {
                format!("  {}", entry.trim_end())
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Paragraph::new(lines)
            .style(
                Style::default()
                    .fg(palette.autocomplete_text)
                    .bg(autocomplete_background(app.theme)),
            )
            .block(
                Block::default()
                    .title("Autocomplete (Tab)")
                    .borders(Borders::ALL)
                    .style(
                        Style::default()
                            .fg(palette.autocomplete_border)
                            .bg(autocomplete_background(app.theme)),
                    ),
            ),
        popup_area,
    );
}

/// Background color for input area based on theme.
fn input_background(theme: UiTheme) -> Color {
    if theme.is_light() {
        Color::Rgb(255, 255, 255)
    } else {
        Color::Rgb(20, 25, 32)
    }
}

/// Background color for autocomplete popup based on theme.
fn autocomplete_background(theme: UiTheme) -> Color {
    if theme.is_light() {
        Color::Rgb(255, 255, 255)
    } else {
        Color::Rgb(18, 24, 33)
    }
}

#[cfg(test)]
mod tests {
    use super::should_show_input_cursor;
    use crate::app::{state::FocusPane, App};

    fn new_app() -> App {
        App::new(
            String::from("http://localhost:11434"),
            String::from("qwen2.5-coder:7b"),
        )
    }

    #[test]
    fn should_show_input_cursor_hides_on_welcome_screen() {
        let mut app = new_app();
        app.active_workspace_id = 0;
        app.focus = FocusPane::Chat;
        assert!(!should_show_input_cursor(&app));
    }

    #[test]
    fn should_show_input_cursor_shows_in_active_workspace_chat_focus() {
        let mut app = new_app();
        app.active_workspace_id = 1;
        app.focus = FocusPane::Chat;
        assert!(should_show_input_cursor(&app));
    }

    #[test]
    fn should_show_input_cursor_shows_for_slash_command_outside_chat_focus() {
        let mut app = new_app();
        app.active_workspace_id = 1;
        app.focus = FocusPane::Workspaces;
        app.input = String::from("/workspace");
        assert!(should_show_input_cursor(&app));
    }

    #[test]
    fn should_show_input_cursor_hides_for_sidebar_non_command() {
        let mut app = new_app();
        app.active_workspace_id = 1;
        app.focus = FocusPane::Workspaces;
        app.input = String::from("hello");
        assert!(!should_show_input_cursor(&app));
    }
}
