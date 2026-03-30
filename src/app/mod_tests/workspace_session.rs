use super::helpers::*;

#[test]
fn workspace_create_without_name_uses_incremented_default_name() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-create-default-name");
    let create_root = root.join("workspace-create-root");
    fs::create_dir_all(&create_root).expect("create workspace root");
    manager
        .create_workspace("New Workspace", "llama3.2:latest", Some(&create_root))
        .expect("create existing New Workspace");
    app.refresh_workspaces(&manager)
        .expect("refresh workspaces before create");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!("/workspace create --path {}", create_root.display()),
    )
    .expect("workspace create command");

    let names = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .map(|workspace| workspace.name)
        .collect::<Vec<_>>();
    assert!(names.iter().any(|name| name == "New Workspace"));
    assert!(names
        .iter()
        .any(|name| name.starts_with("New Workspace ") && name != "New Workspace"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_rename_rejects_duplicate_name() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-rename-duplicate");
    let taken_root = root.join("taken-workspace-root");
    fs::create_dir_all(&taken_root).expect("create taken workspace root");
    manager
        .create_workspace("Taken Name", "llama3.2:latest", Some(&taken_root))
        .expect("create workspace with taken name");
    app.refresh_workspaces(&manager)
        .expect("refresh workspaces before rename");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/workspace rename Taken Name",
    )
    .expect("workspace rename command");

    let names = manager
        .list_workspaces()
        .expect("list workspaces")
        .into_iter()
        .map(|workspace| workspace.name)
        .collect::<Vec<_>>();
    assert_eq!(
        names
            .iter()
            .filter(|name| name.as_str() == "Taken Name")
            .count(),
        1
    );
    assert!(names.iter().any(|name| name == "Default Workspace"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_delete_uses_sidebar_selection_and_confirmation_flow() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-delete-selection");
    let delete_root = root.join("delete-workspace-root");
    fs::create_dir_all(&delete_root).expect("create delete workspace root");
    let deletable = manager
        .create_workspace("Delete Me", "llama3.2:latest", Some(&delete_root))
        .expect("create deletable workspace");

    app.refresh_workspaces(&manager)
        .expect("refresh workspaces before delete");
    app.refresh_sessions(&manager, session.workspace_id)
        .expect("refresh sessions before delete");
    select_workspace_in_sidebar(&mut app, deletable.workspace_id);

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/workspace delete",
    )
    .expect("open workspace delete confirmation");
    assert_eq!(
        app.pending_workspace_delete
            .as_ref()
            .map(|pending| pending.id),
        Some(deletable.workspace_id)
    );

    crate::app::runtime::confirm_delete::confirm_pending_workspace_delete(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
    )
    .expect("confirm delete");
    assert!(app.pending_workspace_delete.is_none());
    assert!(!manager
        .list_workspaces()
        .expect("list workspaces after delete")
        .iter()
        .any(|workspace| workspace.id == deletable.workspace_id));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_delete_uses_sidebar_selection_and_confirmation_flow() {
    let (root, manager, mut session, mut app) = setup_app_state("session-delete-selection");
    let target = manager
        .create_session(session.workspace_id, Some("Delete This Session"))
        .expect("create target session");

    app.refresh_workspaces(&manager)
        .expect("refresh workspaces before delete");
    app.refresh_sessions(&manager, session.workspace_id)
        .expect("refresh sessions before delete");
    select_session_in_sidebar(&mut app, session.workspace_id, target);

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session delete",
    )
    .expect("open session delete confirmation");
    assert_eq!(
        app.pending_session_delete
            .as_ref()
            .map(|pending| pending.id),
        Some(target)
    );

    crate::app::runtime::confirm_delete::confirm_pending_session_delete(
        &mut app,
        &manager,
        &mut session,
    )
    .expect("confirm delete");
    assert!(app.pending_session_delete.is_none());
    assert!(!manager
        .list_conversations(session.workspace_id)
        .expect("list sessions after delete")
        .iter()
        .any(|conversation| conversation.id == target));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_delete_targets_sidebar_selection_from_other_workspace() {
    let (root, manager, mut session, mut app) =
        setup_app_state("session-delete-selection-cross-workspace");
    let original_workspace_id = session.workspace_id;
    let original_session_id = session.conversation_id;

    let other_root = root.join("other-workspace-root");
    fs::create_dir_all(&other_root).expect("create other workspace root");
    let other_workspace = manager
        .create_workspace(
            "Delete Target Workspace",
            "llama3.2:latest",
            Some(&other_root),
        )
        .expect("create other workspace");

    app.refresh_workspaces(&manager)
        .expect("refresh workspaces before cross-workspace delete");
    app.refresh_sessions(&manager, session.workspace_id)
        .expect("refresh active workspace sessions before cross-workspace delete");
    select_session_in_sidebar(
        &mut app,
        other_workspace.workspace_id,
        other_workspace.conversation_id,
    );

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session delete",
    )
    .expect("open cross-workspace session delete confirmation");
    assert_eq!(
        app.pending_session_delete
            .as_ref()
            .map(|pending| (pending.workspace_id, pending.id)),
        Some((
            other_workspace.workspace_id,
            other_workspace.conversation_id
        ))
    );

    crate::app::runtime::confirm_delete::confirm_pending_session_delete(
        &mut app,
        &manager,
        &mut session,
    )
    .expect("confirm cross-workspace delete");

    assert_eq!(session.workspace_id, original_workspace_id);
    assert_eq!(session.conversation_id, original_session_id);
    assert_eq!(app.active_workspace_id, original_workspace_id);
    assert_eq!(app.active_session_id, original_session_id);
    assert!(!manager
        .list_conversations(other_workspace.workspace_id)
        .expect("list conversations in other workspace after delete")
        .iter()
        .any(|conversation| conversation.id == other_workspace.conversation_id));
    assert!(!app
        .workspaces
        .iter()
        .find(|workspace| workspace.id == other_workspace.workspace_id)
        .expect("other workspace should remain")
        .sessions
        .iter()
        .any(|entry| entry.id == other_workspace.conversation_id));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_create_sets_active_session_and_loads_empty_messages() {
    let (root, manager, mut session, mut app) = setup_app_state("session-create-active");
    let previous = session.conversation_id;

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session create Planning",
    )
    .expect("session create command");

    assert_ne!(session.conversation_id, previous);
    assert_eq!(app.active_session_id, session.conversation_id);
    assert!(manager
        .load_messages(session.conversation_id)
        .expect("load new session messages")
        .is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_delete_falls_back_then_enters_zero_session_state() {
    let (root, manager, mut session, mut app) = setup_app_state("session-delete-fallback");
    let main_session = session.conversation_id;
    let secondary = manager
        .create_session(session.workspace_id, Some("Secondary"))
        .expect("create secondary session");

    app.refresh_sessions(&manager, session.workspace_id)
        .expect("refresh sessions after create");
    session.conversation_id = secondary;
    app.active_session_id = secondary;

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!("/session delete {secondary}"),
    )
    .expect("open secondary delete modal");
    assert_eq!(
        app.pending_session_delete
            .as_ref()
            .map(|pending| pending.id),
        Some(secondary)
    );
    crate::app::runtime::confirm_delete::confirm_pending_session_delete(
        &mut app,
        &manager,
        &mut session,
    )
    .expect("confirm secondary delete");

    assert_eq!(session.conversation_id, main_session);
    assert_eq!(app.active_session_id, main_session);

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!("/session delete {main_session}"),
    )
    .expect("open final delete modal");
    assert_eq!(
        app.pending_session_delete
            .as_ref()
            .map(|pending| pending.id),
        Some(main_session)
    );
    crate::app::runtime::confirm_delete::confirm_pending_session_delete(
        &mut app,
        &manager,
        &mut session,
    )
    .expect("confirm final delete");

    assert_eq!(session.conversation_id, 0);
    assert_eq!(app.active_session_id, 0);
    assert_eq!(app.active_session_title, "No Session");
    assert_eq!(app.status, "No session active");
    assert!(app.messages.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_rename_supports_id_and_title_with_to_syntax() {
    let (root, manager, mut session, mut app) = setup_app_state("session-rename-paths");
    let target_id = manager
        .create_session(session.workspace_id, Some("Research Notes"))
        .expect("create target session");
    app.refresh_sessions(&manager, session.workspace_id)
        .expect("refresh sessions before rename");
    session.conversation_id = target_id;
    app.active_session_id = target_id;

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!("/session rename {target_id} Renamed By Id"),
    )
    .expect("rename by id");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session rename Renamed By Id --to Final Session Name",
    )
    .expect("rename by title --to");

    let renamed = manager
        .list_conversations(session.workspace_id)
        .expect("list conversations after rename")
        .into_iter()
        .find(|conversation| conversation.id == target_id)
        .expect("target session should exist");
    assert_eq!(renamed.title.as_deref(), Some("Final Session Name"));
    assert_eq!(app.active_session_title, "Final Session Name");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn model_command_handles_exact_prefix_and_unknown_values() {
    let (root, manager, mut session, mut app) = setup_app_state("model-command-cases");
    app.models = vec![
        String::from("qwen2.5-coder:7b"),
        String::from("gemini-3-flash-preview:cloud"),
    ];

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/model gemini-3-flash-preview:cloud",
    )
    .expect("exact model command");
    assert_eq!(app.model, "gemini-3-flash-preview:cloud");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/model qwen2.5",
    )
    .expect("prefix model command");
    assert_eq!(app.model, "qwen2.5-coder:7b");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/model my-custom-model:cloud",
    )
    .expect("unknown model command");
    assert_eq!(app.model, "my-custom-model:cloud");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn url_command_updates_runtime_url() {
    let (root, manager, mut session, mut app) = setup_app_state("url-command-update");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/url http://127.0.0.1:11435",
    )
    .expect("url command");
    assert_eq!(app.url, "http://127.0.0.1:11435");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_create_without_args_shows_usage() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-create-usage");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/workspace create",
    )
    .expect("workspace create usage branch");

    let message = latest_system_message(&app).expect("expected system usage message");
    assert!(message.contains("Usage: /workspace create <name> --path <folder>"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_set_path_missing_path_shows_usage() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-set-path-usage");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/workspace set-path Default Workspace",
    )
    .expect("workspace set-path usage branch");

    assert_eq!(
        latest_system_message(&app),
        Some("Usage: /workspace set-path <id|name> --path <folder>")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_create_supports_quoted_name_and_path() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-create-quoted");
    let quoted_path = root.join("quoted workspace path");
    fs::create_dir_all(&quoted_path).expect("create quoted workspace path");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!(
            "/workspace create \"Quoted Workspace\" --path \"{}\"",
            quoted_path.display()
        ),
    )
    .expect("workspace create with quoted args");

    let created = manager
        .list_workspaces()
        .expect("list workspaces after create")
        .into_iter()
        .find(|workspace| workspace.name == "Quoted Workspace")
        .expect("quoted workspace should exist");
    assert_eq!(session.workspace_id, created.id);
    assert_eq!(app.workspace_name, "Quoted Workspace");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_set_path_supports_quoted_selector_and_path() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-set-path-quoted");
    let quoted_path = root.join("path with spaces");
    fs::create_dir_all(&quoted_path).expect("create quoted path");
    let canonical = fs::canonicalize(&quoted_path)
        .expect("canonicalize quoted path")
        .to_string_lossy()
        .into_owned();

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!(
            "/workspace set-path \"Default Workspace\" --path \"{}\"",
            quoted_path.display()
        ),
    )
    .expect("workspace set-path with quoted args");

    let updated = manager
        .list_workspaces()
        .expect("list workspaces after set-path")
        .into_iter()
        .find(|workspace| workspace.id == session.workspace_id)
        .expect("active workspace should exist");
    assert_eq!(updated.root_path.as_deref(), Some(canonical.as_str()));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn export_markdown_supports_quoted_output_path() {
    let (root, manager, mut session, mut app) = setup_app_state("export-md-quoted");
    manager
        .save_message(
            session.conversation_id,
            "assistant",
            "quoted export payload",
        )
        .expect("save message for export");
    let output = root.join("exports folder").join("conversation file.md");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!("/export md \"{}\"", output.display()),
    )
    .expect("export markdown with quoted path");

    assert!(output.exists());
    let raw = fs::read_to_string(&output).expect("read exported markdown");
    assert!(raw.contains("quoted export payload"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_rename_supports_quoted_titles_with_to_syntax() {
    let (root, manager, mut session, mut app) = setup_app_state("session-rename-quoted");
    let target_id = manager
        .create_session(session.workspace_id, Some("Old Multi Word Title"))
        .expect("create target session");
    app.refresh_sessions(&manager, session.workspace_id)
        .expect("refresh sessions");
    session.conversation_id = target_id;
    app.active_session_id = target_id;

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session rename \"Old Multi Word Title\" --to \"New Multi Word Title\"",
    )
    .expect("session rename with quoted titles");

    let renamed = manager
        .list_conversations(session.workspace_id)
        .expect("list sessions after rename")
        .into_iter()
        .find(|entry| entry.id == target_id)
        .expect("target session should exist");
    assert_eq!(renamed.title.as_deref(), Some("New Multi Word Title"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_switch_unknown_keeps_active_workspace() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-switch-unknown");
    let before_id = session.workspace_id;
    let before_name = app.workspace_name.clone();

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/workspace switch Missing Workspace",
    )
    .expect("workspace switch unknown branch");

    assert_eq!(session.workspace_id, before_id);
    assert_eq!(app.active_workspace_id, before_id);
    assert_eq!(app.workspace_name, before_name);
    let message = latest_system_message(&app).expect("expected not-found message");
    assert!(message.contains("Workspace not found: Missing Workspace"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_switch_unknown_keeps_active_session() {
    let (root, manager, mut session, mut app) = setup_app_state("session-switch-unknown");
    let before_id = session.conversation_id;

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session switch 999999",
    )
    .expect("session switch unknown branch");

    assert_eq!(session.conversation_id, before_id);
    assert_eq!(app.active_session_id, before_id);
    let message = latest_system_message(&app).expect("expected not-found message");
    assert!(message.contains("Session not found: 999999"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_rename_invalid_syntax_shows_usage() {
    let (root, manager, mut session, mut app) = setup_app_state("session-rename-usage");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session rename onlyone",
    )
    .expect("session rename usage branch");

    let message = latest_system_message(&app).expect("expected usage message");
    assert!(message.contains("Usage: /session rename <id> <new title>"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_delete_unknown_id_does_not_mutate_active_session() {
    let (root, manager, mut session, mut app) = setup_app_state("session-delete-unknown");
    let before_id = session.conversation_id;

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session delete 777777",
    )
    .expect("session delete unknown branch");

    assert_eq!(session.conversation_id, before_id);
    assert_eq!(app.active_session_id, before_id);
    let message = latest_system_message(&app).expect("expected not-found message");
    assert!(message.contains("Session not found: 777777"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_delete_without_selection_and_without_active_shows_fallback_message() {
    let (root, manager, mut session, mut app) = setup_app_state("session-delete-fallback-msg");
    app.active_session_id = 0;
    session.workspace_id = 0;
    app.focus = FocusPane::Chat;

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/session delete",
    )
    .expect("session delete fallback branch");

    let message = latest_system_message(&app).expect("expected fallback system message");
    assert!(message.contains("No active workspace."));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_delete_unknown_selector_shows_not_found_message() {
    let (root, manager, mut session, mut app) = setup_app_state("workspace-delete-not-found");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/workspace delete missing-workspace",
    )
    .expect("workspace delete unknown selector");

    let message = latest_system_message(&app).expect("expected not-found message");
    assert!(message.contains("Workspace not found for delete selection."));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn export_unknown_format_shows_explicit_error() {
    let (root, manager, mut session, mut app) = setup_app_state("export-unknown-format");
    let export_path = root.join("conversation.txt");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        &format!("/export txt {}", export_path.display()),
    )
    .expect("export unknown format branch");

    assert_eq!(
        latest_system_message(&app),
        Some("Unknown export format. Use json or md.")
    );
    assert!(!export_path.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn model_command_without_arg_shows_known_models_help() {
    let (root, manager, mut session, mut app) = setup_app_state("model-command-help");
    app.models = vec![
        String::from("qwen2.5-coder:7b"),
        String::from("gemini-3-flash-preview:cloud"),
    ];
    app.model = String::from("qwen2.5-coder:7b");

    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/model",
    )
    .expect("model help branch");

    let message = latest_system_message(&app).expect("expected model help message");
    assert!(message.contains("Usage: /model <name>"));
    assert!(message.contains("Known models:"));

    let _ = fs::remove_dir_all(root);
}
