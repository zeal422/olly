use crate::{
    app::{App, ToastLevel},
    ollama,
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) async fn execute_slash_command(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    default_model: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if prompt.trim() == "/themes" {
        app.open_theme_picker();
        return Ok(());
    }

    if prompt == "/models refresh" || prompt == "/model refresh" {
        let client = reqwest::Client::new();
        match ollama::models::list_models(&client, &app.url).await {
            Ok(models) => {
                app.connection_status = crate::app::ConnectionStatus::Online;
                if !models.is_empty() {
                    app.models = models;
                    if !app.models.iter().any(|m| m == &app.model) {
                        app.model = app.models[0].clone();
                    }
                }
                app.set_toast(ToastLevel::Success, "Model list refreshed");
            }
            Err(err) => {
                app.connection_status = crate::app::ConnectionStatus::Offline;
                app.push_system(format!("Model refresh failed: {err}"));
                app.set_toast(ToastLevel::Error, "Model refresh failed");
            }
        }
        return Ok(());
    }

    if let Some(raw_ref) = prompt.strip_prefix("/models add ") {
        let candidate = normalize_model_reference(raw_ref);
        if candidate.is_empty() {
            app.push_system(
                "Usage: /models add <model-name-or-ollama-library-url> (pull local or enable cloud)"
                    .to_string(),
            );
            return Ok(());
        }
        let client = reqwest::Client::new();
        match ollama::models::pull_model(&client, &app.url, &candidate).await {
            Ok(()) => match ollama::models::list_models(&client, &app.url).await {
                Ok(models) => {
                    app.connection_status = crate::app::ConnectionStatus::Online;
                    app.models = models;
                    if !app.models.iter().any(|m| m == &app.model) {
                        if let Some(first) = app.models.first().cloned() {
                            app.model = first;
                        }
                    }
                    app.set_toast(ToastLevel::Success, format!("Model enabled: {candidate}"));
                }
                Err(err) => {
                    app.push_system(format!("Model enabled, but refresh failed: {err}"));
                    app.set_toast(ToastLevel::Info, "Model enabled; refresh failed");
                }
            },
            Err(err) => {
                app.connection_status = crate::app::ConnectionStatus::Offline;
                app.push_system(format!("Model add failed: {err}"));
                app.set_toast(ToastLevel::Error, "Model add failed");
            }
        }
        return Ok(());
    }

    if let Some(raw_ref) = prompt.strip_prefix("/models remove ") {
        let candidate = normalize_model_reference(raw_ref);
        if candidate.is_empty() {
            app.push_system("Usage: /models remove <model-name>".to_string());
            return Ok(());
        }
        let client = reqwest::Client::new();
        match ollama::models::delete_model(&client, &app.url, &candidate).await {
            Ok(()) => match ollama::models::list_models(&client, &app.url).await {
                Ok(models) => {
                    app.connection_status = crate::app::ConnectionStatus::Online;
                    app.models = models;
                    if app.model == candidate {
                        if let Some(first) = app.models.first().cloned() {
                            app.model = first;
                        }
                    }
                    app.set_toast(ToastLevel::Success, format!("Model removed: {candidate}"));
                }
                Err(err) => {
                    app.push_system(format!("Model removed, but refresh failed: {err}"));
                    app.set_toast(ToastLevel::Info, "Model removed; refresh failed");
                }
            },
            Err(err) => {
                app.connection_status = crate::app::ConnectionStatus::Offline;
                app.push_system(format!("Model remove failed: {err}"));
                app.set_toast(ToastLevel::Error, "Model remove failed");
            }
        }
        return Ok(());
    }

    crate::app::handle_command(app, manager, session, default_model, prompt)?;
    if let Err(err) = app.refresh_workspaces(manager) {
        app.push_system(format!("Workspace refresh failed: {err}"));
        app.set_toast(ToastLevel::Error, "Workspace refresh failed");
    } else {
        app.sync_active_workspace_label(session.workspace_id);
    }
    if let Err(err) = app.refresh_sessions(manager, session.workspace_id) {
        app.push_system(format!("Session refresh failed: {err}"));
        app.set_toast(ToastLevel::Error, "Session refresh failed");
    } else {
        app.focus_active_session(session.workspace_id, session.conversation_id);
    }
    Ok(())
}

fn normalize_model_reference(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return String::new();
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    let without_suffix = trimmed
        .split(['?', '#'])
        .next()
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if let Some(index) = without_suffix.find("/library/") {
        let tail = &without_suffix[index + "/library/".len()..];
        if let Some(segment) = tail.split('/').find(|part| !part.is_empty()) {
            return segment.to_string();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::normalize_model_reference;

    #[test]
    fn normalize_model_reference_accepts_plain_model_name() {
        assert_eq!(
            normalize_model_reference("qwen3.5:397b-cloud"),
            "qwen3.5:397b-cloud"
        );
    }

    #[test]
    fn normalize_model_reference_extracts_library_segment_from_url() {
        assert_eq!(
            normalize_model_reference("https://ollama.com/library/qwen3.5:397b-cloud"),
            "qwen3.5:397b-cloud"
        );
    }

    #[test]
    fn normalize_model_reference_returns_empty_for_non_library_url() {
        assert_eq!(
            normalize_model_reference("https://example.com/models/qwen3.5:397b-cloud"),
            ""
        );
    }

    #[test]
    fn normalize_model_reference_returns_empty_for_library_url_without_model_segment() {
        assert_eq!(normalize_model_reference("https://ollama.com/library/"), "");
    }
}
