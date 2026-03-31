use super::helpers::*;

#[test]
fn sidebar_model_selection_switches_active_model() {
    let (root, manager, mut session, mut app) = setup_app_state("model-switch-sidebar");

    // Add some models
    app.models = vec![
        String::from("llama3.2:latest"),
        String::from("qwen3.5:cloud"),
        String::from("devstral:latest"),
    ];

    // Find the index of the second model in sidebar entries
    let entries = app.sidebar_entries();
    let model_idx = entries
        .iter()
        .position(|e| matches!(e, SidebarItem::Model(i) if app.models[*i] == "qwen3.5:cloud"))
        .expect("qwen3.5:cloud should be in sidebar");

    app.sidebar_index = model_idx;
    app.activate_sidebar_selection(&manager, &mut session, "llama3.2:latest")
        .expect("activate model");

    assert_eq!(app.model, "qwen3.5:cloud");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn model_command_updates_model_from_argument() {
    let (root, manager, mut session, mut app) = setup_app_state("model-cmd-switch");

    app.models = vec![
        String::from("llama3.2:latest"),
        String::from("qwen3.5:cloud"),
    ];

    // Execute /model command via handle_command
    handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/model qwen3.5:cloud",
    )
    .expect("execute /model");

    assert_eq!(app.model, "qwen3.5:cloud");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn model_command_unknown_sets_model_with_info_toast() {
    let (root, manager, mut session, mut app) = setup_app_state("model-cmd-unknown");

    app.models = vec![String::from("llama3.2:latest")];

    // Execute /model with a model not in cache
    let result = handle_command(
        &mut app,
        &manager,
        &mut session,
        "llama3.2:latest",
        "/model unknown-model",
    );

    // Command should succeed (handled) but model set to unknown
    assert!(result.is_ok());
    assert!(app.toast.is_some());
    assert_eq!(
        app.toast.as_ref().unwrap().level,
        crate::app::ToastLevel::Info
    );
    assert_eq!(app.model, "unknown-model");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_may_change_model_context_true_for_model() {
    assert!(crate::app::command_may_change_model_context("/model"));
    assert!(crate::app::command_may_change_model_context(
        "/model llama3.2"
    ));
    assert!(crate::app::command_may_change_model_context("/models"));
    assert!(crate::app::command_may_change_model_context("/models list"));
}

#[test]
fn command_may_change_model_context_true_for_url() {
    assert!(crate::app::command_may_change_model_context("/url"));
    assert!(crate::app::command_may_change_model_context(
        "/url http://localhost:11434"
    ));
}
