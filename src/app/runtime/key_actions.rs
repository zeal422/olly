use crate::{
    app::chat_pipeline,
    app::commands::execute::execute_slash_command,
    app::{
        command_may_change_model_context, refresh_model_context_window, App, FocusPane, ToastLevel,
    },
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};
use copypasta::{ClipboardContext, ClipboardProvider};
use crossterm::event::MouseEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollRoute {
    CommandSuggestions,
    Sidebar,
    InputCaret,
    ChatScroll,
}

const MOUSE_WHEEL_SCROLL_STEP: u16 = 3;

fn mouse_to_chat_visual_position(app: &App, mouse: MouseEvent) -> Option<(usize, usize)> {
    let area = app.chat_area?;
    let row = mouse.row;
    let col = mouse.column;

    if row < area.y || row >= area.y + area.height {
        return None;
    }
    if col < area.x || col >= area.x + area.width {
        return None;
    }

    let relative_row = row.saturating_sub(area.y) as usize;
    let col_index = col.saturating_sub(area.x) as usize;

    if app.chat_visual_lines.is_empty() {
        return None;
    }

    // Compute the top offset of the visible window in the visual lines buffer.
    // chat_scroll is the offset from the bottom; max_scroll is total_lines - viewport_height.
    // The visible window starts at: max_scroll - chat_scroll.
    let viewport_height = area.height as usize;
    let max_scroll = app.chat_visual_lines.len().saturating_sub(viewport_height);
    let top_offset = max_scroll.saturating_sub(app.chat_scroll as usize);

    let visual_line = top_offset
        .saturating_add(relative_row)
        .min(app.chat_visual_lines.len() - 1);
    Some((visual_line, col_index))
}

fn resolve_scroll_route(app: &App, amount: u16) -> ScrollRoute {
    if app.is_command_suggestion_mode() {
        ScrollRoute::CommandSuggestions
    } else if app.focus == FocusPane::Workspaces {
        ScrollRoute::Sidebar
    } else if app.focus == FocusPane::Chat && amount == 1 {
        ScrollRoute::InputCaret
    } else {
        ScrollRoute::ChatScroll
    }
}

fn apply_mouse_wheel_scroll(app: &mut App, amount: i16) {
    app.clamp_chat_scroll_to_hint();
    if amount > 0 {
        app.scroll_up(
            amount
                .unsigned_abs()
                .saturating_mul(MOUSE_WHEEL_SCROLL_STEP),
        );
    } else if amount < 0 {
        app.scroll_down(
            amount
                .unsigned_abs()
                .saturating_mul(MOUSE_WHEEL_SCROLL_STEP),
        );
    }
}

pub(crate) async fn handle_action(
    app: &mut App,
    action: crate::app::input::InputAction,
    mouse_event: Option<MouseEvent>,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    default_model: &str,
    client: &reqwest::Client,
    terminal: &mut super::AppTerminal,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        crate::app::input::InputAction::Quit => app.request_quit(),
        crate::app::input::InputAction::ToggleHelp => app.toggle_help(),
        crate::app::input::InputAction::DismissOverlay => {
            if app.is_slash_command_input() && !app.input.trim().is_empty() {
                app.input.clear();
                app.clear_input_editor_state();
                app.reset_command_selection();
            } else {
                app.dismiss_overlay();
            }
        }
        crate::app::input::InputAction::SwitchFocus => app.switch_focus(),
        crate::app::input::InputAction::AutoCompleteCommand => {
            if app.focus == FocusPane::Chat || app.is_slash_command_input() {
                app.autocomplete_command();
            }
        }
        crate::app::input::InputAction::MouseWheel(amount) => apply_mouse_wheel_scroll(app, amount),
        crate::app::input::InputAction::ScrollUp(amount) => {
            match resolve_scroll_route(app, amount) {
                ScrollRoute::CommandSuggestions => app.autocomplete_command_prev(),
                ScrollRoute::Sidebar => app.sidebar_up(amount),
                ScrollRoute::InputCaret => app.move_input_cursor_up(),
                ScrollRoute::ChatScroll => app.scroll_up(amount),
            }
        }
        crate::app::input::InputAction::ScrollDown(amount) => {
            match resolve_scroll_route(app, amount) {
                ScrollRoute::CommandSuggestions => app.autocomplete_command_next(),
                ScrollRoute::Sidebar => app.sidebar_down(amount),
                ScrollRoute::InputCaret => app.move_input_cursor_down(),
                ScrollRoute::ChatScroll => app.scroll_down(amount),
            }
        }
        crate::app::input::InputAction::MoveCursorLeft => {
            if app.focus == FocusPane::Chat
                || (app.focus == FocusPane::Workspaces && app.is_slash_command_input())
            {
                app.move_input_cursor_left();
            }
        }
        crate::app::input::InputAction::MoveCursorRight => {
            if app.focus == FocusPane::Chat
                || (app.focus == FocusPane::Workspaces && app.is_slash_command_input())
            {
                app.move_input_cursor_right();
            }
        }
        crate::app::input::InputAction::Insert(c) => {
            if app.focus == FocusPane::Chat {
                app.insert_input_char(c);
            } else if app.focus == FocusPane::Workspaces {
                let can_start_command = app.input.is_empty() && c == '/';
                let continue_command = app.is_slash_command_input();
                if can_start_command || continue_command {
                    app.insert_input_char(c);
                }
            }
        }
        crate::app::input::InputAction::Backspace => {
            if app.focus == FocusPane::Chat
                || (app.focus == FocusPane::Workspaces && app.is_slash_command_input())
            {
                app.backspace_input_char();
            }
        }
        crate::app::input::InputAction::Paste => {
            if app.focus == FocusPane::Chat
                || (app.focus == FocusPane::Workspaces && app.is_slash_command_input())
            {
                match ClipboardContext::new() {
                    Ok(mut ctx) => {
                        match ctx.get_contents() {
                            Ok(content) => {
                                if !content.is_empty() {
                                    app.insert_input_string(content);
                                }
                            }
                            Err(_) => {
                                // Silently fail clipboard access - no toast needed
                            }
                        }
                    }
                    Err(_) => {
                        // Silently fail clipboard creation - no toast needed
                    }
                }
            }
        }
        crate::app::input::InputAction::NewLine => {
            if app.focus == FocusPane::Workspaces && app.is_slash_command_input() {
                if let Some(prompt) = app.take_input_for_send() {
                    if let Err(err) =
                        execute_slash_command(app, manager, session, default_model, &prompt).await
                    {
                        app.push_system(format!("Command failed: {err}"));
                        app.set_toast(ToastLevel::Error, "Command failed");
                    }
                    if command_may_change_model_context(&prompt) {
                        refresh_model_context_window(app, client).await;
                    }
                }
            } else if app.focus == FocusPane::Workspaces {
                let previous_model = app.model.clone();
                if let Err(err) = app.activate_sidebar_selection(manager, session, default_model) {
                    app.push_system(format!("Sidebar action failed: {err}"));
                    app.set_toast(ToastLevel::Error, "Sidebar action failed");
                }
                if app.model != previous_model {
                    refresh_model_context_window(app, client).await;
                }
            } else if app.input.trim_start().starts_with('/') && !app.input.contains('\n') {
                if let Some(prompt) = app.take_input_for_send() {
                    if let Err(err) =
                        execute_slash_command(app, manager, session, default_model, &prompt).await
                    {
                        app.push_system(format!("Command failed: {err}"));
                        app.set_toast(ToastLevel::Error, "Command failed");
                    }
                    if command_may_change_model_context(&prompt) {
                        refresh_model_context_window(app, client).await;
                    }
                }
            } else {
                app.insert_newline_at_cursor();
            }
        }
        crate::app::input::InputAction::Send => {
            if app.focus != FocusPane::Chat && !app.is_slash_command_input() {
                return Ok(());
            }
            if let Some(prompt) = app.take_input_for_send() {
                if prompt.starts_with('/') {
                    if let Err(err) =
                        execute_slash_command(app, manager, session, default_model, &prompt).await
                    {
                        app.push_system(format!("Command failed: {err}"));
                        app.set_toast(ToastLevel::Error, "Command failed");
                    }
                    if command_may_change_model_context(&prompt) {
                        refresh_model_context_window(app, client).await;
                    }
                    return Ok(());
                }
                chat_pipeline::send_prompt(app, manager, session, client, terminal, prompt).await?;
            }
        }
        crate::app::input::InputAction::Noop => {}
        crate::app::input::InputAction::Copy => {
            if app.focus == FocusPane::Chat || app.is_slash_command_input() {
                handle_copy(app);
            }
        }
        crate::app::input::InputAction::StartSelection => {
            if let Some(mouse) = mouse_event {
                handle_start_selection(app, mouse);
            }
        }
        crate::app::input::InputAction::UpdateSelection => {
            if let Some(mouse) = mouse_event {
                handle_update_selection(app, mouse);
            }
        }
        crate::app::input::InputAction::EndSelection => {
            handle_end_selection(app);
        }
    }

    Ok(())
}

fn handle_copy(app: &mut App) {
    if let (Some(anchor), Some(current)) = (app.chat_selection_anchor, app.chat_selection_current) {
        let text = extract_selection_text(&app.chat_visual_lines, anchor, current);
        let text_len = text.chars().count();
        if !text.is_empty() {
            match ClipboardContext::new() {
                Ok(mut ctx) => {
                    if let Err(e) = ctx.set_contents(text) {
                        app.set_toast(
                            ToastLevel::Error,
                            format!("Failed to copy to clipboard: {e}"),
                        );
                        return;
                    }
                }
                Err(_) => {
                    app.set_toast(ToastLevel::Error, "Failed to copy to clipboard");
                    return;
                }
            }
            app.set_toast(
                ToastLevel::Success,
                format!("Copied {} characters", text_len),
            );
        } else {
            app.set_toast(ToastLevel::Info, "No text selected");
        }
    } else {
        app.set_toast(ToastLevel::Info, "No selection");
    }
    app.clear_chat_selection();
}

fn handle_start_selection(app: &mut App, mouse: MouseEvent) {
    if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
        app.chat_selection_anchor = Some(pos);
        app.chat_selection_current = Some(pos);
        app.is_selecting = true;
    }
}

fn handle_update_selection(app: &mut App, mouse: MouseEvent) {
    if app.is_selecting {
        if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
            app.chat_selection_current = Some(pos);
        }
    }
}

fn handle_end_selection(app: &mut App) {
    app.is_selecting = false;
}

fn extract_selection_text(
    visual_lines: &[String],
    anchor: (usize, usize),
    current: (usize, usize),
) -> String {
    let (mut start_line, mut start_col) = anchor;
    let (mut end_line, mut end_col) = current;

    if start_line > end_line || (start_line == end_line && start_col > end_col) {
        std::mem::swap(&mut start_line, &mut end_line);
        std::mem::swap(&mut start_col, &mut end_col);
    }

    let mut lines: Vec<String> = Vec::new();

    for (i, line) in visual_lines.iter().enumerate() {
        if i < start_line || i > end_line {
            continue;
        }

        let line_chars: Vec<char> = line.chars().collect();
        let line_len = line_chars.len();

        // Determine the selection range on this line (exclusive end)
        let (sel_start, sel_end_excl) = if i == start_line && i == end_line {
            // Single line selection
            let s = start_col.min(line_len);
            let e = (end_col + 1).min(line_len);
            (s, e)
        } else if i == start_line {
            // First line of multi-line: from start_col to end of line
            (start_col.min(line_len), line_len)
        } else if i == end_line {
            // Last line: from start to end_col+1
            (0, (end_col + 1).min(line_len))
        } else {
            // Middle lines: full line
            (0, line_len)
        };

        if sel_start >= sel_end_excl {
            // Empty selection on this line; for middle lines push full line? Actually if no characters selected, we may push empty if it's start/end line, or full line if it's a middle line (shouldn't happen if full line selection).
            if i == start_line && i == end_line {
                lines.push(String::new());
            } else if i == start_line || i == end_line {
                // empty segment on boundary line
                lines.push(String::new());
            } else {
                lines.push(line.clone());
            }
            continue;
        }

        let line_str: String = line_chars[sel_start..sel_end_excl].iter().collect();
        lines.push(line_str);
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        apply_mouse_wheel_scroll, extract_selection_text, mouse_to_chat_visual_position,
        resolve_scroll_route, ScrollRoute,
    };
    use crate::app::{App, FocusPane};

    #[test]
    fn scroll_route_prioritizes_command_suggestions_over_sidebar_focus() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.focus = FocusPane::Workspaces;
        app.set_input_text(String::from("/workspace "));

        assert_eq!(
            resolve_scroll_route(&app, 1),
            ScrollRoute::CommandSuggestions
        );
    }

    #[test]
    fn scroll_route_uses_sidebar_when_workspace_focused_without_command_mode() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.focus = FocusPane::Workspaces;
        app.set_input_text(String::new());

        assert_eq!(resolve_scroll_route(&app, 1), ScrollRoute::Sidebar);
    }

    #[test]
    fn scroll_route_uses_input_caret_for_single_line_chat_navigation() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.focus = FocusPane::Chat;
        app.set_input_text(String::from("line one"));

        assert_eq!(resolve_scroll_route(&app, 1), ScrollRoute::InputCaret);
    }

    #[test]
    fn scroll_route_uses_chat_scroll_for_page_navigation() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.focus = FocusPane::Chat;
        app.set_input_text(String::from("line one"));

        assert_eq!(resolve_scroll_route(&app, 8), ScrollRoute::ChatScroll);
    }

    #[test]
    fn mouse_wheel_scrolls_chat_even_when_sidebar_focused() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.focus = FocusPane::Workspaces;
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 100;

        apply_mouse_wheel_scroll(&mut app, 1);

        assert_eq!(app.chat_scroll, 103);
    }

    #[test]
    fn mouse_wheel_down_clamps_to_auto_scroll_target() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 0;

        apply_mouse_wheel_scroll(&mut app, -1);

        assert_eq!(app.chat_scroll, 0);
    }

    #[test]
    fn scroll_up_from_zero_increases() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 0;

        app.scroll_up(30);

        assert_eq!(app.chat_scroll, 30);
    }

    #[test]
    fn scroll_up_increases_correctly() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 100;

        app.scroll_up(30);

        assert_eq!(app.chat_scroll, 130);
    }

    #[test]
    fn scroll_up_caps_at_max_scroll() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 490;

        app.scroll_up(50);

        assert_eq!(app.chat_scroll, 500);
    }

    #[test]
    fn scroll_down_from_max_decreases() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 500;

        app.scroll_down(30);

        assert_eq!(app.chat_scroll, 470);
    }

    #[test]
    fn scroll_down_decreases_correctly() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 100;

        app.scroll_down(30);

        assert_eq!(app.chat_scroll, 70);
    }

    #[test]
    fn scroll_down_caps_at_zero() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 10;

        app.scroll_down(50);

        assert_eq!(app.chat_scroll, 0);
    }

    #[test]
    fn clamp_chat_scroll_to_hint_reduces_when_above_max() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 600;

        app.clamp_chat_scroll_to_hint();

        assert_eq!(app.chat_scroll, 500);
    }

    #[test]
    fn clamp_chat_scroll_to_hint_keeps_valid_value() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 100;

        app.clamp_chat_scroll_to_hint();

        assert_eq!(app.chat_scroll, 100);
    }

    #[test]
    fn apply_mouse_wheel_scroll_scrolls_chat() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 100;

        apply_mouse_wheel_scroll(&mut app, 1);

        assert_eq!(app.chat_scroll, 103);
    }

    #[test]
    fn apply_mouse_wheel_scroll_down_caps_at_zero() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 0;

        apply_mouse_wheel_scroll(&mut app, -1);

        assert_eq!(app.chat_scroll, 0);
    }

    #[test]
    fn mouse_wheel_scroll_step_applies_multiplier() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 0;

        apply_mouse_wheel_scroll(&mut app, 2);

        assert_eq!(app.chat_scroll, 6);
    }

    #[test]
    fn rapid_mouse_wheel_events_accumulate() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 0;

        apply_mouse_wheel_scroll(&mut app, 1);
        apply_mouse_wheel_scroll(&mut app, 1);
        apply_mouse_wheel_scroll(&mut app, 1);

        assert_eq!(app.chat_scroll, 9);
    }

    #[test]
    fn mouse_wheel_scrolls_chat_in_command_suggestion_mode() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.update_chat_max_scroll_hint(500);
        app.chat_scroll = 100;
        app.set_input_text(String::from("/workspace "));

        apply_mouse_wheel_scroll(&mut app, 1);

        assert_eq!(app.chat_scroll, 103);
    }

    #[test]
    fn mouse_to_chat_visual_position_outside_area_returns_none() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.chat_area = Some(ratatui::layout::Rect::new(0, 0, 80, 20));
        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Moved,
            column: 100,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        assert_eq!(mouse_to_chat_visual_position(&app, mouse), None);
    }

    #[test]
    fn mouse_to_chat_visual_position_inside_area_maps_correctly() {
        use ratatui::layout::Rect;
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        // total visual lines = 20, viewport height = 15 => max_scroll = 5
        app.chat_visual_lines = vec![String::new(); 20];
        // Set scroll to 2 (offset from bottom)
        app.chat_scroll = 2;
        // Compute top_offset = max_scroll - chat_scroll = 5 - 2 = 3
        // So visible lines start at index 3
        app.chat_area = Some(Rect::new(5, 2, 70, 15));
        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Moved,
            column: 10,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        let result = mouse_to_chat_visual_position(&app, mouse);
        // relative_row = row - area.y = 5 - 2 = 3
        // visual_line = top_offset + relative_row = 3 + 3 = 6
        // col = column - area.x = 10 - 5 = 5
        assert_eq!(result, Some((6, 5)));
    }

    #[test]
    fn extract_selection_text_single_line() {
        let lines = vec![String::from("Hello world")];
        let anchor = (0, 6);
        let current = (0, 10); // selects "world" (characters from 6 to 10 inclusive)
        let text = extract_selection_text(&lines, anchor, current);
        assert_eq!(text, "world");
    }

    #[test]
    fn extract_selection_text_multi_line() {
        let lines = vec![
            String::from("First line"),
            String::from("Second line"),
            String::from("Third line"),
        ];
        // Select from after the space (position 6) in first line => "line"
        // Full second line => "Second line"
        // "Thi" from third line (positions 0-2 inclusive => 0..3 exclusive)
        let anchor = (0, 6);
        let current = (2, 2);
        let text = extract_selection_text(&lines, anchor, current);
        assert_eq!(text, "line\nSecond line\nThi");
    }
}
