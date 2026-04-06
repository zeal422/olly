pub mod autocomplete;
pub mod chat_navigation;
pub mod chat_pipeline;
pub mod commands;
pub mod context;
pub mod domain;
pub mod input;
pub mod input_editor;
pub mod messages_and_conversation;
pub mod overlay_toast;
pub mod platform;
pub mod runtime;
pub mod state;
pub mod theme_picker;
pub mod tools;
pub mod tui;

pub use state::App;

// Re-export commonly used types from state module
pub use state::{
    apply_saved_ui_state_with_startup_precedence,
    is_cloud_model_name,
    load_ui_state_from_path,
    save_ui_state_to_path,
    // Types needed by tests
    ConnectionStatus,
    FocusPane,
    Message,
    PendingImage,
    PendingSessionDelete,
    PendingToolConfirmation,
    PendingWorkspaceDelete,
    SessionContextSnapshot,
    SessionEntry,
    ThemePickerState,
    Toast,
    ToastLevel,
    UiRuntimeState,
    UiTheme,
    WorkspaceEntry,
};

use crate::{
    history,
    ollama::{self, chat::ChatToolCall},
    workspace::manager::WorkspaceManager,
};
use std::time::{Duration, Instant};

pub(crate) const COMMAND_SUGGESTIONS: &[&str] = &[
    "/commands",
    "/models",
    "/themes",
    "/session",
    "/search",
    "/export",
    "/workspace",
    "/checkpoint",
    "/attach",
    "/clear-attachments",
    "/model",
    "/url",
    "/tools",
    "/diff",
];

pub(crate) fn autocomplete_suggestions(
    input: &str,
    models: &[String],
    workspaces: &[crate::app::state::WorkspaceEntry],
    sessions: &[crate::app::state::SessionEntry],
) -> Vec<String> {
    autocomplete::core::suggestions(input, models, workspaces, sessions)
}

pub async fn run(
    url_override: Option<String>,
    model_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize terminal FIRST — user sees preloader immediately
    let mut terminal = runtime::startup::init_terminal()?;

    // 2. Compute ui_state_path early for error teardown
    let config = crate::config::AppConfig::load_or_create()?;
    let ui_state_path = crate::app::state::ui_state_path_for_config(&config);

    // 3. Create progress channel for bootstrap status updates
    let (progress_tx, mut progress_rx) =
        tokio::sync::watch::channel(String::from("Initializing..."));

    // 4. Spawn bootstrap in background
    let bootstrap_handle = tokio::spawn(async move {
        let result =
            runtime::startup::bootstrap_with_progress(url_override, model_override, |stage| {
                let _ = progress_tx.send(stage.to_string());
            })
            .await;
        result.map_err(|e| e.to_string())
    });

    // 5. Preloader render loop while bootstrap runs, then keep logo briefly on screen.
    let preloader_started_at = Instant::now();
    let mut bootstrap_finished_at: Option<Instant> = None;
    loop {
        let _ = progress_rx.borrow_and_update();
        let status = progress_rx.borrow().clone();
        terminal.draw(|f| crate::app::tui::render_preloader(f, &status))?;

        if bootstrap_handle.is_finished() && bootstrap_finished_at.is_none() {
            bootstrap_finished_at = Some(Instant::now());
        }

        let min_total_visible = preloader_started_at.elapsed() >= Duration::from_millis(1_400);
        let post_ready_hold = bootstrap_finished_at
            .map(|finished| finished.elapsed() >= Duration::from_millis(300))
            .unwrap_or(false);
        if bootstrap_finished_at.is_some() && min_total_visible && post_ready_hold {
            break;
        }

        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    // 6. Bootstrap complete — get result
    let mut runtime = match bootstrap_handle.await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            let _ = runtime::shutdown::teardown_terminal(
                &mut terminal,
                &App::new(String::new(), String::new()),
                &ui_state_path,
            );
            return Err(e.into());
        }
        Err(e) => {
            let _ = runtime::shutdown::teardown_terminal(
                &mut terminal,
                &App::new(String::new(), String::new()),
                &ui_state_path,
            );
            return Err(Box::new(e));
        }
    };

    // 7. Persist UI state and enter event loop
    let mut last_saved_ui_state =
        runtime::startup::persist_initial_ui_state(&runtime.app, &runtime.ui_state_path);

    let run_result = runtime::event_loop::run(
        &mut runtime.app,
        &runtime.manager,
        &mut runtime.session,
        &runtime.config.defaults.model,
        &runtime.client,
        &runtime.input_bindings,
        &mut terminal,
        &runtime.ui_state_path,
        &mut last_saved_ui_state,
    )
    .await;

    runtime::shutdown::teardown_terminal(&mut terminal, &runtime.app, &runtime.ui_state_path)?;

    run_result
}

pub(crate) fn load_conversation_into_app(
    app: &mut App,
    manager: &WorkspaceManager,
    conversation_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    app.messages.clear();
    app.prompt_tokens_used = None;
    app.last_completion_tokens = None;
    app.peak_prompt_tokens = 0;
    if conversation_id <= 0 {
        return Ok(());
    }
    let persisted_messages = history::load_conversation(manager, conversation_id)?;
    for persisted in persisted_messages {
        if let Some(message) = App::from_storage_role(
            &persisted.role,
            persisted.content,
            persisted.images,
            persisted.tool_calls,
            persisted.timestamp,
        ) {
            app.messages.push(message);
        }
    }
    app.restore_session_context_snapshot_if_match();
    Ok(())
}

pub(crate) fn command_may_change_model_context(prompt: &str) -> bool {
    let trimmed = prompt.trim_start();
    trimmed == "/model"
        || trimmed.starts_with("/model ")
        || trimmed == "/models"
        || trimmed.starts_with("/models ")
        || trimmed == "/url"
        || trimmed.starts_with("/url ")
}

pub(crate) async fn refresh_model_context_window(app: &mut App, client: &reqwest::Client) {
    if let Ok(Some(window)) =
        ollama::models::model_context_window(client, &app.url, &app.model).await
    {
        app.model_context_window = Some(window);
    }
    app.persist_active_session_context_snapshot();
}

pub(crate) fn workspace_help_text() -> String {
    [
        "Workspace commands:",
        "- /workspace list",
        "- /workspace switch <id|name>",
        "- /workspace create <name> --path <folder>",
        "- /workspace create --path <folder> (name defaults to New Workspace)",
        "- /workspace rename <new name> (renames selected/active workspace)",
        "- /workspace delete (opens modal for selected workspace in sidebar)",
        "- /workspace delete <id|name> (optional explicit target)",
        "- Confirm delete modal with Enter, cancel with Esc",
        "- Deleting the last workspace is allowed (you will see the welcome screen)",
        "- /workspace where",
        "- /workspace set-path <id|name> --path <folder>",
        "- /workspace export <path>",
        "- /workspace import <path>",
    ]
    .join("\n")
}

pub(crate) fn models_help_text() -> String {
    [
        "Model commands:",
        "- /models",
        "- /models refresh",
        "- /models add <model-name-or-ollama-library-url> (pull local or enable cloud)",
        "- /models remove <model-name>",
        "- /model <name> (switch active model)",
    ]
    .join("\n")
}

pub(crate) fn session_help_text() -> String {
    [
        "Session commands:",
        "- /session list",
        "- /session new <title>",
        "- /session create <title> (alias of new)",
        "- /session switch <id|title>",
        "- /session rename <id> <new title>",
        "- /session rename <title> --to <new title>",
        "- /session delete (opens modal for selected session in sidebar)",
        "- /session delete <id|title> (optional explicit target)",
        "- Confirm delete modal with Enter, cancel with Esc",
    ]
    .join("\n")
}

pub(crate) fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .map(|chunk| chunk.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
}

pub(crate) fn unquote_command_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.as_bytes()[0];
        let last = trimmed.as_bytes()[trimmed.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
    }
    trimmed.to_string()
}

pub(crate) fn next_workspace_name(existing: &[crate::workspace::Workspace], base: &str) -> String {
    if !existing
        .iter()
        .any(|workspace| workspace.name.eq_ignore_ascii_case(base))
    {
        return base.to_string();
    }

    let mut index = 2_u32;
    loop {
        let candidate = format!("{base} {index}");
        if !existing
            .iter()
            .any(|workspace| workspace.name.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
        index += 1;
    }
}

pub(crate) fn handle_command(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut crate::workspace::WorkspaceSession,
    default_model: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = commands::parse_command(prompt);
    if parsed.is_empty() {
        return Ok(());
    }

    let parts = parsed.parts();
    let route = commands::route_command(parts[0]);

    if commands::handlers::utility::handle(app, route, parts)? {
        return Ok(());
    }
    if commands::handlers::models::handle(app, manager, route, parts)? {
        return Ok(());
    }
    if commands::handlers::misc::handle(app, manager, session, route, parts)? {
        return Ok(());
    }

    match route {
        commands::CommandRoute::Workspace => {
            if commands::handlers::workspace::handle(
                app,
                manager,
                session,
                default_model,
                prompt,
                parts,
            )? {
                return Ok(());
            }
        }
        commands::CommandRoute::Session => {
            if commands::handlers::session::handle(app, manager, session, prompt, parts)? {
                return Ok(());
            }
        }
        commands::CommandRoute::Unknown => {
            app.push_system("Unknown command. Use /commands".to_string());
            return Ok(());
        }
        _ => {
            app.push_system("Unknown command. Use /commands".to_string());
            return Ok(());
        }
    }

    Ok(())
}

pub(crate) fn tool_calls_preview(tool_calls: &[ChatToolCall]) -> Option<String> {
    if tool_calls.is_empty() {
        return None;
    }
    let names = tool_calls
        .iter()
        .filter_map(|call| call.function.as_ref().map(|f| f.name.clone()))
        .collect::<Vec<_>>();
    if names.is_empty() {
        Some(format!("{} tool call(s)", tool_calls.len()))
    } else {
        Some(format!("Tools: {}", names.join(", ")))
    }
}

#[cfg(test)]
fn next_autocomplete_value(current_input: &str, suggestions: &[String]) -> Option<String> {
    if suggestions.is_empty() {
        return None;
    }
    let normalized = current_input.trim();
    if let Some(index) = suggestions
        .iter()
        .position(|candidate| candidate.trim() == normalized)
    {
        let next = (index + 1) % suggestions.len();
        return Some(suggestions[next].clone());
    }
    Some(suggestions[0].clone())
}

#[cfg(test)]
mod mod_tests;
