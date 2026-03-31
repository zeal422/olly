use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub(super) fn render_help_modal(frame: &mut Frame<'_>, app: &App, palette: super::ThemePalette) {
    if !app.show_help {
        return;
    }
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(help_lines())
            .style(Style::default().fg(palette.help_fg).bg(palette.help_bg))
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(palette.help_border)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn render_workspace_delete_modal(
    frame: &mut Frame<'_>,
    app: &App,
    palette: super::ThemePalette,
) {
    let Some(pending_delete) = &app.pending_workspace_delete else {
        return;
    };
    render_confirm_delete_modal(frame, "workspace", &pending_delete.name, palette);
}

pub(super) fn render_session_delete_modal(
    frame: &mut Frame<'_>,
    app: &App,
    palette: super::ThemePalette,
) {
    let Some(pending_delete) = &app.pending_session_delete else {
        return;
    };
    render_confirm_delete_modal(frame, "session", &pending_delete.name, palette);
}

pub(super) fn render_tool_confirmation_modal(
    frame: &mut Frame<'_>,
    app: &App,
    palette: super::ThemePalette,
) {
    let Some(pending) = &app.pending_tool_confirmation else {
        return;
    };
    let area = centered_rect(70, 50, frame.area());
    frame.render_widget(Clear, area);

    let body = format!(
        "Confirm tool execution?\n\n\
        Tool: {}\n\
        Step: {}/{}\n\n\
        Arguments:\n{}\n\n\
        Enter = execute    Esc = cancel",
        pending.tool_name, pending.step, pending.total_steps, pending.tool_args
    );

    frame.render_widget(
        Paragraph::new(body)
            .style(Style::default().fg(palette.modal_fg).bg(palette.modal_bg))
            .block(
                Block::default()
                    .title("Tool Confirmation")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(palette.modal_border)),
            )
            .alignment(ratatui::layout::Alignment::Left)
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_theme_picker_modal(
    frame: &mut Frame<'_>,
    app: &App,
    palette: super::ThemePalette,
) {
    let Some(state) = app.theme_picker else {
        return;
    };

    let area = centered_rect(62, 72, frame.area());
    frame.render_widget(Clear, area);

    let lines = app
        .theme_picker_entries()
        .iter()
        .enumerate()
        .map(|(index, theme)| {
            let cursor = if index == state.selected { ">" } else { " " };
            let active = if *theme == app.theme { "*" } else { " " };
            let mode = if theme.is_light() { "light" } else { "dark" };
            format!("{cursor} {active} {} ({mode})", theme.label())
        })
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "Theme Picker (live preview)\n\n{}\n\nUp/Down preview  |  Enter keep  |  Esc cancel",
        lines
    );

    frame.render_widget(
        Paragraph::new(body)
            .style(Style::default().fg(palette.help_fg).bg(palette.help_bg))
            .block(
                Block::default()
                    .title("Themes (/themes)")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(palette.help_border)),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_confirm_delete_modal(
    frame: &mut Frame<'_>,
    noun: &str,
    name: &str,
    palette: super::ThemePalette,
) {
    let area = centered_rect(58, 28, frame.area());
    frame.render_widget(Clear, area);
    let body = confirm_delete_body(noun, name);
    frame.render_widget(
        Paragraph::new(body)
            .style(Style::default().fg(palette.modal_fg).bg(palette.modal_bg))
            .block(
                Block::default()
                    .title("Confirm Delete")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(palette.modal_border)),
            )
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn confirm_delete_body(noun: &str, name: &str) -> String {
    format!("Delete {noun}?\n\n{name}\n\nEnter = confirm    Esc = cancel")
}

fn help_lines() -> Vec<Line<'static>> {
    vec![
        Line::from("Ctrl+Enter  Send message"),
        Line::from("Enter       New line (or run slash command)"),
        Line::from("Ctrl+Q      Quit (press twice)"),
        Line::from("F1          Toggle help"),
        Line::from("Esc         Close help/toast"),
        Line::from("Tab         Switch focus (or cycle slash autocomplete)"),
        Line::from("Arrows      Move input caret in chat (or move slash/sidebar selection)"),
        Line::from("PgUp/PgDn   Scroll chat faster"),
        Line::from("Enter       Select sidebar item (headers collapse/expand)"),
        Line::from(""),
        Line::from("Slash commands:"),
        Line::from(
            "/commands, /models, /themes, /search, /export, /workspace, /session, /checkpoint",
        ),
        Line::from("/attach, /clear-attachments, /model, /url, /tools"),
    ]
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::{centered_rect, confirm_delete_body};
    use ratatui::layout::Rect;

    #[test]
    fn confirm_delete_body_keeps_expected_controls_text() {
        let body = confirm_delete_body("workspace", "Default Workspace");
        assert!(body.contains("Delete workspace?"));
        assert!(body.contains("Default Workspace"));
        assert!(body.contains("Enter = confirm"));
        assert!(body.contains("Esc = cancel"));
    }

    #[test]
    fn centered_rect_stays_within_bounds() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 40,
        };
        let centered = centered_rect(60, 50, area);
        assert!(centered.width <= area.width);
        assert!(centered.height <= area.height);
        assert!(centered.x + centered.width <= area.x + area.width);
        assert!(centered.y + centered.height <= area.y + area.height);
    }
}
