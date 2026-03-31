use crate::{
    app::{App, ToastLevel},
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) fn handle_overlay_action(
    app: &mut App,
    action: crate::app::input::InputAction,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    default_model: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    if app.show_help {
        match action {
            crate::app::input::InputAction::ToggleHelp
            | crate::app::input::InputAction::DismissOverlay => {
                app.show_help = false;
            }
            crate::app::input::InputAction::Quit => app.request_quit(),
            _ => {}
        }
        return Ok(true);
    }

    if app.pending_workspace_delete.is_some() {
        match action {
            crate::app::input::InputAction::Quit => app.request_quit(),
            crate::app::input::InputAction::DismissOverlay => {
                app.pending_workspace_delete = None;
                app.set_toast(ToastLevel::Info, "Workspace delete canceled");
            }
            crate::app::input::InputAction::NewLine | crate::app::input::InputAction::Send => {
                if let Err(err) = super::confirm_delete::confirm_pending_workspace_delete(
                    app,
                    manager,
                    session,
                    default_model,
                ) {
                    app.push_system(format!("Workspace delete failed: {err}"));
                    app.set_toast(ToastLevel::Error, "Workspace delete failed");
                }
            }
            _ => {}
        }
        return Ok(true);
    }

    if app.pending_session_delete.is_some() {
        match action {
            crate::app::input::InputAction::Quit => app.request_quit(),
            crate::app::input::InputAction::DismissOverlay => {
                app.pending_session_delete = None;
                app.set_toast(ToastLevel::Info, "Session delete canceled");
            }
            crate::app::input::InputAction::NewLine | crate::app::input::InputAction::Send => {
                if let Err(err) =
                    super::confirm_delete::confirm_pending_session_delete(app, manager, session)
                {
                    app.push_system(format!("Session delete failed: {err}"));
                    app.set_toast(ToastLevel::Error, "Session delete failed");
                }
            }
            _ => {}
        }
        return Ok(true);
    }

    if app.theme_picker.is_some() {
        match action {
            crate::app::input::InputAction::Quit => app.request_quit(),
            crate::app::input::InputAction::DismissOverlay => app.theme_picker_cancel(),
            crate::app::input::InputAction::NewLine | crate::app::input::InputAction::Send => {
                app.theme_picker_confirm()
            }
            crate::app::input::InputAction::ScrollUp(_) => app.theme_picker_prev(),
            crate::app::input::InputAction::ScrollDown(_) => app.theme_picker_next(),
            _ => {}
        }
        return Ok(true);
    }

    // Tool confirmation modal handling
    if app.pending_tool_confirmation.is_some() {
        match action {
            crate::app::input::InputAction::Quit => app.request_quit(),
            crate::app::input::InputAction::DismissOverlay => {
                app.pending_tool_confirmation = None;
                app.set_toast(ToastLevel::Info, "Tool execution canceled");
            }
            crate::app::input::InputAction::NewLine | crate::app::input::InputAction::Send => {
                // Confirm and proceed with tool execution
                // The actual execution will be handled by the chat_pipeline
                // Just clear the modal here and let the loop continue
                app.pending_tool_confirmation = None;
            }
            _ => {}
        }
        return Ok(true);
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use crate::{
        app::{input::InputAction, App, PendingSessionDelete},
        workspace::{manager::WorkspaceManager, WorkspaceSession},
    };

    fn setup_manager_and_session(label: &str) -> (WorkspaceManager, WorkspaceSession) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "olly-overlay-tests-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create test root");
        let db_path = root.join("workspaces.db");
        let manager = WorkspaceManager::open(&db_path).expect("open manager");
        let session = manager
            .bootstrap_default("Default Workspace", "llama3.2:latest")
            .expect("bootstrap default workspace");
        (manager, session)
    }

    #[test]
    fn help_overlay_consumes_actions() {
        let (manager, mut session) = setup_manager_and_session("help-overlay");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.show_help = true;
        app.set_input_text(String::from("unchanged"));

        let consumed = super::handle_overlay_action(
            &mut app,
            InputAction::Insert('x'),
            &manager,
            &mut session,
            "llama3.2:latest",
        )
        .expect("overlay handling succeeds");

        assert!(consumed);
        assert_eq!(app.input, "unchanged");
    }

    #[test]
    fn dismiss_session_delete_overlay_clears_modal_state() {
        let (manager, mut session) = setup_manager_and_session("session-delete-overlay");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.pending_session_delete = Some(PendingSessionDelete {
            workspace_id: 1,
            id: 2,
            name: String::from("Session 2"),
            expires_at: Instant::now() + Duration::from_secs(30),
        });

        let consumed = super::handle_overlay_action(
            &mut app,
            InputAction::DismissOverlay,
            &manager,
            &mut session,
            "llama3.2:latest",
        )
        .expect("overlay handling succeeds");

        assert!(consumed);
        assert!(app.pending_session_delete.is_none());
    }

    #[test]
    fn no_overlay_returns_not_consumed() {
        let (manager, mut session) = setup_manager_and_session("no-overlay");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );

        let consumed = super::handle_overlay_action(
            &mut app,
            InputAction::Insert('x'),
            &manager,
            &mut session,
            "llama3.2:latest",
        )
        .expect("overlay handling succeeds");

        assert!(!consumed);
    }

    #[test]
    fn tool_confirmation_overlay_consumes_actions() {
        let (manager, mut session) = setup_manager_and_session("tool-confirmation-overlay");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        use crate::app::PendingToolConfirmation;
        app.pending_tool_confirmation = Some(PendingToolConfirmation {
            tool_call_id: String::from("test-call-id"),
            tool_name: String::from("execute_command"),
            tool_args: String::from("{\"command\": \"git status\"}"),
            expires_at: Instant::now() + Duration::from_secs(30),
            step: 1,
            total_steps: 3,
        });

        // Test that keypress is consumed
        let consumed = super::handle_overlay_action(
            &mut app,
            InputAction::Insert('x'),
            &manager,
            &mut session,
            "llama3.2:latest",
        )
        .expect("overlay handling succeeds");

        assert!(consumed);
        // Modal should still be open (not dismissed by random key)
        assert!(app.pending_tool_confirmation.is_some());
    }

    #[test]
    fn dismiss_tool_confirmation_overlay_clears_modal() {
        let (manager, mut session) = setup_manager_and_session("tool-confirmation-dismiss");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        use crate::app::PendingToolConfirmation;
        app.pending_tool_confirmation = Some(PendingToolConfirmation {
            tool_call_id: String::from("test-call-id"),
            tool_name: String::from("execute_command"),
            tool_args: String::from("{\"command\": \"git status\"}"),
            expires_at: Instant::now() + Duration::from_secs(30),
            step: 1,
            total_steps: 3,
        });

        let consumed = super::handle_overlay_action(
            &mut app,
            InputAction::DismissOverlay,
            &manager,
            &mut session,
            "llama3.2:latest",
        )
        .expect("overlay handling succeeds");

        assert!(consumed);
        assert!(app.pending_tool_confirmation.is_none());
    }

    #[test]
    fn confirm_tool_execution_on_enter() {
        let (manager, mut session) = setup_manager_and_session("tool-confirmation-confirm");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        use crate::app::PendingToolConfirmation;
        app.pending_tool_confirmation = Some(PendingToolConfirmation {
            tool_call_id: String::from("test-call-id"),
            tool_name: String::from("execute_command"),
            tool_args: String::from("{\"command\": \"git status\"}"),
            expires_at: Instant::now() + Duration::from_secs(30),
            step: 1,
            total_steps: 3,
        });

        // Pressing Enter confirms and clears the modal
        let consumed = super::handle_overlay_action(
            &mut app,
            InputAction::NewLine,
            &manager,
            &mut session,
            "llama3.2:latest",
        )
        .expect("overlay handling succeeds");

        assert!(consumed);
        assert!(app.pending_tool_confirmation.is_none());
    }
}
