use ratatui::{layout::Rect, Frame};

pub(super) fn render_status_bar(
    frame: &mut Frame<'_>,
    app: &crate::app::App,
    area: Rect,
    palette: super::ThemePalette,
) {
    super::render_status_bar(frame, app, area, palette);
}

pub(super) fn render_models_panel(
    frame: &mut Frame<'_>,
    app: &crate::app::App,
    area: Rect,
    palette: super::ThemePalette,
) {
    super::render_models_panel(frame, app, area, palette);
}
