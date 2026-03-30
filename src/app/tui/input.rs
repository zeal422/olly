use ratatui::{layout::Rect, Frame};

pub(super) fn render_input_panel(
    frame: &mut Frame<'_>,
    app: &crate::app::App,
    area: Rect,
    palette: super::ThemePalette,
) {
    super::render_input_panel(frame, app, area, palette);
}
