pub(super) use super::super::{
    apply_saved_ui_state_with_startup_precedence, autocomplete_suggestions, handle_command,
    load_ui_state_from_path, models_help_text, save_ui_state_to_path, session_help_text,
    workspace_help_text, App, UiRuntimeState,
};
pub(super) use crate::app::domain::{ModelBucket, SidebarItem};
pub(super) use crate::app::state::UiTheme;
pub(super) use crate::app::state::{
    FocusPane, Message, SessionContextSnapshot, SessionEntry, WorkspaceEntry,
};
pub(super) use crate::app::tools::ToolsState;
pub(super) use crate::workspace::{manager::WorkspaceManager, WorkspaceSession};
pub(super) use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn sample_workspaces() -> Vec<WorkspaceEntry> {
    vec![
        WorkspaceEntry {
            id: 1,
            name: String::from("Default Workspace"),
            root_path: Some(String::from("C:/CODING/olly")),
            sessions: Vec::new(),
        },
        WorkspaceEntry {
            id: 2,
            name: String::from("Client Project"),
            root_path: Some(String::from("C:/CODING/client")),
            sessions: Vec::new(),
        },
    ]
}

pub(super) fn sample_sessions() -> Vec<SessionEntry> {
    vec![
        SessionEntry {
            id: 10,
            title: String::from("Main"),
        },
        SessionEntry {
            id: 11,
            title: String::from("Refactor"),
        },
    ]
}

pub(super) fn unique_temp_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "olly-app-tests-{label}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create test root");
    root
}

pub(super) fn setup_app_state(label: &str) -> (PathBuf, WorkspaceManager, WorkspaceSession, App) {
    let root = unique_temp_root(label);
    let db_path = root.join("workspaces.db");
    let manager = WorkspaceManager::open(&db_path).expect("open manager");
    let session = manager
        .bootstrap_default("Default Workspace", "llama3.2:latest")
        .expect("bootstrap default workspace");
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.active_workspace_id = session.workspace_id;
    app.active_session_id = session.conversation_id;
    app.workspace_name = String::from("Default Workspace");
    app.refresh_workspaces(&manager)
        .expect("refresh workspaces after bootstrap");
    app.refresh_sessions(&manager, session.workspace_id)
        .expect("refresh sessions after bootstrap");
    app.sync_active_workspace_label(session.workspace_id);
    app.focus_active_session(session.workspace_id, session.conversation_id);
    (root, manager, session, app)
}

pub(super) fn select_workspace_in_sidebar(app: &mut App, workspace_id: i64) {
    let entries = app.sidebar_entries();
    let target_index = entries
        .iter()
        .position(|entry| match entry {
            SidebarItem::Workspace(index) => app
                .workspaces
                .get(*index)
                .map(|workspace| workspace.id == workspace_id)
                .unwrap_or(false),
            _ => false,
        })
        .expect("workspace should exist in sidebar");
    app.sidebar_index = target_index;
}

pub(super) fn select_session_in_sidebar(app: &mut App, workspace_id: i64, session_id: i64) {
    let entries = app.sidebar_entries();
    let target_index = entries
        .iter()
        .position(|entry| match entry {
            SidebarItem::WorkspaceSession(workspace_index, session_index) => app
                .workspaces
                .get(*workspace_index)
                .and_then(|workspace| {
                    workspace
                        .sessions
                        .get(*session_index)
                        .map(|session| (workspace.id, session.id))
                })
                .map(|(wid, sid)| wid == workspace_id && sid == session_id)
                .unwrap_or(false),
            _ => false,
        })
        .expect("session should exist in sidebar");
    app.sidebar_index = target_index;
}

pub(super) fn latest_system_message(app: &App) -> Option<&str> {
    app.messages
        .iter()
        .rev()
        .find(|message| message.role == "System")
        .map(|message| message.content.as_str())
}
