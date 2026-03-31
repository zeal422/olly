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

use crate::app::{
    App, ConnectionStatus, FocusPane, ModelBucket, SidebarItem, SidebarSection, ToastLevel, UiTheme,
};

mod chat;
mod input;
mod markdown;
mod modals;
mod palette;
mod sidebar;

const SPINNER_FRAMES: &[&str] = &["|", "/", "-", "\\"];

fn calculate_scroll_position(chat_scroll: u16, max_scroll: u16) -> u16 {
    let from_bottom = chat_scroll.min(max_scroll);
    max_scroll.saturating_sub(from_bottom)
}

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

#[derive(Debug, Clone, Copy)]
struct ThemePalette {
    status_fg: Color,
    status_bg: Color,
    border_active: Color,
    border_inactive: Color,
    sidebar_text: Color,
    chat_text: Color,
    body_muted: Color,
    footer_hint: Color,
    footer_context: Color,
    input_text: Color,
    input_border: Color,
    toast_fg: Color,
    toast_info_bg: Color,
    toast_success_bg: Color,
    toast_error_bg: Color,
    help_fg: Color,
    help_bg: Color,
    help_border: Color,
    modal_fg: Color,
    modal_bg: Color,
    modal_border: Color,
    autocomplete_text: Color,
    autocomplete_border: Color,
    you_fg: Color,
    you_bg: Color,
    you_header: Color,
    assistant_fg: Color,
    assistant_bg: Color,
    assistant_header: Color,
    tool_fg: Color,
    tool_bg: Color,
    tool_header: Color,
    system_fg: Color,
    system_bg: Color,
    system_header: Color,
    image_badge_fg: Color,
    image_badge_bg: Color,
    code_fg: Color,
    code_bg: Color,
    inline_code_fg: Color,
    inline_code_bg: Color,
}

fn theme_palette(theme: UiTheme) -> ThemePalette {
    match theme {
        UiTheme::EmeraldDusk => ThemePalette {
            status_fg: Color::Black,
            status_bg: Color::Rgb(112, 230, 174),
            border_active: Color::Rgb(70, 190, 140),
            border_inactive: Color::Rgb(88, 98, 110),
            sidebar_text: Color::Rgb(210, 220, 224),
            chat_text: Color::White,
            body_muted: Color::Rgb(205, 220, 228),
            footer_hint: Color::Rgb(148, 176, 188),
            footer_context: Color::Rgb(130, 150, 160),
            input_text: Color::Rgb(230, 240, 240),
            input_border: Color::Rgb(60, 130, 110),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(20, 63, 114),
            toast_success_bg: Color::Rgb(13, 97, 65),
            toast_error_bg: Color::Rgb(120, 33, 33),
            help_fg: Color::White,
            help_bg: Color::Rgb(16, 21, 28),
            help_border: Color::Rgb(96, 212, 165),
            modal_fg: Color::White,
            modal_bg: Color::Rgb(22, 18, 18),
            modal_border: Color::Rgb(235, 114, 114),
            autocomplete_text: Color::Rgb(205, 224, 239),
            autocomplete_border: Color::Rgb(100, 175, 220),
            you_fg: Color::Black,
            you_bg: Color::Rgb(112, 230, 174),
            you_header: Color::Rgb(220, 255, 240),
            assistant_fg: Color::Rgb(225, 232, 238),
            assistant_bg: Color::Rgb(53, 62, 72),
            assistant_header: Color::Rgb(180, 196, 210),
            tool_fg: Color::Rgb(230, 230, 190),
            tool_bg: Color::Rgb(62, 63, 35),
            tool_header: Color::Rgb(232, 236, 170),
            system_fg: Color::Rgb(200, 200, 205),
            system_bg: Color::Rgb(44, 44, 55),
            system_header: Color::Gray,
            image_badge_fg: Color::Rgb(45, 55, 45),
            image_badge_bg: Color::Rgb(197, 224, 197),
            code_fg: Color::Rgb(206, 217, 228),
            code_bg: Color::Rgb(34, 40, 46),
            inline_code_fg: Color::Rgb(223, 233, 242),
            inline_code_bg: Color::Rgb(34, 40, 46),
        },
        UiTheme::NeonCircuit => ThemePalette {
            status_fg: Color::Black,
            status_bg: Color::Rgb(102, 224, 245),
            border_active: Color::Rgb(69, 220, 214),
            border_inactive: Color::Rgb(86, 95, 108),
            sidebar_text: Color::Rgb(214, 238, 241),
            chat_text: Color::Rgb(236, 250, 252),
            body_muted: Color::Rgb(196, 222, 226),
            footer_hint: Color::Rgb(133, 209, 214),
            footer_context: Color::Rgb(120, 181, 186),
            input_text: Color::Rgb(233, 251, 252),
            input_border: Color::Rgb(63, 170, 178),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(26, 82, 133),
            toast_success_bg: Color::Rgb(25, 122, 93),
            toast_error_bg: Color::Rgb(154, 51, 64),
            help_fg: Color::Rgb(238, 251, 252),
            help_bg: Color::Rgb(13, 20, 25),
            help_border: Color::Rgb(88, 231, 224),
            modal_fg: Color::Rgb(250, 240, 244),
            modal_bg: Color::Rgb(31, 14, 20),
            modal_border: Color::Rgb(255, 110, 143),
            autocomplete_text: Color::Rgb(221, 248, 252),
            autocomplete_border: Color::Rgb(88, 231, 224),
            you_fg: Color::Black,
            you_bg: Color::Rgb(130, 245, 217),
            you_header: Color::Rgb(236, 255, 249),
            assistant_fg: Color::Rgb(230, 243, 247),
            assistant_bg: Color::Rgb(40, 56, 66),
            assistant_header: Color::Rgb(150, 220, 231),
            tool_fg: Color::Rgb(247, 231, 190),
            tool_bg: Color::Rgb(72, 56, 26),
            tool_header: Color::Rgb(255, 236, 159),
            system_fg: Color::Rgb(225, 212, 234),
            system_bg: Color::Rgb(55, 42, 67),
            system_header: Color::Rgb(184, 161, 199),
            image_badge_fg: Color::Rgb(22, 56, 52),
            image_badge_bg: Color::Rgb(190, 245, 236),
            code_fg: Color::Rgb(216, 232, 242),
            code_bg: Color::Rgb(22, 31, 40),
            inline_code_fg: Color::Rgb(225, 245, 255),
            inline_code_bg: Color::Rgb(22, 31, 40),
        },
        UiTheme::SunsetEmber => ThemePalette {
            status_fg: Color::Black,
            status_bg: Color::Rgb(255, 181, 118),
            border_active: Color::Rgb(236, 142, 73),
            border_inactive: Color::Rgb(101, 91, 92),
            sidebar_text: Color::Rgb(242, 224, 216),
            chat_text: Color::Rgb(248, 233, 227),
            body_muted: Color::Rgb(232, 210, 201),
            footer_hint: Color::Rgb(214, 167, 150),
            footer_context: Color::Rgb(188, 151, 138),
            input_text: Color::Rgb(251, 238, 233),
            input_border: Color::Rgb(190, 118, 84),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(48, 88, 140),
            toast_success_bg: Color::Rgb(36, 113, 77),
            toast_error_bg: Color::Rgb(154, 57, 57),
            help_fg: Color::Rgb(252, 237, 231),
            help_bg: Color::Rgb(34, 23, 24),
            help_border: Color::Rgb(255, 171, 120),
            modal_fg: Color::Rgb(255, 235, 228),
            modal_bg: Color::Rgb(42, 18, 18),
            modal_border: Color::Rgb(252, 130, 108),
            autocomplete_text: Color::Rgb(248, 226, 216),
            autocomplete_border: Color::Rgb(255, 171, 120),
            you_fg: Color::Black,
            you_bg: Color::Rgb(255, 195, 133),
            you_header: Color::Rgb(255, 244, 228),
            assistant_fg: Color::Rgb(238, 226, 221),
            assistant_bg: Color::Rgb(67, 53, 52),
            assistant_header: Color::Rgb(222, 186, 171),
            tool_fg: Color::Rgb(245, 229, 185),
            tool_bg: Color::Rgb(86, 62, 31),
            tool_header: Color::Rgb(255, 219, 147),
            system_fg: Color::Rgb(230, 209, 227),
            system_bg: Color::Rgb(67, 46, 68),
            system_header: Color::Rgb(190, 158, 192),
            image_badge_fg: Color::Rgb(64, 43, 31),
            image_badge_bg: Color::Rgb(244, 208, 178),
            code_fg: Color::Rgb(241, 223, 214),
            code_bg: Color::Rgb(50, 37, 36),
            inline_code_fg: Color::Rgb(255, 234, 224),
            inline_code_bg: Color::Rgb(50, 37, 36),
        },
        UiTheme::OceanMidnight => ThemePalette {
            status_fg: Color::Black,
            status_bg: Color::Rgb(136, 200, 255),
            border_active: Color::Rgb(101, 170, 242),
            border_inactive: Color::Rgb(80, 91, 111),
            sidebar_text: Color::Rgb(214, 225, 242),
            chat_text: Color::Rgb(233, 239, 252),
            body_muted: Color::Rgb(197, 210, 236),
            footer_hint: Color::Rgb(142, 176, 226),
            footer_context: Color::Rgb(124, 152, 201),
            input_text: Color::Rgb(236, 244, 255),
            input_border: Color::Rgb(88, 140, 226),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(34, 83, 141),
            toast_success_bg: Color::Rgb(31, 110, 88),
            toast_error_bg: Color::Rgb(154, 53, 72),
            help_fg: Color::Rgb(232, 239, 255),
            help_bg: Color::Rgb(15, 23, 38),
            help_border: Color::Rgb(116, 186, 255),
            modal_fg: Color::Rgb(236, 236, 250),
            modal_bg: Color::Rgb(21, 18, 37),
            modal_border: Color::Rgb(188, 138, 255),
            autocomplete_text: Color::Rgb(221, 236, 255),
            autocomplete_border: Color::Rgb(116, 186, 255),
            you_fg: Color::Black,
            you_bg: Color::Rgb(156, 216, 255),
            you_header: Color::Rgb(236, 248, 255),
            assistant_fg: Color::Rgb(226, 234, 245),
            assistant_bg: Color::Rgb(42, 55, 76),
            assistant_header: Color::Rgb(167, 194, 232),
            tool_fg: Color::Rgb(236, 226, 182),
            tool_bg: Color::Rgb(70, 61, 30),
            tool_header: Color::Rgb(248, 224, 146),
            system_fg: Color::Rgb(222, 217, 236),
            system_bg: Color::Rgb(58, 47, 73),
            system_header: Color::Rgb(182, 170, 205),
            image_badge_fg: Color::Rgb(30, 52, 78),
            image_badge_bg: Color::Rgb(182, 216, 248),
            code_fg: Color::Rgb(216, 231, 249),
            code_bg: Color::Rgb(27, 40, 60),
            inline_code_fg: Color::Rgb(226, 239, 255),
            inline_code_bg: Color::Rgb(27, 40, 60),
        },
        UiTheme::MonoAmber => ThemePalette {
            status_fg: Color::Black,
            status_bg: Color::Rgb(238, 188, 113),
            border_active: Color::Rgb(217, 163, 80),
            border_inactive: Color::Rgb(100, 100, 100),
            sidebar_text: Color::Rgb(225, 221, 214),
            chat_text: Color::Rgb(238, 234, 226),
            body_muted: Color::Rgb(205, 200, 190),
            footer_hint: Color::Rgb(177, 167, 147),
            footer_context: Color::Rgb(160, 150, 132),
            input_text: Color::Rgb(242, 238, 230),
            input_border: Color::Rgb(179, 144, 91),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(56, 88, 126),
            toast_success_bg: Color::Rgb(43, 111, 82),
            toast_error_bg: Color::Rgb(148, 64, 64),
            help_fg: Color::Rgb(236, 234, 228),
            help_bg: Color::Rgb(26, 26, 26),
            help_border: Color::Rgb(232, 184, 106),
            modal_fg: Color::Rgb(244, 235, 220),
            modal_bg: Color::Rgb(36, 25, 17),
            modal_border: Color::Rgb(222, 137, 94),
            autocomplete_text: Color::Rgb(231, 223, 212),
            autocomplete_border: Color::Rgb(213, 171, 104),
            you_fg: Color::Black,
            you_bg: Color::Rgb(238, 200, 136),
            you_header: Color::Rgb(252, 240, 216),
            assistant_fg: Color::Rgb(227, 226, 221),
            assistant_bg: Color::Rgb(56, 56, 56),
            assistant_header: Color::Rgb(190, 187, 178),
            tool_fg: Color::Rgb(248, 226, 178),
            tool_bg: Color::Rgb(77, 58, 29),
            tool_header: Color::Rgb(253, 214, 141),
            system_fg: Color::Rgb(228, 214, 229),
            system_bg: Color::Rgb(60, 50, 64),
            system_header: Color::Rgb(188, 168, 192),
            image_badge_fg: Color::Rgb(60, 47, 24),
            image_badge_bg: Color::Rgb(233, 209, 163),
            code_fg: Color::Rgb(226, 220, 209),
            code_bg: Color::Rgb(40, 38, 33),
            inline_code_fg: Color::Rgb(240, 232, 218),
            inline_code_bg: Color::Rgb(40, 38, 33),
        },
        UiTheme::SakuraNight => ThemePalette {
            status_fg: Color::Black,
            status_bg: Color::Rgb(255, 171, 214),
            border_active: Color::Rgb(227, 113, 185),
            border_inactive: Color::Rgb(101, 90, 110),
            sidebar_text: Color::Rgb(238, 221, 244),
            chat_text: Color::Rgb(246, 234, 252),
            body_muted: Color::Rgb(223, 204, 230),
            footer_hint: Color::Rgb(194, 157, 208),
            footer_context: Color::Rgb(174, 139, 191),
            input_text: Color::Rgb(249, 238, 252),
            input_border: Color::Rgb(183, 103, 168),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(52, 87, 144),
            toast_success_bg: Color::Rgb(35, 112, 84),
            toast_error_bg: Color::Rgb(156, 53, 81),
            help_fg: Color::Rgb(248, 236, 252),
            help_bg: Color::Rgb(31, 20, 35),
            help_border: Color::Rgb(255, 140, 214),
            modal_fg: Color::Rgb(255, 236, 246),
            modal_bg: Color::Rgb(42, 16, 27),
            modal_border: Color::Rgb(255, 116, 171),
            autocomplete_text: Color::Rgb(244, 225, 249),
            autocomplete_border: Color::Rgb(233, 131, 203),
            you_fg: Color::Black,
            you_bg: Color::Rgb(255, 189, 224),
            you_header: Color::Rgb(255, 242, 249),
            assistant_fg: Color::Rgb(237, 228, 243),
            assistant_bg: Color::Rgb(64, 50, 79),
            assistant_header: Color::Rgb(209, 180, 226),
            tool_fg: Color::Rgb(250, 229, 188),
            tool_bg: Color::Rgb(84, 62, 31),
            tool_header: Color::Rgb(255, 217, 142),
            system_fg: Color::Rgb(228, 212, 238),
            system_bg: Color::Rgb(66, 46, 78),
            system_header: Color::Rgb(190, 166, 204),
            image_badge_fg: Color::Rgb(70, 37, 58),
            image_badge_bg: Color::Rgb(245, 204, 229),
            code_fg: Color::Rgb(236, 223, 242),
            code_bg: Color::Rgb(44, 34, 52),
            inline_code_fg: Color::Rgb(247, 232, 255),
            inline_code_bg: Color::Rgb(44, 34, 52),
        },
        UiTheme::PaperMint => ThemePalette {
            status_fg: Color::White,
            status_bg: Color::Rgb(0, 122, 204),
            border_active: Color::Rgb(0, 122, 204),
            border_inactive: Color::Rgb(188, 188, 188),
            sidebar_text: Color::Rgb(51, 51, 51),
            chat_text: Color::Rgb(30, 30, 30),
            body_muted: Color::Rgb(71, 71, 71),
            footer_hint: Color::Rgb(106, 115, 125),
            footer_context: Color::Rgb(106, 115, 125),
            input_text: Color::Rgb(30, 30, 30),
            input_border: Color::Rgb(0, 122, 204),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(45, 102, 164),
            toast_success_bg: Color::Rgb(34, 134, 96),
            toast_error_bg: Color::Rgb(173, 58, 58),
            help_fg: Color::Rgb(30, 30, 30),
            help_bg: Color::Rgb(255, 255, 255),
            help_border: Color::Rgb(0, 122, 204),
            modal_fg: Color::Rgb(30, 30, 30),
            modal_bg: Color::Rgb(255, 255, 255),
            modal_border: Color::Rgb(176, 37, 37),
            autocomplete_text: Color::Rgb(30, 30, 30),
            autocomplete_border: Color::Rgb(0, 122, 204),
            you_fg: Color::Rgb(30, 30, 30),
            you_bg: Color::Rgb(227, 242, 253),
            you_header: Color::Rgb(0, 90, 158),
            assistant_fg: Color::Rgb(30, 30, 30),
            assistant_bg: Color::Rgb(245, 245, 245),
            assistant_header: Color::Rgb(97, 97, 97),
            tool_fg: Color::Rgb(73, 53, 23),
            tool_bg: Color::Rgb(255, 244, 230),
            tool_header: Color::Rgb(157, 104, 35),
            system_fg: Color::Rgb(56, 50, 67),
            system_bg: Color::Rgb(240, 236, 248),
            system_header: Color::Rgb(111, 100, 133),
            image_badge_fg: Color::Rgb(31, 71, 51),
            image_badge_bg: Color::Rgb(217, 242, 228),
            code_fg: Color::Rgb(51, 51, 51),
            code_bg: Color::Rgb(246, 248, 250),
            inline_code_fg: Color::Rgb(51, 51, 51),
            inline_code_bg: Color::Rgb(240, 242, 245),
        },
        UiTheme::DaybreakCanvas => ThemePalette {
            status_fg: Color::White,
            status_bg: Color::Rgb(0, 99, 177),
            border_active: Color::Rgb(58, 129, 196),
            border_inactive: Color::Rgb(196, 196, 196),
            sidebar_text: Color::Rgb(45, 45, 45),
            chat_text: Color::Rgb(33, 33, 33),
            body_muted: Color::Rgb(85, 85, 85),
            footer_hint: Color::Rgb(110, 120, 130),
            footer_context: Color::Rgb(120, 130, 140),
            input_text: Color::Rgb(33, 33, 33),
            input_border: Color::Rgb(58, 129, 196),
            toast_fg: Color::White,
            toast_info_bg: Color::Rgb(45, 102, 164),
            toast_success_bg: Color::Rgb(34, 134, 96),
            toast_error_bg: Color::Rgb(173, 58, 58),
            help_fg: Color::Rgb(33, 33, 33),
            help_bg: Color::Rgb(252, 252, 252),
            help_border: Color::Rgb(58, 129, 196),
            modal_fg: Color::Rgb(33, 33, 33),
            modal_bg: Color::Rgb(255, 255, 255),
            modal_border: Color::Rgb(184, 77, 77),
            autocomplete_text: Color::Rgb(33, 33, 33),
            autocomplete_border: Color::Rgb(58, 129, 196),
            you_fg: Color::Rgb(33, 33, 33),
            you_bg: Color::Rgb(235, 244, 255),
            you_header: Color::Rgb(27, 85, 152),
            assistant_fg: Color::Rgb(33, 33, 33),
            assistant_bg: Color::Rgb(248, 248, 248),
            assistant_header: Color::Rgb(92, 92, 92),
            tool_fg: Color::Rgb(79, 58, 25),
            tool_bg: Color::Rgb(255, 247, 236),
            tool_header: Color::Rgb(150, 111, 39),
            system_fg: Color::Rgb(63, 53, 76),
            system_bg: Color::Rgb(244, 240, 250),
            system_header: Color::Rgb(118, 103, 138),
            image_badge_fg: Color::Rgb(30, 59, 84),
            image_badge_bg: Color::Rgb(214, 235, 250),
            code_fg: Color::Rgb(55, 55, 55),
            code_bg: Color::Rgb(245, 247, 249),
            inline_code_fg: Color::Rgb(55, 55, 55),
            inline_code_bg: Color::Rgb(237, 241, 245),
        },
    }
}

fn app_background(theme: UiTheme) -> Color {
    if theme.is_light() {
        Color::Rgb(250, 250, 250)
    } else {
        Color::Rgb(17, 21, 27)
    }
}

fn sidebar_background(theme: UiTheme) -> Color {
    if theme.is_light() {
        Color::Rgb(243, 243, 243)
    } else {
        Color::Rgb(22, 27, 34)
    }
}

fn chat_background(theme: UiTheme) -> Color {
    if theme.is_light() {
        Color::Rgb(255, 255, 255)
    } else {
        Color::Rgb(17, 21, 27)
    }
}

fn input_background(theme: UiTheme) -> Color {
    if theme.is_light() {
        Color::Rgb(255, 255, 255)
    } else {
        Color::Rgb(20, 25, 32)
    }
}

fn autocomplete_background(theme: UiTheme) -> Color {
    if theme.is_light() {
        Color::Rgb(255, 255, 255)
    } else {
        Color::Rgb(18, 24, 33)
    }
}

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    if frame.area().width < 2 || frame.area().height < 2 {
        return;
    }
    let palette = palette::theme_palette(app.theme);
    frame.render_widget(
        Block::default().style(Style::default().bg(palette::app_background(app.theme))),
        frame.area(),
    );

    let input_height = input_panel_height(app, frame.area());
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

fn render_status_bar(frame: &mut Frame<'_>, app: &App, area: Rect, palette: ThemePalette) {
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
    let tools_status = if app.tools_state.enabled {
        format!("tools:on/{}", app.tools_state.policy.as_str())
    } else {
        "tools:off".to_string()
    };
    let workspace_path = app
        .workspace_root_path
        .as_deref()
        .unwrap_or("(path not set)");
    let status = format!(
        "{spinner} ws={} | path={} | session={}#{} | msgs={} | model={} | conn={} | imgs={} | {hl_mode} | {} | {} | F1 help",
        app.workspace_name,
        truncate_middle(workspace_path, 36),
        app.active_session_title,
        app.active_session_id,
        app.messages.len(),
        app.model,
        connection,
        app.pending_images.len(),
        tools_status,
        app.status
    );
    frame.render_widget(
        Paragraph::new(status).style(
            Style::default()
                .fg(palette.status_fg)
                .bg(palette.status_bg)
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
}

fn render_models_panel(frame: &mut Frame<'_>, app: &App, area: Rect, palette: ThemePalette) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let selected = app.sidebar_item();
    let mut lines = Vec::new();
    let line_width = area.width.saturating_sub(2) as usize;
    let mut push_sidebar_line = |raw: String, is_selected: bool| {
        lines.push(sidebar_line_with_marquee(
            &raw,
            is_selected,
            line_width,
            app.spinner_frame,
        ));
    };
    for entry in app.sidebar_entries() {
        match entry {
            SidebarItem::SectionHeader(SidebarSection::Models) => {
                let marker = if selected == Some(entry) { ">" } else { " " };
                let fold = if app.models_collapsed { "+" } else { "-" };
                push_sidebar_line(
                    format!("{marker}[{fold}] Models ({})", app.models.len()),
                    selected == Some(entry),
                );
            }
            SidebarItem::ModelGroupHeader(bucket) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                let (label, fold, count) = match bucket {
                    ModelBucket::Cloud => (
                        "Cloud",
                        if app.cloud_models_collapsed { "+" } else { "-" },
                        app.model_count_for_bucket(ModelBucket::Cloud),
                    ),
                    ModelBucket::Local => (
                        "Local",
                        if app.local_models_collapsed { "+" } else { "-" },
                        app.model_count_for_bucket(ModelBucket::Local),
                    ),
                };
                push_sidebar_line(
                    format!("{marker}   [{fold}] {label} ({count})"),
                    is_selected,
                );
            }
            SidebarItem::SectionHeader(SidebarSection::Workspaces) => {
                let marker = if selected == Some(entry) { ">" } else { " " };
                let fold = if app.workspaces_collapsed { "+" } else { "-" };
                push_sidebar_line(
                    format!("{marker}[{fold}] Workspaces ({})", app.workspaces.len()),
                    selected == Some(entry),
                );
            }
            SidebarItem::Model(index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(model) = app.models.get(index) {
                    let active = if model == &app.model { "*" } else { " " };
                    push_sidebar_line(format!("{marker}     {active} {model}"), is_selected);
                }
            }
            SidebarItem::WorkspaceCreate => {
                let marker = if selected == Some(entry) { ">" } else { " " };
                push_sidebar_line(format!("{marker} + New Workspace"), selected == Some(entry));
            }
            SidebarItem::Workspace(index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(workspace) = app.workspaces.get(index) {
                    let active = if workspace.name == app.workspace_name {
                        "*"
                    } else {
                        " "
                    };
                    let fold = if app.collapsed_workspace_ids.contains(&workspace.id) {
                        "+"
                    } else {
                        "-"
                    };
                    push_sidebar_line(
                        format!("{marker} {active} [{fold}] {}", workspace.name),
                        is_selected,
                    );
                }
            }
            SidebarItem::WorkspaceSessionsLabel(workspace_index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(workspace) = app.workspaces.get(workspace_index) {
                    push_sidebar_line(
                        format!("{marker}   Sessions ({})", workspace.sessions.len()),
                        is_selected,
                    );
                }
            }
            SidebarItem::WorkspaceSession(workspace_index, session_index) => {
                let is_selected = selected == Some(entry);
                let marker = if is_selected { ">" } else { " " };
                if let Some(session) = app
                    .workspaces
                    .get(workspace_index)
                    .and_then(|workspace| workspace.sessions.get(session_index))
                {
                    let active = if session.id == app.active_session_id {
                        "*"
                    } else {
                        " "
                    };
                    push_sidebar_line(
                        format!("{marker}     {active} Session: {}", session.title),
                        is_selected,
                    );
                }
            }
        }
    }

    let border_style = if app.focus == FocusPane::Workspaces {
        Style::default()
            .fg(palette.border_active)
            .bg(sidebar_background(app.theme))
    } else {
        Style::default()
            .fg(palette.border_inactive)
            .bg(sidebar_background(app.theme))
    };

    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .style(
                Style::default()
                    .fg(palette.sidebar_text)
                    .bg(sidebar_background(app.theme)),
            )
            .block(
                Block::default()
                    .title("Sidebar")
                    .borders(Borders::ALL)
                    .style(border_style),
            ),
        area,
    );
}

fn render_chat_panel(frame: &mut Frame<'_>, app: &App, area: Rect, palette: ThemePalette) {
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
    let text = render_chat_text(app, palette);
    let line_count = text.lines.len() as u16;
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
        let welcome_lines = vec![
            Line::from(Span::styled(
                "======================================================",
                Style::default()
                    .fg(palette.border_active)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "        OLLY",
                Style::default()
                    .fg(palette.border_active)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "======================================================",
                Style::default()
                    .fg(palette.border_active)
                    .add_modifier(Modifier::BOLD),
            )),
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
        ];
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
        return;
    }

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let viewport_height = split[0].height;
    let max_scroll = line_count.saturating_sub(viewport_height);
    app.update_chat_max_scroll_hint(max_scroll);
    let scroll_y = calculate_scroll_position(app.chat_scroll, max_scroll);
    frame.render_widget(
        Paragraph::new(text)
            .style(
                Style::default()
                    .fg(palette.chat_text)
                    .bg(chat_background(app.theme)),
            )
            .scroll((scroll_y, 0))
            .wrap(Wrap { trim: false }),
        split[0],
    );

    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(split[1]);

    let action_hint = sidebar_action_hint(app);
    frame.render_widget(
        Paragraph::new(truncate_end(
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
        truncate_middle(path, 28)
    );
    frame.render_widget(
        Paragraph::new(truncate_middle(
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

fn render_input_panel(frame: &mut Frame<'_>, app: &App, area: Rect, palette: ThemePalette) {
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

    if app.focus == FocusPane::Chat || app.is_slash_command_input() {
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

fn input_panel_height(app: &App, frame_area: Rect) -> u16 {
    let min_total_height = 3;
    let max_content_lines = 6_usize;
    let inner_width = frame_area.width.saturating_sub(2).max(1);
    let visual_lines = estimate_input_visual_lines(&app.input, inner_width);
    let desired_total = visual_lines.min(max_content_lines) as u16 + 2;

    // Reserve room for status + at least one body row.
    let max_allowed = frame_area.height.saturating_sub(2).max(min_total_height);
    desired_total.clamp(min_total_height, max_allowed)
}

fn estimate_input_visual_lines(input: &str, inner_width: u16) -> usize {
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

fn input_cursor_visual_position(
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

fn input_cursor_screen_position(
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

fn render_autocomplete_popup(
    frame: &mut Frame<'_>,
    app: &App,
    input_area: Rect,
    palette: ThemePalette,
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

fn render_chat_text(app: &App, palette: ThemePalette) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();

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
                        .fg(palette.image_badge_fg)
                        .bg(palette.image_badge_bg)
                        .add_modifier(Modifier::BOLD),
                )])
                .alignment(align),
            );
        }

        let message_lines = markdown::markdown_lines(
            &message.content,
            base_style,
            app.syntax_highlighting,
            palette,
        );
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

fn pad_line(mut spans: Vec<Span<'static>>, style: Style) -> Vec<Span<'static>> {
    let mut padded = Vec::with_capacity(spans.len() + 2);
    padded.push(Span::styled(" ".to_string(), style));
    padded.append(&mut spans);
    padded.push(Span::styled(" ".to_string(), style));
    padded
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

fn token_label(value: Option<u32>) -> String {
    value.map(format_tokens).unwrap_or_else(|| "?".to_string())
}

fn format_tokens(value: u32) -> String {
    if value >= 1_000_000 {
        format!("{:.1}m", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

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
    use super::{
        calculate_scroll_position, estimate_input_visual_lines, input_cursor_screen_position,
        input_cursor_visual_position, input_panel_height, scroll_window, sidebar_action_hint,
        sidebar_line_with_marquee, truncate_end, truncate_middle,
    };
    use crate::app::{App, ModelBucket, SessionEntry, SidebarItem, WorkspaceEntry};
    use ratatui::layout::Rect;

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
}
