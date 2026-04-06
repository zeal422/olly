use crate::{
    app::chat_pipeline,
    app::commands::execute::execute_slash_command,
    app::{
        command_may_change_model_context, refresh_model_context_window,
        state::{FocusPane, ToastLevel},
        App,
    },
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};
use copypasta::{ClipboardContext, ClipboardProvider};
use crossterm::event::MouseEvent;
use ratatui::layout::Alignment;

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

    let col_usize = col as usize;
    let (line_text, alignment_opt) = &app.chat_visual_lines[visual_line];
    let line_width = line_text.chars().count();
    let area_width = area.width as usize;
    let start_x = match alignment_opt.unwrap_or(Alignment::Left) {
        Alignment::Left => area.x as usize,
        Alignment::Right => area.x as usize + area_width.saturating_sub(line_width),
        Alignment::Center => area.x as usize + area_width.saturating_sub(line_width) / 2,
    };
    let end_x = start_x + line_width;

    // Reject clicks outside the rendered text span (horizontal margins)
    if col_usize < start_x || col_usize >= end_x {
        return None;
    }

    let col_index = col_usize - start_x;

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
        crate::app::input::InputAction::ExtendSelection => {
            if let Some(mouse) = mouse_event {
                handle_extend_selection(app, mouse);
            }
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
    // First, check if click is on a code block
    if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
        if let Some(block) = find_code_block_at_line(&app.code_blocks, pos.0) {
            // Copy code block to clipboard
            match ClipboardContext::new() {
                Ok(mut ctx) => {
                    let lang_label = if block.language.is_empty() {
                        "code".to_string()
                    } else {
                        block.language.clone()
                    };
                    let line_count = block.raw_content.lines().count();
                    if let Err(e) = ctx.set_contents(block.raw_content.clone()) {
                        app.set_toast(ToastLevel::Error, format!("Failed to copy: {e}"));
                    } else {
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Copied {lang_label} code block ({line_count} lines)"),
                        );
                    }
                }
                Err(_) => {
                    app.set_toast(ToastLevel::Error, "Failed to copy to clipboard");
                }
            }
            return; // Don't start selection
        }
    }

    // Normal text selection behavior
    if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
        app.chat_selection_anchor = Some(pos);
        app.chat_selection_current = Some(pos);
        app.is_selecting = true;
    }
}

fn find_code_block_at_line(
    blocks: &[crate::app::state::CodeBlockRange],
    visual_line: usize,
) -> Option<&crate::app::state::CodeBlockRange> {
    blocks
        .iter()
        .find(|b| visual_line >= b.start_visual_line && visual_line <= b.end_visual_line)
}

fn handle_update_selection(app: &mut App, mouse: MouseEvent) {
    if app.is_selecting {
        if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
            app.chat_selection_current = Some(pos);
        }

        // Check if mouse is near chat area edges for auto-scroll
        if let Some(area) = app.chat_area {
            let mouse_row = mouse.row;
            let chat_top = area.y;
            let chat_bottom = area.y + area.height - 1;

            // Edge threshold: 3 rows from top/bottom
            const EDGE_THRESHOLD: u16 = 3;

            if mouse_row <= chat_top + EDGE_THRESHOLD && mouse_row > chat_top {
                // Near top edge - scroll up
                app.auto_scroll_direction = Some(-1);
            } else if mouse_row >= chat_bottom - EDGE_THRESHOLD && mouse_row < chat_bottom {
                // Near bottom edge - scroll down
                app.auto_scroll_direction = Some(1);
            } else {
                // Outside edge zone - stop auto-scroll
                app.auto_scroll_direction = None;
            }
        }
    }
}

fn handle_end_selection(app: &mut App) {
    app.is_selecting = false;
    app.auto_scroll_direction = None;
}

fn handle_extend_selection(app: &mut App, mouse: MouseEvent) {
    if app.chat_selection_anchor.is_some() {
        if let Some(pos) = mouse_to_chat_visual_position(app, mouse) {
            app.chat_selection_current = Some(pos);
        }
    } else {
        handle_start_selection(app, mouse);
    }
}

fn extract_selection_text(
    visual_lines: &[(String, Option<Alignment>)],
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

    for (i, (text, _alignment)) in visual_lines.iter().enumerate() {
        if i < start_line || i > end_line {
            continue;
        }

        let line_chars: Vec<char> = text.chars().collect();
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
                lines.push(text.clone());
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
        apply_mouse_wheel_scroll, extract_selection_text, find_code_block_at_line,
        handle_start_selection, mouse_to_chat_visual_position, resolve_scroll_route, ScrollRoute,
    };
    use crate::app::state::CodeBlockRange;
    use crate::app::{App, FocusPane};
    use ratatui::layout::Alignment;

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
        // Provide dummy visual lines with alignment
        app.chat_visual_lines = vec![(String::new(), Some(Alignment::Left)); 20];
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
        // Use non-empty lines to test hit detection
        let line_text = "Hello world".to_string();
        app.chat_visual_lines = vec![(line_text.clone(), Some(Alignment::Left)); 20];
        // Set scroll to 2 (offset from bottom)
        app.chat_scroll = 2;
        // Compute top_offset = max_scroll - chat_scroll = 5 - 2 = 3
        // So visible lines start at index 3
        app.chat_area = Some(Rect::new(5, 2, 70, 15));
        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Moved,
            column: 10, // area.x=5, text starts at 5, so this hits char index 5 (the space before 'w')
            row: 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        let result = mouse_to_chat_visual_position(&app, mouse);
        // relative_row = row - area.y = 5 - 2 = 3
        // visual_line = top_offset + relative_row = 3 + 3 = 6
        // start_x = area.x = 5; line width = 11; valid range [5, 16)
        // column 10 is within range, so col_index = 10 - 5 = 5
        assert_eq!(result, Some((6, 5)));
    }

    #[test]
    fn mouse_to_chat_visual_position_rejects_margin_click_left_of_text() {
        use ratatui::layout::Rect;
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        // Line with text "Hello" (5 chars)
        let line_text = "Hello".to_string();
        app.chat_visual_lines = vec![(line_text.clone(), Some(Alignment::Left)); 1];
        app.chat_scroll = 0;
        // area.x=10, line starts at 10, valid range [10, 15)
        app.chat_area = Some(Rect::new(10, 0, 80, 10));
        // Click left of text at column 5 (before start_x=10)
        let mouse_left = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Moved,
            column: 5,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        assert_eq!(mouse_to_chat_visual_position(&app, mouse_left), None);

        // Click right of text at column 20 (after end=15)
        let mouse_right = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Moved,
            column: 20,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        assert_eq!(mouse_to_chat_visual_position(&app, mouse_right), None);
    }

    #[test]
    fn extract_selection_text_single_line() {
        let lines = vec![(String::from("Hello world"), Some(Alignment::Left))];
        let anchor = (0, 6);
        let current = (0, 10); // selects "world" (characters from 6 to 10 inclusive)
        let text = extract_selection_text(&lines, anchor, current);
        assert_eq!(text, "world");
    }

    #[test]
    fn extract_selection_text_multi_line() {
        let lines = vec![
            (String::from("First line"), Some(Alignment::Left)),
            (String::from("Second line"), Some(Alignment::Left)),
            (String::from("Third line"), Some(Alignment::Left)),
        ];
        // Select from after the space (position 6) in first line => "line"
        // Full second line => "Second line"
        // "Thi" from third line (positions 0-2 inclusive => 0..3 exclusive)
        let anchor = (0, 6);
        let current = (2, 2);
        let text = extract_selection_text(&lines, anchor, current);
        assert_eq!(text, "line\nSecond line\nThi");
    }

    #[test]
    fn find_code_block_at_line_returns_block_when_inside_range() {
        let blocks = vec![CodeBlockRange {
            start_visual_line: 5,
            end_visual_line: 10,
            raw_content: "code".into(),
            language: "rust".into(),
        }];
        assert!(find_code_block_at_line(&blocks, 7).is_some());
        assert!(find_code_block_at_line(&blocks, 4).is_none());
        assert!(find_code_block_at_line(&blocks, 11).is_none());
    }

    #[test]
    fn find_code_block_at_line_returns_first_match() {
        let blocks = vec![
            CodeBlockRange {
                start_visual_line: 0,
                end_visual_line: 2,
                raw_content: "a".into(),
                language: "py".into(),
            },
            CodeBlockRange {
                start_visual_line: 5,
                end_visual_line: 7,
                raw_content: "b".into(),
                language: "rs".into(),
            },
        ];
        let found = find_code_block_at_line(&blocks, 1).unwrap();
        assert_eq!(found.language, "py");
    }

    #[test]
    fn handle_start_selection_triggers_toast_on_code_block_click() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.chat_area = Some(ratatui::layout::Rect::new(0, 0, 80, 20));
        app.chat_visual_lines = vec![(String::from("some code"), Some(Alignment::Left)); 10];
        app.code_blocks = vec![CodeBlockRange {
            start_visual_line: 2,
            end_visual_line: 5,
            raw_content: "fn main() {}".into(),
            language: "rust".into(),
        }];

        let mouse = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 5, // Fixed: within "some code" text (9 chars wide, area.x = 0)
            row: 3,    // Inside block range
            modifiers: crossterm::event::KeyModifiers::NONE,
        };

        handle_start_selection(&mut app, mouse);

        // Toast should be set (either success or error depending on clipboard availability)
        assert!(app.toast.is_some());
        let toast_msg = app.toast.as_ref().unwrap().message.to_lowercase();
        assert!(
            toast_msg.contains("copied") || toast_msg.contains("failed to copy"),
            "Expected copy toast, got: {}",
            app.toast.as_ref().unwrap().message
        );
    }

    #[test]
    fn find_code_block_at_line_returns_same_block_for_wrapped_lines() {
        let blocks = vec![crate::app::state::CodeBlockRange {
            start_visual_line: 3,
            end_visual_line: 5,
            raw_content: "fn main() {\n    let x = a_long_line_that_wraps;\n}".into(),
            language: "rust".into(),
        }];

        let b3 = find_code_block_at_line(&blocks, 3).unwrap();
        let b4 = find_code_block_at_line(&blocks, 4).unwrap();
        let b5 = find_code_block_at_line(&blocks, 5).unwrap();

        assert!(std::ptr::eq(b3, b4));
        assert!(std::ptr::eq(b4, b5));
        assert_eq!(b3.language, "rust");

        assert!(find_code_block_at_line(&blocks, 2).is_none());
        assert!(find_code_block_at_line(&blocks, 6).is_none());
    }
}
