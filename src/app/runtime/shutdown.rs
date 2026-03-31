use std::path::Path;

use crossterm::{
    cursor::SetCursorStyle,
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};

use crate::app::{save_ui_state_to_path, App};

pub(crate) fn teardown_terminal(
    terminal: &mut super::AppTerminal,
    app: &App,
    ui_state_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let ui_state = app.snapshot_ui_state();
    disable_raw_mode()?;
    let _ = execute!(terminal.backend_mut(), SetCursorStyle::DefaultUserShape);
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    let _ = save_ui_state_to_path(ui_state_path, &ui_state);
    Ok(())
}
