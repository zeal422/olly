use super::helpers::*;

#[test]
fn ui_runtime_state_roundtrips_for_sidebar_preferences() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.models_collapsed = true;
    app.cloud_models_collapsed = true;
    app.local_models_collapsed = false;
    app.workspaces_collapsed = true;
    app.collapsed_workspace_ids = [4_i64, 2_i64].into_iter().collect();
    app.sidebar_index = 7;

    let state = app.snapshot_ui_state();
    assert_eq!(
        state,
        UiRuntimeState {
            model: Some(String::from("llama3.2:latest")),
            theme: UiTheme::EmeraldDusk,
            models_collapsed: true,
            cloud_models_collapsed: true,
            local_models_collapsed: false,
            workspaces_collapsed: true,
            collapsed_workspace_ids: vec![2, 4],
            sidebar_index: 7,
            session_context: None,
            tools_state: ToolsState::default(),
        }
    );

    let mut next = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    next.apply_ui_state(state);
    assert!(next.models_collapsed);
    assert!(next.cloud_models_collapsed);
    assert!(!next.local_models_collapsed);
    assert!(next.workspaces_collapsed);
    assert!(next.collapsed_workspace_ids.contains(&2));
    assert!(next.collapsed_workspace_ids.contains(&4));
    assert_eq!(next.sidebar_index, 7);
    assert_eq!(next.theme, UiTheme::EmeraldDusk);
    assert_eq!(next.model, "llama3.2:latest");
}

#[test]
fn ui_state_file_save_and_load_roundtrip() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("olly-ui-state-{unique}"));
    let path = root.join("ui_state.json");
    let state = UiRuntimeState {
        model: Some(String::from("qwen3.5:cloud")),
        theme: UiTheme::SakuraNight,
        models_collapsed: true,
        cloud_models_collapsed: false,
        local_models_collapsed: true,
        workspaces_collapsed: false,
        collapsed_workspace_ids: vec![10, 3],
        sidebar_index: 5,
        session_context: Some(SessionContextSnapshot {
            workspace_id: 10,
            session_id: 11,
            model: String::from("qwen3.5:cloud"),
            model_context_window: Some(262_144),
            prompt_tokens_used: Some(4_096),
            last_completion_tokens: Some(128),
            peak_prompt_tokens: 4_096,
        }),
        tools_state: ToolsState::default(),
    };

    save_ui_state_to_path(&path, &state).expect("save ui state");
    let loaded = load_ui_state_from_path(&path).expect("load ui state");
    assert_eq!(loaded, state);

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ui_state_backward_compat_missing_new_fields_deserializes() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("olly-ui-state-compat-{unique}"));
    let path = root.join("ui_state.json");
    fs::create_dir_all(&root).expect("create compat ui-state root");
    let legacy_raw = r#"{
  "theme": "emerald-dusk",
  "models_collapsed": true,
  "cloud_models_collapsed": false,
  "local_models_collapsed": false,
  "workspaces_collapsed": false,
  "collapsed_workspace_ids": [1, 2],
  "sidebar_index": 3
}"#;
    fs::write(&path, legacy_raw).expect("write legacy ui_state json");

    let loaded = load_ui_state_from_path(&path).expect("load legacy ui_state");
    assert_eq!(loaded.model, None);
    assert_eq!(loaded.session_context, None);
    assert_eq!(loaded.theme, UiTheme::EmeraldDusk);
    assert_eq!(loaded.sidebar_index, 3);
    assert_eq!(loaded.collapsed_workspace_ids, vec![1, 2]);

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn startup_model_precedence_cli_override_beats_saved_ui_model() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("qwen3.5:cloud"),
    );
    let saved = UiRuntimeState {
        model: Some(String::from("llama3.2:latest")),
        theme: UiTheme::EmeraldDusk,
        models_collapsed: false,
        cloud_models_collapsed: false,
        local_models_collapsed: false,
        workspaces_collapsed: false,
        collapsed_workspace_ids: Vec::new(),
        sidebar_index: 0,
        session_context: None,
        tools_state: ToolsState::default(),
    };

    apply_saved_ui_state_with_startup_precedence(&mut app, saved, "qwen3.5:cloud", true);

    assert_eq!(app.model, "qwen3.5:cloud");
}

#[test]
fn startup_model_precedence_uses_saved_ui_model_without_cli_override() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    let saved = UiRuntimeState {
        model: Some(String::from("qwen3.5:cloud")),
        theme: UiTheme::EmeraldDusk,
        models_collapsed: false,
        cloud_models_collapsed: false,
        local_models_collapsed: false,
        workspaces_collapsed: false,
        collapsed_workspace_ids: Vec::new(),
        sidebar_index: 0,
        session_context: None,
        tools_state: ToolsState::default(),
    };

    apply_saved_ui_state_with_startup_precedence(&mut app, saved, "llama3.2:latest", false);

    assert_eq!(app.model, "qwen3.5:cloud");
}

#[test]
fn restore_session_context_snapshot_applies_when_model_and_session_match() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("qwen3.5:cloud"),
    );
    let state = UiRuntimeState {
        model: Some(String::from("qwen3.5:cloud")),
        theme: UiTheme::EmeraldDusk,
        models_collapsed: false,
        cloud_models_collapsed: false,
        local_models_collapsed: false,
        workspaces_collapsed: false,
        collapsed_workspace_ids: Vec::new(),
        sidebar_index: 0,
        session_context: Some(SessionContextSnapshot {
            workspace_id: 3,
            session_id: 9,
            model: String::from("qwen3.5:cloud"),
            model_context_window: Some(262_144),
            prompt_tokens_used: Some(3_210),
            last_completion_tokens: Some(78),
            peak_prompt_tokens: 3_210,
        }),
        tools_state: ToolsState::default(),
    };
    app.apply_ui_state(state);
    app.active_workspace_id = 3;
    app.active_session_id = 9;
    app.model_context_window = None;
    app.prompt_tokens_used = None;
    app.last_completion_tokens = None;
    app.peak_prompt_tokens = 0;

    app.restore_session_context_snapshot_if_match();

    assert_eq!(app.model_context_window, Some(262_144));
    assert_eq!(app.prompt_tokens_used, Some(3_210));
    assert_eq!(app.last_completion_tokens, Some(78));
    assert_eq!(app.peak_prompt_tokens, 3_210);
}

#[test]
fn clamp_sidebar_index_clamps_to_last_valid_sidebar_entry() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.sidebar_index = 42;
    app.models_collapsed = true;
    app.cloud_models_collapsed = true;
    app.local_models_collapsed = true;
    app.workspaces_collapsed = true;
    app.workspaces.clear();
    app.models.clear();
    app.clamp_sidebar_index();

    assert_eq!(app.sidebar_entries().len(), 2);
    assert_eq!(app.sidebar_index, 1);
}

#[test]
fn sync_active_session_label_falls_back_to_latest_when_stale() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.sessions = vec![
        SessionEntry {
            id: 21,
            title: String::from("First"),
        },
        SessionEntry {
            id: 22,
            title: String::from("Latest"),
        },
    ];
    app.active_session_id = 999;

    app.sync_active_session_label();

    assert_eq!(app.active_session_id, 22);
    assert_eq!(app.active_session_title, "Latest");
}

#[test]
fn clear_active_workspace_state_resets_all_related_fields() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.active_workspace_id = 5;
    app.workspace_name = String::from("Some Workspace");
    app.workspace_root_path = Some(String::from("C:/tmp/workspace"));
    app.active_session_id = 77;
    app.active_session_title = String::from("Custom Session");
    app.sessions = vec![SessionEntry {
        id: 77,
        title: String::from("Custom Session"),
    }];
    app.messages.push(Message {
        role: String::from("System"),
        content: String::from("stale"),
        timestamp: String::from("00:00"),
        image_count: 0,
        tool_calls_preview: None,
        tool_name: None,
    });
    app.status = String::from("busy");

    app.clear_active_workspace_state();

    assert_eq!(app.active_workspace_id, 0);
    assert_eq!(app.workspace_name, "No Workspace");
    assert_eq!(app.workspace_root_path, None);
    assert_eq!(app.active_session_id, 0);
    assert_eq!(app.active_session_title, "No Session");
    assert!(app.sessions.is_empty());
    assert!(app.messages.is_empty());
    assert_eq!(app.status, "No workspace active");
}

#[test]
fn app_state_views_reflect_current_values() {
    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    app.focus = FocusPane::Workspaces;
    app.show_help = true;
    app.sidebar_index = 3;
    app.active_workspace_id = 11;
    app.active_session_id = 22;
    app.workspace_name = String::from("Refactor WS");
    app.active_session_title = String::from("Main");
    app.workspace_root_path = Some(String::from("C:/CODING/olly"));
    app.model_context_window = Some(262_144);
    app.prompt_tokens_used = Some(1024);
    app.last_completion_tokens = Some(256);
    app.peak_prompt_tokens = 4096;

    let ui = app.ui_state_view();
    assert_eq!(ui.focus, FocusPane::Workspaces);
    assert!(ui.show_help);
    assert_eq!(ui.sidebar_index, 3);

    let session = app.session_state_view();
    assert_eq!(session.active_workspace_id, 11);
    assert_eq!(session.active_session_id, 22);
    assert_eq!(session.workspace_name, "Refactor WS");
    assert_eq!(session.active_session_title, "Main");
    assert_eq!(session.workspace_root_path, Some("C:/CODING/olly"));

    let telemetry = app.telemetry_state_view();
    assert_eq!(telemetry.model_context_window, Some(262_144));
    assert_eq!(telemetry.prompt_tokens_used, Some(1024));
    assert_eq!(telemetry.last_completion_tokens, Some(256));
    assert_eq!(telemetry.peak_prompt_tokens, 4096);
}

#[test]
fn ui_state_backward_compat_missing_tools_state_deserializes() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("olly-ui-state-tools-compat-{unique}"));
    let path = root.join("ui_state.json");
    fs::create_dir_all(&root).expect("create compat ui-state root");
    // Legacy state without tools_state field
    let legacy_raw = r#"{
  "model": "llama3.2:latest",
  "theme": "emerald-dusk",
  "models_collapsed": true,
  "cloud_models_collapsed": false,
  "local_models_collapsed": false,
  "workspaces_collapsed": false,
  "collapsed_workspace_ids": [1, 2],
  "sidebar_index": 3,
  "session_context": null
}"#;
    fs::write(&path, legacy_raw).expect("write legacy ui_state json");

    let loaded = load_ui_state_from_path(&path).expect("load legacy ui_state");
    // ToolsState should use defaults when missing from JSON
    assert!(!loaded.tools_state.enabled); // Default is disabled
    assert_eq!(loaded.tools_state.policy.as_str(), "auto"); // Default policy
    assert!(loaded.tools_state.allowed_tools.is_empty());
    assert!(loaded.tools_state.denied_tools.is_empty());

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ui_state_tools_state_roundtrip() {
    use crate::app::tools::{ToolPolicy, ToolsState};

    let mut app = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    // Configure tools state
    app.tools_state = ToolsState {
        enabled: true,
        policy: ToolPolicy::Confirm,
        allowed_tools: vec!["read_file".to_string(), "list_dir".to_string()],
        denied_tools: vec!["execute_command".to_string()],
    };

    let state = app.snapshot_ui_state();
    assert!(state.tools_state.enabled);
    assert_eq!(state.tools_state.policy, ToolPolicy::Confirm);
    assert_eq!(
        state.tools_state.allowed_tools,
        vec!["read_file", "list_dir"]
    );
    assert_eq!(state.tools_state.denied_tools, vec!["execute_command"]);

    // Apply state to new app
    let mut next = App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    next.apply_ui_state(state);

    assert!(next.tools_state.enabled);
    assert_eq!(next.tools_state.policy, ToolPolicy::Confirm);
    assert_eq!(
        next.tools_state.allowed_tools,
        vec!["read_file", "list_dir"]
    );
    assert_eq!(next.tools_state.denied_tools, vec!["execute_command"]);
}

#[test]
fn ui_state_file_save_and_load_with_tools_state() {
    use crate::app::tools::{ToolPolicy, ToolsState};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("olly-ui-state-tools-{unique}"));
    let path = root.join("ui_state.json");

    let state = UiRuntimeState {
        model: Some(String::from("qwen3.5:cloud")),
        theme: UiTheme::SakuraNight,
        models_collapsed: true,
        cloud_models_collapsed: false,
        local_models_collapsed: true,
        workspaces_collapsed: false,
        collapsed_workspace_ids: vec![10, 3],
        sidebar_index: 5,
        session_context: None,
        tools_state: ToolsState {
            enabled: true,
            policy: ToolPolicy::Confirm,
            allowed_tools: vec!["read_file".to_string()],
            denied_tools: vec!["execute_command".to_string()],
        },
    };

    save_ui_state_to_path(&path, &state).expect("save ui state");
    let loaded = load_ui_state_from_path(&path).expect("load ui state");
    assert!(loaded.tools_state.enabled);
    assert_eq!(loaded.tools_state.policy, ToolPolicy::Confirm);
    assert_eq!(loaded.tools_state.allowed_tools, vec!["read_file"]);
    assert_eq!(loaded.tools_state.denied_tools, vec!["execute_command"]);

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(&root);
}
