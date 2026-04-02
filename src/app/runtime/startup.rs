use std::path::Path;

use crossterm::{
    cursor::SetCursorStyle,
    event::EnableMouseCapture,
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    app::{
        apply_saved_ui_state_with_startup_precedence, load_conversation_into_app,
        load_ui_state_from_path, refresh_model_context_window,
        state::{ui_state_path_for_config, ConnectionStatus},
        App, UiRuntimeState,
    },
    config::AppConfig,
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) struct RuntimeBootstrap {
    pub(crate) config: AppConfig,
    pub(crate) ui_state_path: std::path::PathBuf,
    pub(crate) app: App,
    pub(crate) manager: WorkspaceManager,
    pub(crate) session: WorkspaceSession,
    pub(crate) client: reqwest::Client,
    pub(crate) input_bindings: crate::app::input::InputBindings,
}

pub(crate) async fn bootstrap(
    url_override: Option<String>,
    model_override: Option<String>,
) -> Result<RuntimeBootstrap, Box<dyn std::error::Error>> {
    let config = AppConfig::load_or_create()?;
    let ui_state_path = ui_state_path_for_config(&config);
    let url = url_override.unwrap_or_else(|| config.ollama.url.clone());
    let model_was_overridden = model_override.is_some();
    let startup_model = model_override.unwrap_or_else(|| config.defaults.model.clone());
    let mut app = App::new(url, startup_model.clone());
    app.workspace_name = config.defaults.workspace.clone();
    app.syntax_highlighting = config.ui.syntax_highlighting;
    if let Some(saved_ui_state) = load_ui_state_from_path(&ui_state_path) {
        apply_saved_ui_state_with_startup_precedence(
            &mut app,
            saved_ui_state,
            &startup_model,
            model_was_overridden,
        );
    }
    let input_bindings = crate::app::input::InputBindings::from_config(&config.keybindings);

    let manager = WorkspaceManager::open(Path::new(&config.storage.db_path))?;
    let mut session = WorkspaceSession {
        workspace_id: 0,
        conversation_id: 0,
        preset_id: 0,
    };

    app.refresh_workspaces(&manager)?;
    let initial_workspace = app
        .workspaces
        .iter()
        .find(|workspace| workspace.name == config.defaults.workspace)
        .cloned()
        .or_else(|| app.workspaces.first().cloned());
    if let Some(workspace) = initial_workspace {
        let next = manager.open_workspace_session(workspace.id, &config.defaults.model)?;
        session.workspace_id = next.workspace_id;
        session.conversation_id = next.conversation_id;
        session.preset_id = next.preset_id;
        app.active_workspace_id = session.workspace_id;
        app.active_session_id = session.conversation_id;
        load_conversation_into_app(&mut app, &manager, session.conversation_id)?;
        app.refresh_sessions(&manager, session.workspace_id)?;
        app.sync_active_workspace_label(session.workspace_id);
        app.focus_active_session(session.workspace_id, session.conversation_id);
    } else {
        app.clear_active_workspace_state();
    }

    let client = reqwest::Client::new();
    match crate::ollama::models::list_models(&client, &app.url).await {
        Ok(models) => {
            app.connection_status = ConnectionStatus::Online;
            if !models.is_empty() {
                if !models.iter().any(|m| m == &app.model) {
                    app.model = models[0].clone();
                }
                app.models = models;
            }
        }
        Err(err) => {
            app.connection_status = ConnectionStatus::Offline;
            app.set_toast(
                crate::app::state::ToastLevel::Error,
                format!("Model list unavailable: {err}"),
            );
        }
    }

    app.restore_session_context_snapshot_if_match();
    refresh_model_context_window(&mut app, &client).await;

    Ok(RuntimeBootstrap {
        config,
        ui_state_path,
        app,
        manager,
        session,
        client,
        input_bindings,
    })
}

pub(crate) fn init_terminal() -> Result<super::AppTerminal, Box<dyn std::error::Error>> {
    let mut stdout = std::io::stdout();
    crate::app::platform::maximize_console_window();
    crate::app::platform::snap_console_window_to_cell_grid();
    crate::app::platform::hide_console_scrollbars();
    crate::app::platform::sync_console_buffer_to_window(true);
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    crate::app::platform::maximize_console_window();
    crate::app::platform::snap_console_window_to_cell_grid();
    crate::app::platform::hide_console_scrollbars();
    crate::app::platform::sync_console_buffer_to_window(true);
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.autoresize()?;
    terminal.clear()?;
    let _ = execute!(terminal.backend_mut(), SetCursorStyle::DefaultUserShape);
    terminal.show_cursor()?;
    Ok(terminal)
}

pub(crate) fn persist_initial_ui_state(
    app: &crate::app::App,
    ui_state_path: &std::path::Path,
) -> UiRuntimeState {
    let state = app.snapshot_ui_state();
    let _ = crate::app::state::save_ui_state_to_path(ui_state_path, &state);
    state
}
