use super::helpers::*;

#[test]
fn pending_workspace_delete_expires_after_tick() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.pending_workspace_delete = Some(crate::app::PendingWorkspaceDelete {
        id: 123,
        name: "Test Workspace".to_string(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(4),
    });
    assert!(app.pending_workspace_delete.is_some());

    // Set to past
    if let Some(pending) = &mut app.pending_workspace_delete {
        pending.expires_at = std::time::Instant::now() - std::time::Duration::from_secs(1);
    }

    app.tick();

    assert!(app.pending_workspace_delete.is_none());
}

#[test]
fn pending_session_delete_expires_after_tick() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.pending_session_delete = Some(crate::app::PendingSessionDelete {
        workspace_id: 789,
        id: 456,
        name: "Test Session".to_string(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(4),
    });
    assert!(app.pending_session_delete.is_some());

    // Set to past
    if let Some(pending) = &mut app.pending_session_delete {
        pending.expires_at = std::time::Instant::now() - std::time::Duration::from_secs(1);
    }

    app.tick();

    assert!(app.pending_session_delete.is_none());
}

#[test]
fn pending_tool_confirmation_expires_after_tick() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );

    app.pending_tool_confirmation = Some(crate::app::PendingToolConfirmation {
        tool_call_id: "call_123".to_string(),
        tool_name: "test_tool".to_string(),
        tool_args: r#"{"arg": "value"}"#.to_string(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(4),
        step: 1,
        total_steps: 3,
    });
    assert!(app.pending_tool_confirmation.is_some());

    // Set to past
    if let Some(pending) = &mut app.pending_tool_confirmation {
        pending.expires_at = std::time::Instant::now() - std::time::Duration::from_secs(1);
    }

    app.tick();

    assert!(app.pending_tool_confirmation.is_none());
}
