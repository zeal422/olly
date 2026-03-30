use crate::{
    app::{load_conversation_into_app, App, ToastLevel},
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) fn confirm_pending_workspace_delete(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    default_model: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pending = app.pending_workspace_delete.clone();
    let Some(pending_delete) = pending else {
        return Ok(());
    };

    let workspaces = manager.list_workspaces()?;

    let target = workspaces
        .iter()
        .find(|workspace| workspace.id == pending_delete.id)
        .cloned();
    let Some(target_workspace) = target else {
        app.push_system("Workspace to delete was not found.".to_string());
        app.pending_workspace_delete = None;
        return Ok(());
    };

    manager.delete_workspace(target_workspace.id)?;
    app.pending_workspace_delete = None;
    app.refresh_workspaces(manager)?;

    if target_workspace.id == session.workspace_id {
        if let Some(next_workspace) = app.workspaces.first().cloned() {
            let next_session = manager.open_workspace_session(next_workspace.id, default_model)?;
            session.workspace_id = next_session.workspace_id;
            session.conversation_id = next_session.conversation_id;
            session.preset_id = next_session.preset_id;
            app.active_workspace_id = session.workspace_id;
            app.active_session_id = session.conversation_id;
            app.workspace_name = next_workspace.name;
            app.workspace_root_path = next_workspace.root_path;
            app.refresh_sessions(manager, session.workspace_id)?;
            app.sync_active_workspace_label(session.workspace_id);
            load_conversation_into_app(app, manager, session.conversation_id)?;
            app.focus_active_session(session.workspace_id, session.conversation_id);
            app.set_toast(ToastLevel::Success, "Workspace deleted and switched");
        } else {
            session.workspace_id = 0;
            session.conversation_id = 0;
            session.preset_id = 0;
            app.clear_active_workspace_state();
            app.set_toast(ToastLevel::Success, "Workspace deleted");
        }
    } else {
        app.refresh_sessions(manager, session.workspace_id)?;
        app.sync_active_workspace_label(session.workspace_id);
        app.focus_active_session(session.workspace_id, session.conversation_id);
        app.set_toast(ToastLevel::Success, "Workspace deleted");
    }
    Ok(())
}

pub(crate) fn confirm_pending_session_delete(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
) -> Result<(), Box<dyn std::error::Error>> {
    let pending = app.pending_session_delete.clone();
    let Some(pending_delete) = pending else {
        return Ok(());
    };

    if pending_delete.workspace_id <= 0 {
        app.push_system(format!(
            "Session delete dropped: invalid pending workspace_id={} (pending={pending_delete:?})",
            pending_delete.workspace_id
        ));
        app.pending_session_delete = None;
        return Ok(());
    }

    let sessions = manager.list_conversations(pending_delete.workspace_id)?;
    let target = sessions
        .iter()
        .find(|conversation| conversation.id == pending_delete.id)
        .cloned();
    let Some(target_session) = target else {
        app.pending_session_delete = None;
        app.push_system("Session to delete was not found.".to_string());
        return Ok(());
    };

    manager.delete_session(target_session.id)?;
    app.pending_session_delete = None;

    if pending_delete.workspace_id == session.workspace_id
        && target_session.id == session.conversation_id
    {
        let remaining = manager.list_conversations(session.workspace_id)?;
        if let Some(next_session) = remaining.last() {
            session.conversation_id = next_session.id;
            app.active_session_id = next_session.id;
            app.active_session_title = next_session
                .title
                .clone()
                .unwrap_or_else(|| format!("Session {}", next_session.id));
            load_conversation_into_app(app, manager, session.conversation_id)?;
        } else {
            session.conversation_id = 0;
            app.active_session_id = 0;
            app.active_session_title = String::from("No Session");
            app.messages.clear();
            app.status = String::from("No session active");
        }
    }
    app.refresh_workspaces(manager)?;
    app.sync_active_workspace_label(session.workspace_id);
    app.focus_active_session(session.workspace_id, session.conversation_id);
    app.set_toast(ToastLevel::Success, "Session deleted");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use crate::{
        app::{App, PendingSessionDelete},
        workspace::{manager::WorkspaceManager, WorkspaceSession},
    };

    fn setup_manager(label: &str) -> WorkspaceManager {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "olly-confirm-delete-tests-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create test root");
        let db_path = root.join("workspaces.db");
        WorkspaceManager::open(&db_path).expect("open manager")
    }

    fn latest_system_message(app: &App) -> Option<&str> {
        app.messages
            .iter()
            .rev()
            .find(|message| message.role == "System")
            .map(|message| message.content.as_str())
    }

    #[test]
    fn confirm_pending_session_delete_warns_and_clears_invalid_workspace_id() {
        let manager = setup_manager("invalid-workspace-id");
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        app.pending_session_delete = Some(PendingSessionDelete {
            workspace_id: 0,
            id: 99,
            name: String::from("Session 99"),
            expires_at: Instant::now() + Duration::from_secs(30),
        });
        let mut session = WorkspaceSession {
            workspace_id: 1,
            conversation_id: 1,
            preset_id: 1,
        };

        super::confirm_pending_session_delete(&mut app, &manager, &mut session)
            .expect("confirm handler succeeds");

        assert!(app.pending_session_delete.is_none());
        let diagnostic = latest_system_message(&app).expect("diagnostic system message");
        assert!(diagnostic.contains("invalid pending workspace_id=0"));
        assert!(diagnostic.contains("pending=PendingSessionDelete"));
    }
}
