use std::sync::OnceLock;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

use crate::app::{App, ConnectionStatus, FocusPane, SidebarItem, ToastLevel};

const SPINNER_FRAMES: &[&str] = &["|", "/", "-", "\\"];

struct SyntaxEngine {
    syntaxes: SyntaxSet,
    theme: Theme,
}

static SYNTAX_ENGINE: OnceLock<Option<SyntaxEngine>> = OnceLock::new();

fn syntax_engine() -> Option<&'static SyntaxEngine> {
    SYNTAX_ENGINE
        .get_or_init(|| {
            let syntaxes = SyntaxSet::load_defaults_newlines();
            let themes = ThemeSet::load_defaults();
            let theme = themes
                .themes
                .get("base16-ocean.dark")
                .cloned()
                .or_else(|| themes.themes.values().next().cloned())?;
            Some(SyntaxEngine { syntaxes, theme })
        })
        .as_ref()
}

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    if frame.area().width < 2 || frame.area().height < 2 {
        return;
    }

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_status_bar(frame, app, root[0]);

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

    render_models_panel(frame, app, body[0]);
    render_chat_panel(frame, app, body[1]);
    render_input_panel(frame, app, root[2]);

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
                .bg(Color::Rgb(20, 63, 114))
                .fg(Color::White),
            ToastLevel::Success => Style::default().bg(Color::Rgb(13, 97, 65)).fg(Color::White),
            ToastLevel::Error => Style::default()
                .bg(Color::Rgb(120, 33, 33))
                .fg(Color::White),
        };
        frame.render_widget(
            Paragraph::new(toast.message.as_str())
                .style(style)
                .block(Block::default().borders(Borders::ALL).title("Toast")),
            area,
        );
    }

    if app.show_help {
        let area = centered_rect(70, 70, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(help_text())
                .style(Style::default().fg(Color::White).bg(Color::Rgb(16, 21, 28)))
                .block(
                    Block::default()
                        .title("Help")
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Rgb(96, 212, 165))),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }
}

fn render_status_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
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
    let status = format!(
        "{spinner} ws={} | model={} | conn={} | imgs={} | {hl_mode} | {} | ? help",
        app.workspace_name,
        app.model,
        connection,
        app.pending_images.len(),
        app.status
    );
    frame.render_widget(
        Paragraph::new(status).style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(100, 230, 173))
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
}

fn render_models_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let mut lines = Vec::new();
    lines.push(String::from("Models"));
    if app.models.is_empty() {
        lines.push(String::from("  (none loaded)"));
    } else {
        for (index, model) in app.models.iter().enumerate() {
            let is_selected = app.sidebar_item() == Some(SidebarItem::Model(index));
            let marker = if is_selected { ">" } else { " " };
            let active = if model == &app.model { "*" } else { " " };
            lines.push(format!("{marker}{active} {model}"));
        }
    }

    lines.push(String::new());
    lines.push(String::from("Workspaces"));
    if app.workspaces.is_empty() {
        lines.push(String::from("  (none)"));
    } else {
        for (index, workspace) in app.workspaces.iter().enumerate() {
            let is_selected = app.sidebar_item() == Some(SidebarItem::Workspace(index));
            let marker = if is_selected { ">" } else { " " };
            let active = if workspace.name == app.workspace_name {
                "*"
            } else {
                " "
            };
            lines.push(format!("{marker}{active} {}", workspace.name));
        }
    }

    let border_style = if app.focus == FocusPane::Workspaces {
        Style::default().fg(Color::Rgb(70, 190, 140))
    } else {
        Style::default().fg(Color::DarkGray)
    };

    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .style(Style::default().fg(Color::Rgb(210, 220, 224)))
            .block(
                Block::default()
                    .title("Sidebar (Tab focus, Enter select)")
                    .borders(Borders::ALL)
                    .style(border_style),
            ),
        area,
    );
}

fn render_chat_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let border_style = if app.focus == FocusPane::Chat {
        Style::default().fg(Color::Rgb(70, 190, 140))
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let text = render_chat_text(app);
    let line_count = text.lines.len() as u16;
    let viewport_height = area.height.saturating_sub(2);
    let max_scroll = line_count.saturating_sub(viewport_height);
    let scroll_y = app.chat_scroll.min(max_scroll);

    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(Color::White))
            .scroll((scroll_y, 0))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title("Chat")
                    .borders(Borders::ALL)
                    .style(border_style),
            ),
        area,
    );
}

fn render_input_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    frame.render_widget(
        Paragraph::new(app.input.as_str())
            .style(Style::default().fg(Color::Rgb(230, 240, 240)))
            .block(
                Block::default()
                    .title("Input (Ctrl+Enter send, Tab autocomplete)")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Rgb(60, 130, 110))),
            ),
        area,
    );
    render_autocomplete_popup(frame, app, area);
}

fn render_autocomplete_popup(frame: &mut Frame<'_>, app: &App, input_area: Rect) {
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
            .style(Style::default().fg(Color::Rgb(205, 224, 239)))
            .block(
                Block::default()
                    .title("Autocomplete (Tab)")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Rgb(100, 175, 220))),
            ),
        popup_area,
    );
}

fn render_chat_text(app: &App) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for message in &app.messages {
        let (avatar, align, base_style, header_style) = match message.role.as_str() {
            "You" => (
                "YOU",
                Alignment::Right,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(112, 230, 174)),
                Style::default()
                    .fg(Color::Rgb(220, 255, 240))
                    .add_modifier(Modifier::BOLD),
            ),
            "Assistant" => (
                "AI",
                Alignment::Left,
                Style::default()
                    .fg(Color::Rgb(225, 232, 238))
                    .bg(Color::Rgb(53, 62, 72)),
                Style::default()
                    .fg(Color::Rgb(180, 196, 210))
                    .add_modifier(Modifier::BOLD),
            ),
            "Tool" => (
                "TOOL",
                Alignment::Left,
                Style::default()
                    .fg(Color::Rgb(230, 230, 190))
                    .bg(Color::Rgb(62, 63, 35)),
                Style::default()
                    .fg(Color::Rgb(232, 236, 170))
                    .add_modifier(Modifier::BOLD),
            ),
            _ => (
                "SYS",
                Alignment::Center,
                Style::default()
                    .fg(Color::Rgb(200, 200, 205))
                    .bg(Color::Rgb(44, 44, 55)),
                Style::default()
                    .fg(Color::Gray)
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
                        .fg(Color::Rgb(45, 55, 45))
                        .bg(Color::Rgb(197, 224, 197))
                        .add_modifier(Modifier::BOLD),
                )])
                .alignment(align),
            );
        }

        let message_lines = markdown_lines(&message.content, base_style, app.syntax_highlighting);
        for spans in message_lines {
            lines.push(Line::from(spans).alignment(align));
        }
        lines.push(Line::default());
    }

    Text::from(lines)
}

fn markdown_lines(
    content: &str,
    base_style: Style,
    enable_highlighting: bool,
) -> Vec<Vec<Span<'static>>> {
    let mut out = Vec::new();
    let mut in_code_block = false;
    let mut highlighter: Option<HighlightLines<'static>> = None;
    let mut syntax_set_ref: Option<&'static SyntaxSet> = None;

    for raw in content.lines() {
        let trimmed = raw.trim_end();
        if trimmed.trim_start().starts_with("```") {
            if in_code_block {
                in_code_block = false;
                highlighter = None;
                syntax_set_ref = None;
            } else {
                in_code_block = true;
                if enable_highlighting {
                    if let Some(engine) = syntax_engine() {
                        let lang = trimmed
                            .trim_start()
                            .trim_start_matches("```")
                            .split_whitespace()
                            .next();
                        let syntax = syntax_for_lang(&engine.syntaxes, lang);
                        highlighter = Some(HighlightLines::new(syntax, &engine.theme));
                        syntax_set_ref = Some(&engine.syntaxes);
                    }
                }
            }
            continue;
        }

        if in_code_block {
            out.push(render_code_line(
                trimmed,
                &mut highlighter,
                syntax_set_ref,
                base_style,
                enable_highlighting,
            ));
            continue;
        }

        let leading = trimmed.trim_start();
        let normalized = if let Some(rest) = leading.strip_prefix("- ") {
            format!("- {rest}")
        } else if let Some(rest) = leading.strip_prefix("* ") {
            format!("- {rest}")
        } else {
            leading.to_string()
        };

        let spans = inline_markdown_spans(&normalized, base_style);
        out.push(pad_line(spans, base_style));
    }

    if out.is_empty() {
        out.push(vec![Span::styled("  ", base_style)]);
    }

    out
}

fn syntax_for_lang<'a>(syntaxes: &'a SyntaxSet, lang: Option<&str>) -> &'a SyntaxReference {
    if let Some(token) = lang {
        if let Some(syntax) = syntaxes.find_syntax_by_token(token) {
            return syntax;
        }
    }
    syntaxes.find_syntax_plain_text()
}

fn render_code_line(
    line: &str,
    highlighter: &mut Option<HighlightLines<'static>>,
    syntax_set_ref: Option<&'static SyntaxSet>,
    base_style: Style,
    enable_highlighting: bool,
) -> Vec<Span<'static>> {
    let fallback = || {
        pad_line(
            vec![Span::styled(
                line.to_string(),
                base_style
                    .fg(Color::Rgb(206, 217, 228))
                    .bg(Color::Rgb(34, 40, 46))
                    .add_modifier(Modifier::DIM),
            )],
            base_style
                .fg(Color::Rgb(206, 217, 228))
                .bg(Color::Rgb(34, 40, 46))
                .add_modifier(Modifier::DIM),
        )
    };

    if !enable_highlighting {
        return fallback();
    }

    let Some(hl) = highlighter.as_mut() else {
        return fallback();
    };
    let Some(syntax_set) = syntax_set_ref else {
        return fallback();
    };

    match hl.highlight_line(line, syntax_set) {
        Ok(ranges) => {
            let mut spans = Vec::new();
            for (style, text) in ranges {
                if !text.is_empty() {
                    spans.push(Span::styled(text.to_string(), syntect_style(style)));
                }
            }
            if spans.is_empty() {
                spans.push(Span::styled(String::new(), base_style));
            }
            pad_line(spans, base_style)
        }
        Err(_) => fallback(),
    }
}

fn syntect_style(style: SyntectStyle) -> Style {
    Style::default()
        .fg(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ))
        .bg(Color::Rgb(
            style.background.r,
            style.background.g,
            style.background.b,
        ))
}

fn inline_markdown_spans(line: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buffer = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut code = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '`' {
            flush_segment(&mut spans, &mut buffer, base_style, bold, italic, code);
            code = !code;
            i += 1;
            continue;
        }

        if !code && chars[i] == '*' {
            if i + 1 < chars.len() && chars[i + 1] == '*' {
                flush_segment(&mut spans, &mut buffer, base_style, bold, italic, code);
                bold = !bold;
                i += 2;
                continue;
            }
            flush_segment(&mut spans, &mut buffer, base_style, bold, italic, code);
            italic = !italic;
            i += 1;
            continue;
        }

        buffer.push(chars[i]);
        i += 1;
    }

    flush_segment(&mut spans, &mut buffer, base_style, bold, italic, code);
    spans
}

fn flush_segment(
    spans: &mut Vec<Span<'static>>,
    buffer: &mut String,
    base_style: Style,
    bold: bool,
    italic: bool,
    code: bool,
) {
    if buffer.is_empty() {
        return;
    }

    let mut style = base_style;
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if code {
        style = style
            .fg(Color::Rgb(223, 233, 242))
            .bg(Color::Rgb(34, 40, 46))
            .add_modifier(Modifier::DIM);
    }

    spans.push(Span::styled(std::mem::take(buffer), style));
}

fn pad_line(mut spans: Vec<Span<'static>>, style: Style) -> Vec<Span<'static>> {
    let mut padded = Vec::with_capacity(spans.len() + 2);
    padded.push(Span::styled(" ".to_string(), style));
    padded.append(&mut spans);
    padded.push(Span::styled(" ".to_string(), style));
    padded
}

fn help_text() -> &'static str {
    "Ctrl+Enter  Send message\n\
Enter       New line (or run slash command)\n\
Ctrl+Q      Quit (press twice)\n\
?           Toggle help\n\
Esc         Close help/toast\n\
Tab         Switch focus (or cycle slash autocomplete)\n\
Up/Down     Scroll chat (or move sidebar selection)\n\
PgUp/PgDn   Fast scroll\n\
Enter       Select sidebar item when sidebar is focused\n\
\n\
Slash commands:\n\
/commands, /models, /search, /export, /workspace, /checkpoint\n\
/attach, /clear-attachments, /model, /url"
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
