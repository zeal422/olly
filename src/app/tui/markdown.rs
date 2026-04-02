//! Markdown rendering with syntax highlighting.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

use std::sync::OnceLock;

use super::palette::ThemePalette;

/// Render markdown content to styled spans with optional syntax highlighting.
pub(crate) fn markdown_lines(
    content: &str,
    base_style: Style,
    enable_highlighting: bool,
    palette: ThemePalette,
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
                palette,
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

        let spans = inline_markdown_spans(&normalized, base_style, palette);
        out.push(pad_line(spans, base_style));
    }

    if out.is_empty() {
        out.push(vec![Span::styled("  ", base_style)]);
    }

    out
}

/// Syntax highlighting engine with lazy initialization.
struct SyntaxEngine {
    syntaxes: SyntaxSet,
    theme: Theme,
}

static SYNTAX_ENGINE: OnceLock<Option<SyntaxEngine>> = OnceLock::new();

/// Get or initialize the syntax highlighting engine.
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

/// Find syntax reference for a language token.
fn syntax_for_lang<'a>(syntaxes: &'a SyntaxSet, lang: Option<&str>) -> &'a SyntaxReference {
    if let Some(token) = lang {
        if let Some(syntax) = syntaxes.find_syntax_by_token(token) {
            return syntax;
        }
    }
    syntaxes.find_syntax_plain_text()
}

/// Render a single code line with syntax highlighting.
fn render_code_line(
    line: &str,
    highlighter: &mut Option<HighlightLines<'static>>,
    syntax_set_ref: Option<&'static SyntaxSet>,
    base_style: Style,
    enable_highlighting: bool,
    palette: ThemePalette,
) -> Vec<Span<'static>> {
    let fallback = || {
        pad_line(
            vec![Span::styled(
                line.to_string(),
                base_style
                    .fg(palette.code_fg)
                    .bg(palette.code_bg)
                    .add_modifier(Modifier::DIM),
            )],
            base_style
                .fg(palette.code_fg)
                .bg(palette.code_bg)
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

/// Convert syntect style to ratatui style.
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

/// Parse inline markdown (bold, italic, code) to styled spans.
fn inline_markdown_spans(
    line: &str,
    base_style: Style,
    palette: ThemePalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buffer = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut code = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '`' {
            flush_segment(
                &mut spans,
                &mut buffer,
                base_style,
                bold,
                italic,
                code,
                palette,
            );
            code = !code;
            i += 1;
            continue;
        }

        if !code && chars[i] == '*' {
            if i + 1 < chars.len() && chars[i + 1] == '*' {
                flush_segment(
                    &mut spans,
                    &mut buffer,
                    base_style,
                    bold,
                    italic,
                    code,
                    palette,
                );
                bold = !bold;
                i += 2;
                continue;
            }
            flush_segment(
                &mut spans,
                &mut buffer,
                base_style,
                bold,
                italic,
                code,
                palette,
            );
            italic = !italic;
            i += 1;
            continue;
        }

        buffer.push(chars[i]);
        i += 1;
    }

    flush_segment(
        &mut spans,
        &mut buffer,
        base_style,
        bold,
        italic,
        code,
        palette,
    );
    spans
}

/// Flush accumulated buffer segment to spans with appropriate styling.
fn flush_segment(
    spans: &mut Vec<Span<'static>>,
    buffer: &mut String,
    base_style: Style,
    bold: bool,
    italic: bool,
    code: bool,
    palette: ThemePalette,
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
            .fg(palette.inline_code_fg)
            .bg(palette.inline_code_bg)
            .add_modifier(Modifier::DIM);
    }

    spans.push(Span::styled(std::mem::take(buffer), style));
}

/// Pad a line of spans with leading and trailing spaces.
fn pad_line(mut spans: Vec<Span<'static>>, style: Style) -> Vec<Span<'static>> {
    let mut padded = Vec::with_capacity(spans.len() + 2);
    padded.push(Span::styled(" ".to_string(), style));
    padded.append(&mut spans);
    padded.push(Span::styled(" ".to_string(), style));
    padded
}
