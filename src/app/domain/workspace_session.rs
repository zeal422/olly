use std::time::{Duration, Instant};

use crate::{
    app::{
        load_conversation_into_app,
        state::{PendingSessionDelete, PendingWorkspaceDelete},
        App,
    },
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) fn apply_workspace_session_transition(
    app: &mut App,
    session: &mut WorkspaceSession,
    next: WorkspaceSession,
    workspace_name: String,
    workspace_root_path: Option<String>,
    manager: &WorkspaceManager,
) -> Result<(), Box<dyn std::error::Error>> {
    session.workspace_id = next.workspace_id;
    session.conversation_id = next.conversation_id;
    session.preset_id = next.preset_id;
    app.active_workspace_id = session.workspace_id;
    app.active_session_id = session.conversation_id;
    app.workspace_name = workspace_name;
    app.workspace_root_path = workspace_root_path;
    load_conversation_into_app(app, manager, session.conversation_id)?;
    app.focus_active_session(session.workspace_id, session.conversation_id);
    Ok(())
}

pub(crate) fn apply_session_switch(
    app: &mut App,
    session: &mut WorkspaceSession,
    conversation_id: i64,
    manager: &WorkspaceManager,
) -> Result<(), Box<dyn std::error::Error>> {
    session.conversation_id = conversation_id;
    app.active_session_id = conversation_id;
    load_conversation_into_app(app, manager, session.conversation_id)?;
    Ok(())
}

pub(crate) fn open_workspace_delete_modal(
    app: &mut App,
    workspace_id: i64,
    workspace_name: String,
) {
    app.pending_session_delete = None;
    app.pending_workspace_delete = Some(PendingWorkspaceDelete {
        id: workspace_id,
        name: workspace_name,
        expires_at: Instant::now() + Duration::from_secs(45),
    });
}

pub(crate) fn open_session_delete_modal(
    app: &mut App,
    workspace_id: i64,
    session_id: i64,
    session_name: String,
) {
    app.pending_workspace_delete = None;
    app.pending_session_delete = Some(PendingSessionDelete {
        workspace_id,
        id: session_id,
        name: session_name,
        expires_at: Instant::now() + Duration::from_secs(45),
    });
}
