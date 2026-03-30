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
    use super::{resolve_scroll_route, ScrollRoute};
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
}
