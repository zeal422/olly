use super::helpers::*;
use crate::app::next_autocomplete_value;

#[test]
fn autocomplete_promotes_mod_to_model_command() {
    let models = vec![String::from("llama3.2:latest")];
    assert_eq!(
        autocomplete_suggestions("/mod", &models, &sample_workspaces(), &sample_sessions()),
        vec![String::from("/model ")]
    );
}

#[test]
fn autocomplete_includes_themes_command() {
    let models = vec![String::from("llama3.2:latest")];
    assert_eq!(
        autocomplete_suggestions("/the", &models, &sample_workspaces(), &sample_sessions()),
        vec![String::from("/themes ")]
    );
}

#[test]
fn autocomplete_fills_model_name_by_prefix() {
    let models = vec![
        String::from("devstral-2:123b-cloud"),
        String::from("llama3.2:latest"),
    ];
    assert_eq!(
        autocomplete_suggestions(
            "/model dev",
            &models,
            &sample_workspaces(),
            &sample_sessions(),
        ),
        vec![String::from("/model devstral-2:123b-cloud")]
    );
}

#[test]
fn autocomplete_expands_models_subcommands() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/models ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/models refresh")));
    assert!(suggestions.contains(&String::from("/models add ")));
    assert!(suggestions.contains(&String::from("/models remove ")));
}

#[test]
fn autocomplete_returns_empty_for_non_command_input() {
    let models = vec![String::from("llama3.2:latest")];
    assert!(
        autocomplete_suggestions("hello", &models, &sample_workspaces(), &sample_sessions())
            .is_empty()
    );
}

#[test]
fn autocomplete_cycles_to_next_option() {
    let suggestions = vec![String::from("/model one"), String::from("/model two")];
    assert_eq!(
        next_autocomplete_value("/model one", &suggestions),
        Some(String::from("/model two"))
    );
}

#[test]
fn autocomplete_expands_workspace_subcommands() {
    let models = vec![String::from("llama3.2:latest")];
    assert!(autocomplete_suggestions(
        "/workspace ",
        &models,
        &sample_workspaces(),
        &sample_sessions()
    )
    .contains(&String::from("/workspace create ")));
    assert!(autocomplete_suggestions(
        "/workspace ",
        &models,
        &sample_workspaces(),
        &sample_sessions()
    )
    .contains(&String::from("/workspace delete ")));
    assert!(autocomplete_suggestions(
        "/workspace ",
        &models,
        &sample_workspaces(),
        &sample_sessions()
    )
    .contains(&String::from("/workspace rename ")));
}

#[test]
fn autocomplete_workspace_switch_suggests_names_and_ids() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/workspace switch ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/workspace switch 1")));
    assert!(suggestions.contains(&String::from("/workspace switch Client Project")));
}

#[test]
fn autocomplete_workspace_delete_includes_confirm_cancel() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/workspace delete ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/workspace delete")));
    assert!(suggestions.contains(&String::from("/workspace delete --confirm")));
    assert!(suggestions.contains(&String::from("/workspace delete --cancel")));
}

#[test]
fn autocomplete_workspace_create_offers_path_hint() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/workspace create Client",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/workspace create Client --path ")));
}

#[test]
fn autocomplete_workspace_rename_includes_template() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/workspace rename ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/workspace rename ")));
}

#[test]
fn autocomplete_workspace_set_path_suggests_targets() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/workspace set-path ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/workspace set-path 1 --path ")));
    assert!(suggestions.contains(&String::from("/workspace set-path Client Project --path ")));
}

#[test]
fn autocomplete_session_switch_suggests_ids_and_titles() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/session switch ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/session switch 10")));
    assert!(suggestions.contains(&String::from("/session switch Refactor")));
}

#[test]
fn autocomplete_session_subcommands_include_rename() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/session ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/session rename ")));
    assert!(suggestions.contains(&String::from("/session create ")));
    assert!(suggestions.contains(&String::from("/session delete")));
}

#[test]
fn autocomplete_session_rename_suggests_ids_and_titles() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/session rename ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert!(suggestions.contains(&String::from("/session rename 10 ")));
    assert!(suggestions.contains(&String::from("/session rename Refactor --to ")));
}

#[test]
fn autocomplete_session_delete_suggests_single_command() {
    let models = vec![String::from("llama3.2:latest")];
    let suggestions = autocomplete_suggestions(
        "/session delete ",
        &models,
        &sample_workspaces(),
        &sample_sessions(),
    );
    assert_eq!(suggestions, vec![String::from("/session delete")]);
}
