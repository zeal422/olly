use std::path::PathBuf;

use crate::{
    app::{commands::CommandRoute, load_conversation_into_app, state::ToastLevel, App},
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) fn handle(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    route: CommandRoute,
    parts: &[&str],
) -> Result<bool, Box<dyn std::error::Error>> {
    match route {
        CommandRoute::Search => {
            let query = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();
            if query.trim().is_empty() {
                app.push_system("Usage: /search <query>".to_string());
            } else {
                let hits = manager.search_messages(&query)?;
                if hits.is_empty() {
                    app.push_system("No matches found.".to_string());
                } else {
                    let preview = hits
                        .into_iter()
                        .take(8)
                        .map(|h| format!("#{} {}: {}", h.conversation_id, h.role, h.content))
                        .collect::<Vec<_>>()
                        .join("\n");
                    app.push_system(format!("Search results:\n{preview}"));
                }
            }
        }
        CommandRoute::Export => {
            if !app.has_active_session() || session.conversation_id <= 0 {
                app.push_system(
                    "No active session to export. Create or switch to a workspace first."
                        .to_string(),
                );
                return Ok(true);
            }
            if parts.len() < 3 {
                app.push_system("Usage: /export <json|md> <path>".to_string());
            } else {
                let format = parts[1];
                let path = PathBuf::from(crate::app::unquote_command_value(&parts[2..].join(" ")));
                match format {
                    "json" => manager.export_conversation_json(session.conversation_id, &path)?,
                    "md" | "markdown" => {
                        manager.export_conversation_markdown(session.conversation_id, &path)?
                    }
                    _ => {
                        app.push_system("Unknown export format. Use json or md.".to_string());
                        return Ok(true);
                    }
                }
                app.set_toast(
                    ToastLevel::Success,
                    format!("Conversation exported to {}", path.display()),
                );
            }
        }
        CommandRoute::WorkspacesAlias => {
            app.push_system("Use `/workspace list`.".to_string());
        }
        CommandRoute::Checkpoint => {
            if !app.has_active_session() || session.conversation_id <= 0 {
                app.push_system(
                    "No active session for checkpoints. Create or switch to a workspace first."
                        .to_string(),
                );
                return Ok(true);
            }
            if parts.len() < 3 {
                app.push_system("Usage: /checkpoint <save|restore> <name>".to_string());
            } else {
                let sub = parts[1];
                let name = parts[2..].join(" ");
                match sub {
                    "save" => {
                        manager.save_checkpoint(session.conversation_id, &name)?;
                        app.set_toast(ToastLevel::Success, format!("Checkpoint saved: {name}"));
                    }
                    "restore" => {
                        let restored = manager.restore_checkpoint(
                            session.workspace_id,
                            session.conversation_id,
                            &name,
                        )?;
                        session.conversation_id = restored;
                        app.active_session_id = restored;
                        load_conversation_into_app(app, manager, session.conversation_id)?;
                        app.set_toast(
                            ToastLevel::Success,
                            format!("Restored checkpoint into new conversation: {name}"),
                        );
                    }
                    _ => app.push_system("Unknown checkpoint subcommand.".to_string()),
                }
            }
        }
        CommandRoute::Diff => {
            if let Some(ref output) = app.last_tool_output {
                app.push_system(format!("````\n{output}\n````"));
            } else {
                app.push_system(
                    "No tool output available. Run a tool first, then use /diff to view its output.".to_string()
                );
            }
        }
        _ => return Ok(false),
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn setup_test_env() -> (App, WorkspaceManager, WorkspaceSession) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("olly-diff-tests-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let db_path = root.join("workspaces.db");
        let manager = WorkspaceManager::open(&db_path).unwrap();
        let session = manager
            .bootstrap_default("Test WS", "llama3.2:latest")
            .unwrap();
        let app = App::new("http://localhost:11434".into(), "llama3.2:latest".into());
        (app, manager, session)
    }

    #[test]
    fn diff_shows_last_tool_output() {
        let (mut app, manager, mut session) = setup_test_env();
        app.last_tool_output = Some("tool result".into());
        let _ = handle(
            &mut app,
            &manager,
            &mut session,
            CommandRoute::Diff,
            &["/diff"],
        );
        assert!(app.messages.last().unwrap().content.contains("tool result"));
    }

    #[test]
    fn diff_shows_message_when_no_output() {
        let (mut app, manager, mut session) = setup_test_env();
        app.last_tool_output = None;
        let _ = handle(
            &mut app,
            &manager,
            &mut session,
            CommandRoute::Diff,
            &["/diff"],
        );
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("No tool output available"));
    }

    #[test]
    fn diff_shows_fallback_when_output_cleared() {
        let (mut app, manager, mut session) = setup_test_env();
        // Simulate state cleared after a failed/empty tool call (P1 fix)
        app.last_tool_output = None;
        let _ = handle(
            &mut app,
            &manager,
            &mut session,
            CommandRoute::Diff,
            &["/diff"],
        );
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("No tool output available"));
    }
}
