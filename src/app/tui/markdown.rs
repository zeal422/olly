pub(super) fn markdown_lines(
    content: &str,
    base_style: ratatui::style::Style,
    enable_highlighting: bool,
    palette: super::ThemePalette,
) -> Vec<Vec<ratatui::text::Span<'static>>> {
    super::markdown_lines(content, base_style, enable_highlighting, palette)
}
