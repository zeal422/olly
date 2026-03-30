use std::path::Path;

use crossterm::event::{self, Event};

use crate::{
    app::{save_ui_state_to_path, App, UiRuntimeState},
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    default_model: &str,
    client: &reqwest::Client,
    input_bindings: &crate::app::input::InputBindings,
    terminal: &mut super::AppTerminal,
    ui_state_path: &Path,
    last_saved_ui_state: &mut UiRuntimeState,
) -> Result<(), Box<dyn std::error::Error>> {
    while !app.should_quit() {
        app.tick();
        let current_ui_state = app.snapshot_ui_state();
        if current_ui_state != *last_saved_ui_state {
            let _ = save_ui_state_to_path(ui_state_path, &current_ui_state);
            *last_saved_ui_state = current_ui_state;
        }
        terminal.draw(|f| crate::app::tui::draw(f, app))?;
        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    terminal.clear()?;
                    crate::app::platform::hide_console_scrollbars();
                }
                Event::Key(key) => {
                    let action = crate::app::input::resolve_key_event_with_bindings(
                        key,
                        input_bindings,
                        &app.input,
                    );

                    if super::overlays::handle_overlay_action(
                        app,
                        action.clone(),
                        manager,
                        session,
                        default_model,
                    )? {
                        continue;
                    }

                    super::key_actions::handle_action(
                        app,
                        action,
                        manager,
                        session,
                        default_model,
                        client,
                        terminal,
                    )
                    .await?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}
