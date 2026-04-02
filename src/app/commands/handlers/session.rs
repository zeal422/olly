use chrono::Local;

use crate::{
    app::domain::workspace_session::{apply_session_switch, open_session_delete_modal},
    app::{
        domain::SidebarItem, session_help_text, starts_with_ignore_ascii_case, state::ToastLevel,
        unquote_command_value, App,
    },
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) fn handle(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    prompt: &str,
    parts: &[&str],
) -> Result<bool, Box<dyn std::error::Error>> {
    if session.workspace_id <= 0 {
        app.push_system(
            "No active workspace. Create one from the sidebar (+ New Workspace) or run `/workspace create ...`."
                .to_string(),
        );
        return Ok(true);
    }

    if parts.len() < 2 {
        app.push_system(session_help_text());
        return Ok(true);
    }

    match parts[1] {
        "list" => {
            let sessions = manager.list_conversations(session.workspace_id)?;
            if sessions.is_empty() {
                app.push_system("No sessions found in this workspace.".to_string());
            } else {
                let list = sessions
                    .into_iter()
                    .map(|conversation| {
                        let title = conversation
                            .title
                            .unwrap_or_else(|| format!("Session {}", conversation.id));
                        if conversation.id == session.conversation_id {
                            format!("* {} (id: {}, active)", title, conversation.id)
                        } else {
                            format!("- {} (id: {})", title, conversation.id)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                app.push_system(format!(
                    "Sessions in {}:\n{}\n\nUse `/session new <title>`, `/session switch <id|title>`, `/session rename ...`, or `/session delete`.",
                    app.workspace_name, list
                ));
            }
        }
        "new" | "create" => {
            let raw_title = parts
                .get(2..)
                .map(|v| v.join(" "))
                .unwrap_or_else(|| format!("Session {}", Local::now().format("%H:%M")));
            let normalized_title = raw_title.trim().to_string();
            let title_arg = if normalized_title.is_empty() {
                format!("Session {}", Local::now().format("%H:%M"))
            } else {
                normalized_title
            };
            let conversation_id = manager.create_session(session.workspace_id, Some(&title_arg))?;
            apply_session_switch(app, session, conversation_id, manager)?;
            app.set_toast(ToastLevel::Success, "New session created");
        }
        "switch" => {
            if parts.len() < 3 {
                app.push_system("Usage: /session switch <id|title>".to_string());
                return Ok(true);
            }
            let selector = parts[2..].join(" ");
            let sessions = manager.list_conversations(session.workspace_id)?;
            let target = if let Ok(id) = selector.trim().parse::<i64>() {
                sessions.into_iter().find(|c| c.id == id)
            } else {
                sessions.into_iter().find(|c| {
                    c.title
                        .as_deref()
                        .map(|t| t.eq_ignore_ascii_case(selector.trim()))
                        .unwrap_or(false)
                })
            };
            if let Some(target_session) = target {
                apply_session_switch(app, session, target_session.id, manager)?;
                app.set_toast(ToastLevel::Success, "Session switched");
            } else {
                app.push_system(format!(
                    "Session not found: {}. Use `/session list`.",
                    selector.trim()
                ));
            }
        }
        "delete" => {
            let (target_workspace_id, target, explicit_selector) = if parts.len() >= 3 {
                let selector = parts[2..].join(" ");
                let sessions = manager.list_conversations(session.workspace_id)?;
                let target = if let Ok(id) = selector.trim().parse::<i64>() {
                    sessions.into_iter().find(|entry| entry.id == id)
                } else {
                    sessions.into_iter().find(|entry| {
                        entry
                            .title
                            .as_deref()
                            .map(|title| title.eq_ignore_ascii_case(selector.trim()))
                            .unwrap_or(false)
                    })
                };
                (
                    session.workspace_id,
                    target,
                    Some(selector.trim().to_string()),
                )
            } else {
                let selected_workspace_and_session = match app.sidebar_item() {
                    Some(SidebarItem::WorkspaceSession(workspace_index, session_index)) => {
                        app.workspaces.get(workspace_index).and_then(|workspace| {
                            workspace
                                .sessions
                                .get(session_index)
                                .map(|entry| (workspace.id, entry.id))
                        })
                    }
                    _ => None,
                };
                let selected_or_active = selected_workspace_and_session.or_else(|| {
                    (session.workspace_id > 0 && app.active_session_id > 0)
                        .then_some((session.workspace_id, app.active_session_id))
                });
                let Some((workspace_id, session_id)) = selected_or_active else {
                    app.push_system(
                        "No session selected. Select a session in sidebar, then run `/session delete`."
                            .to_string(),
                    );
                    return Ok(true);
                };
                let sessions = manager.list_conversations(workspace_id)?;
                (
                    workspace_id,
                    sessions.into_iter().find(|entry| entry.id == session_id),
                    None,
                )
            };
            let Some(target_session) = target else {
                if let Some(selector) = explicit_selector {
                    app.push_system(format!(
                        "Session not found: {selector}. Use `/session list`."
                    ));
                } else {
                    app.push_system(
                        "Selected session was not found. Select a session in sidebar, then run `/session delete`."
                            .to_string(),
                    );
                }
                return Ok(true);
            };
            open_session_delete_modal(
                app,
                target_workspace_id,
                target_session.id,
                target_session
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Session {}", target_session.id)),
            );
            app.set_toast(
                ToastLevel::Info,
                "Delete modal opened (Enter confirm, Esc cancel)",
            );
        }
        "rename" => {
            let raw_args = prompt
                .strip_prefix("/session rename")
                .map(str::trim)
                .unwrap_or_default();
            if raw_args.is_empty() {
                app.push_system(
                    "Usage: /session rename <id> <new title>\n   or: /session rename <title> --to <new title>"
                        .to_string(),
                );
                return Ok(true);
            }

            let sessions = manager.list_conversations(session.workspace_id)?;
            if sessions.is_empty() {
                app.push_system("No sessions found in this workspace.".to_string());
                return Ok(true);
            }

            let (selector, new_title) = if let Some((left, right)) = raw_args.split_once(" --to ") {
                (
                    unquote_command_value(left.trim()),
                    unquote_command_value(right.trim()),
                )
            } else if let Some(index) = raw_args.find(char::is_whitespace) {
                let first = raw_args[..index].trim();
                let rest = raw_args[index..].trim();
                if first.parse::<i64>().is_ok() && !rest.is_empty() {
                    (first.to_string(), unquote_command_value(rest))
                } else {
                    let mut titled_sessions = sessions
                        .iter()
                        .filter_map(|entry| {
                            entry.title.as_ref().map(|title| (title.as_str(), entry.id))
                        })
                        .collect::<Vec<_>>();
                    titled_sessions.sort_by_key(|(title, _)| std::cmp::Reverse(title.len()));
                    let resolved =
                        titled_sessions
                            .into_iter()
                            .find_map(|(title, _conversation_id)| {
                                if starts_with_ignore_ascii_case(raw_args, title) {
                                    let suffix = raw_args.get(title.len()..).unwrap_or_default();
                                    let candidate = suffix.trim();
                                    if !candidate.is_empty() {
                                        return Some((
                                            title.to_string(),
                                            unquote_command_value(candidate),
                                        ));
                                    }
                                }
                                None
                            });
                    if let Some(pair) = resolved {
                        pair
                    } else {
                        app.push_system(
                            "Usage: /session rename <id> <new title>\nFor multi-word source titles, use: /session rename <title> --to <new title>"
                                .to_string(),
                        );
                        return Ok(true);
                    }
                }
            } else {
                app.push_system(
                    "Usage: /session rename <id> <new title>\n   or: /session rename <title> --to <new title>"
                        .to_string(),
                );
                return Ok(true);
            };

            if selector.trim().is_empty() || new_title.trim().is_empty() {
                app.push_system(
                    "Usage: /session rename <id> <new title>\n   or: /session rename <title> --to <new title>"
                        .to_string(),
                );
                return Ok(true);
            }

            let target = if let Ok(id) = selector.trim().parse::<i64>() {
                sessions.iter().find(|entry| entry.id == id)
            } else {
                sessions.iter().find(|entry| {
                    entry
                        .title
                        .as_deref()
                        .map(|title| title.eq_ignore_ascii_case(selector.trim()))
                        .unwrap_or(false)
                })
            };

            let Some(target_session) = target else {
                app.push_system(format!(
                    "Session not found: {}. Use `/session list`.",
                    selector.trim()
                ));
                return Ok(true);
            };

            manager.rename_session(target_session.id, new_title.trim())?;
            if target_session.id == session.conversation_id {
                app.active_session_title = new_title.trim().to_string();
            }
            app.set_toast(
                ToastLevel::Success,
                format!("Session renamed to \"{}\"", new_title.trim()),
            );
        }
        _ => app.push_system(session_help_text()),
    }

    Ok(true)
}
