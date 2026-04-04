use std::path::Path;
use std::time::{Duration, Instant};

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
    const TICK_RATE: Duration = Duration::from_millis(80);
    let mut last_tick = Instant::now();

    while !app.should_quit() {
        let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    terminal.clear()?;
                    crate::app::platform::hide_console_scrollbars();
                    app.clear_chat_selection();
                    app.chat_visual_lines_styled.clear();
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
                        None,
                        manager,
                        session,
                        default_model,
                        client,
                        terminal,
                    )
                    .await?;
                }
                Event::Mouse(mouse) => {
                    let action = crate::app::input::resolve_mouse_event(mouse);
                    if action != crate::app::input::InputAction::Noop {
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
                            Some(mouse),
                            manager,
                            session,
                            default_model,
                            client,
                            terminal,
                        )
                        .await?;
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= TICK_RATE {
            app.tick();
            last_tick = Instant::now();
        }

        let current_ui_state = app.snapshot_ui_state();
        if current_ui_state != *last_saved_ui_state {
            let _ = save_ui_state_to_path(ui_state_path, &current_ui_state);
            *last_saved_ui_state = current_ui_state;
        }
        terminal.draw(|f| crate::app::tui::draw(f, app))?;
    }

    Ok(())
}
