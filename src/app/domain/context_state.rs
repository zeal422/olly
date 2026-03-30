use crate::{app::App, workspace::manager::WorkspaceManager};

pub(crate) fn refresh_workspaces_and_sync_active(
    app: &mut App,
    manager: &WorkspaceManager,
    active_workspace_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    app.refresh_workspaces(manager)?;
    app.sync_active_workspace_label(active_workspace_id);
    Ok(())
}
