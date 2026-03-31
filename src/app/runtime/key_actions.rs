use crate::{
    app::chat_pipeline,
    app::commands::execute::execute_slash_command,
    app::{
        command_may_change_model_context, refresh_model_context_window, App, FocusPane, ToastLevel,
    },
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollRoute {
    CommandSuggestions,
    Sidebar,
    InputCaret,
    ChatScroll,
}

const MOUSE_WHEEL_SCROLL_STEP: u16 = 3;

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
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply_mouse_wheel_scroll, resolve_scroll_route, ScrollRoute};
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
}
