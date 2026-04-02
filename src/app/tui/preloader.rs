use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::sync::OnceLock;
use std::time::Instant;

const SPINNER: &[&str] = &["-", "\\", "|", "/"];

const DOS_REBEL_OLLY: &[&str] = &[
    "    \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}    \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}       \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}       \u{2588}\u{2588}\u{2588}\u{2588}\u{2588} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
    "  \u{2588}\u{2588}\u{2588}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2588}\u{2588}\u{2588} \u{2591}\u{2591}\u{2588}\u{2588}\u{2588}       \u{2591}\u{2591}\u{2588}\u{2588}\u{2588}       \u{2591}\u{2591}\u{2588}\u{2588}\u{2588} \u{2591}\u{2591}\u{2588}\u{2588}\u{2588} ",
    " \u{2588}\u{2588}\u{2588}     \u{2591}\u{2591}\u{2588}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}\u{2588}        \u{2591}\u{2588}\u{2588}\u{2588}        \u{2591}\u{2591}\u{2588}\u{2588}\u{2588} \u{2588}\u{2588}\u{2588}  ",
    "\u{2591}\u{2588}\u{2588}\u{2588}      \u{2591}\u{2588}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}\u{2588}        \u{2591}\u{2588}\u{2588}\u{2588}         \u{2591}\u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}   ",
    "\u{2591}\u{2588}\u{2588}\u{2588}      \u{2591}\u{2588}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}\u{2588}        \u{2591}\u{2588}\u{2588}\u{2588}          \u{2591}\u{2591}\u{2588}\u{2588}\u{2588}    ",
    "\u{2591}\u{2591}\u{2588}\u{2588}\u{2588}     \u{2588}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}\u{2588}      \u{2588} \u{2591}\u{2588}\u{2588}\u{2588}      \u{2588}    \u{2591}\u{2588}\u{2588}\u{2588}    ",
    " \u{2591}\u{2591}\u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2591}   \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}    \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}   ",
    "   \u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}    \u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591} \u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}    \u{2591}\u{2591}\u{2591}\u{2591}\u{2591}    ",
];

const LOGO_COLORS: &[Color] = &[
    Color::Rgb(80, 160, 248),
    Color::Rgb(110, 184, 255),
    Color::Rgb(92, 168, 252),
    Color::Rgb(124, 194, 255),
];

static ANIMATION_EPOCH: OnceLock<Instant> = OnceLock::new();

fn animation_epoch() -> Instant {
    *ANIMATION_EPOCH.get_or_init(Instant::now)
}

fn phase_from_elapsed_ms(elapsed_ms: u128, period_ms: u128, cycle_len: usize) -> usize {
    let period = period_ms.max(1);
    let len = cycle_len.max(1);
    ((elapsed_ms / period) as usize) % len
}

fn animation_phase(period_ms: u128) -> usize {
    phase_from_elapsed_ms(
        animation_epoch().elapsed().as_millis(),
        period_ms,
        LOGO_COLORS.len(),
    )
}

fn logo_lines_for_phase(phase: usize) -> Vec<Line<'static>> {
    let logo_color = LOGO_COLORS[phase % LOGO_COLORS.len()];
    DOS_REBEL_OLLY
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                (*line).to_string(),
                Style::default().fg(logo_color),
            ))
        })
        .collect()
}

pub(super) fn animated_logo_lines() -> Vec<Line<'static>> {
    // Used by preloader: slightly faster cadence.
    logo_lines_for_phase(animation_phase(110))
}

pub(super) fn animated_logo_lines_welcome() -> Vec<Line<'static>> {
    // Used by welcome screen: intentionally slower cadence.
    logo_lines_for_phase(animation_phase(280))
}

pub fn render_preloader(frame: &mut Frame<'_>, status: &str) {
    let area = frame.area();
    if area.width < 4 || area.height < 4 {
        return;
    }

    let logo_lines = animated_logo_lines();
    let logo_height = logo_lines.len() as u16;

    let top_pad = area.height.saturating_sub(logo_height + 2) / 2;

    for (idx, line) in logo_lines.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(line.clone()).alignment(Alignment::Center),
            Rect::new(0, top_pad + idx as u16, area.width, 1),
        );
    }

    let spinner_idx = ((animation_epoch().elapsed().as_millis() / 110) as usize) % SPINNER.len();
    let status_text = format!("{} {}", SPINNER[spinner_idx], status);
    let status_y = top_pad.saturating_add(logo_height).saturating_add(1);
    if status_y < area.height {
        frame.render_widget(
            Paragraph::new(status_text)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            Rect::new(0, status_y, area.width, 1),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{logo_lines_for_phase, phase_from_elapsed_ms, DOS_REBEL_OLLY, LOGO_COLORS};
    use ratatui::style::Color;

    fn line_text(line: &ratatui::text::Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn phase_from_elapsed_ms_cycles_by_period() {
        assert_eq!(phase_from_elapsed_ms(0, 110, 4), 0);
        assert_eq!(phase_from_elapsed_ms(109, 110, 4), 0);
        assert_eq!(phase_from_elapsed_ms(110, 110, 4), 1);
        assert_eq!(phase_from_elapsed_ms(220, 110, 4), 2);
        assert_eq!(phase_from_elapsed_ms(550, 110, 4), 1);
    }

    #[test]
    fn phase_from_elapsed_ms_handles_zero_period_and_len() {
        assert_eq!(phase_from_elapsed_ms(25, 0, 4), 1);
        assert_eq!(phase_from_elapsed_ms(25, 10, 0), 0);
    }

    #[test]
    fn dos_rebel_logo_keeps_expected_shape_rows() {
        assert_eq!(DOS_REBEL_OLLY.len(), 8);
        assert!(DOS_REBEL_OLLY.iter().all(|line| !line.is_empty()));
        assert!(DOS_REBEL_OLLY
            .iter()
            .all(|line| line.contains('\u{2588}') || line.contains('\u{2591}')));
    }

    #[test]
    fn logo_lines_for_phase_preserves_text_and_changes_color_by_phase() {
        let phase0 = logo_lines_for_phase(0);
        let phase1 = logo_lines_for_phase(1);

        assert_eq!(phase0.len(), DOS_REBEL_OLLY.len());
        assert_eq!(phase1.len(), DOS_REBEL_OLLY.len());

        for (idx, expected) in DOS_REBEL_OLLY.iter().enumerate() {
            assert_eq!(line_text(&phase0[idx]), *expected);
            assert_eq!(line_text(&phase1[idx]), *expected);
        }

        let color0 = phase0[0].spans[0].style.fg;
        let color1 = phase1[0].spans[0].style.fg;
        assert_eq!(color0, Some(LOGO_COLORS[0]));
        assert_eq!(color1, Some(LOGO_COLORS[1]));
        assert_ne!(color0, color1);

        // Ensure palette is still our expected RGB family.
        assert!(matches!(LOGO_COLORS[0], Color::Rgb(80, 160, 248)));
    }
}
