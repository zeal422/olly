use crate::{
    app::{commands::CommandRoute, models_help_text, App, ToastLevel},
    workspace::manager::WorkspaceManager,
};

pub(crate) fn handle(
    app: &mut App,
    _manager: &WorkspaceManager,
    route: CommandRoute,
    parts: &[&str],
) -> Result<bool, Box<dyn std::error::Error>> {
    match route {
        CommandRoute::Models => {
            if parts.len() > 1 {
                app.push_system(models_help_text());
                return Ok(true);
            }
            if app.models.is_empty() {
                app.push_system(format!("No models cached yet.\n\n{}", models_help_text()));
            } else {
                let lines = app
                    .models
                    .iter()
                    .map(|model| {
                        if model == &app.model {
                            format!("* {model} (active)")
                        } else {
                            format!("- {model}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                app.push_system(format!("Loaded models:\n{lines}\n\n{}", models_help_text()));
            }
        }
        CommandRoute::Model => {
            let model = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if model.trim().is_empty() {
                if app.models.is_empty() {
                    app.push_system(
                        "Usage: /model <name>\nNo models cached yet. Try `/models refresh`."
                            .to_string(),
                    );
                } else {
                    let lines = app
                        .models
                        .iter()
                        .take(20)
                        .map(|entry| {
                            if entry == &app.model {
                                format!("* {entry} (active)")
                            } else {
                                format!("- {entry}")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    app.push_system(format!(
                        "Usage: /model <name>\nCurrent: {}\n\nKnown models:\n{}\n\nTip: `/models refresh` to reload.",
                        app.model, lines
                    ));
                }
            } else {
                let requested = model.trim();
                if let Some(exact) = app.models.iter().find(|entry| *entry == requested).cloned() {
                    app.model = exact;
                    app.set_toast(ToastLevel::Success, "Model switched");
                } else if let Some(prefix_match) = app
                    .models
                    .iter()
                    .find(|entry| entry.starts_with(requested))
                    .cloned()
                {
                    app.model = prefix_match;
                    app.set_toast(
                        ToastLevel::Success,
                        format!("Model switched (prefix match): {}", app.model),
                    );
                } else {
                    app.model = requested.to_string();
                    app.set_toast(
                        ToastLevel::Info,
                        "Model set (not in cache). Try `/models refresh` if needed.",
                    );
                }
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
