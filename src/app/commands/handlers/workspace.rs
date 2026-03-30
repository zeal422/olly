use std::path::PathBuf;

use crate::{
    app::domain::{
        context_state::refresh_workspaces_and_sync_active,
        workspace_session::{apply_workspace_session_transition, open_workspace_delete_modal},
    },
    app::{
        next_workspace_name, unquote_command_value, workspace_help_text, App, SidebarItem,
        ToastLevel,
    },
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

pub(crate) fn handle(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    default_model: &str,
    prompt: &str,
    parts: &[&str],
) -> Result<bool, Box<dyn std::error::Error>> {
    if parts.len() < 2 {
        app.push_system(workspace_help_text());
        return Ok(true);
    }

    let sub = parts[1];
    match sub {
        "list" => {
            let workspaces = manager.list_workspaces()?;
            if workspaces.is_empty() {
                app.push_system("No workspaces found.".to_string());
            } else {
                let list = workspaces
                    .into_iter()
                    .map(|workspace| {
                        let path_label = workspace.root_path.as_deref().unwrap_or("(path not set)");
                        if workspace.id == session.workspace_id {
                            format!(
                                "* {} (id: {}, active, path: {})",
                                workspace.name, workspace.id, path_label
                            )
                        } else {
                            format!(
                                "- {} (id: {}, path: {})",
                                workspace.name, workspace.id, path_label
                            )
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                app.push_system(format!(
                    "Workspaces:\n{list}\n\nUse `/workspace switch <id|name>` or `/workspace where`."
                ));
            }
        }
        "switch" => {
            if parts.len() < 3 {
                app.push_system("Usage: /workspace switch <id|name>".to_string());
                return Ok(true);
            }
            let selector = parts[2..].join(" ");
            let workspaces = manager.list_workspaces()?;
            let target = if let Ok(id) = selector.parse::<i64>() {
                workspaces.into_iter().find(|w| w.id == id)
            } else {
                workspaces
                    .into_iter()
                    .find(|w| w.name.eq_ignore_ascii_case(&selector))
            };
            if let Some(workspace) = target {
                let next = manager.open_workspace_session(workspace.id, default_model)?;
                apply_workspace_session_transition(
                    app,
                    session,
                    next,
                    workspace.name,
                    workspace.root_path,
                    manager,
                )?;
                app.set_toast(ToastLevel::Success, "Workspace switched");
            } else {
                app.push_system(format!(
                    "Workspace not found: {selector}. Use `/workspace list`."
                ));
            }
        }
        "create" => {
            let raw_create = prompt
                .strip_prefix("/workspace create")
                .map(str::trim)
                .unwrap_or_default();
            if raw_create.is_empty() {
                app.push_system(
                    "Usage: /workspace create <name> --path <folder>\n   or: /workspace create --path <folder>"
                        .to_string(),
                );
                return Ok(true);
            }

            let (name_part, root_path_raw) =
                if let Some(path_only) = raw_create.strip_prefix("--path ") {
                    ("", Some(path_only.trim()))
                } else if let Some((left, right)) = raw_create.split_once(" --path ") {
                    (left.trim(), Some(right.trim()))
                } else {
                    (raw_create, None)
                };

            let explicit_path = root_path_raw.is_some();
            let root_path = if let Some(path_raw) = root_path_raw {
                let normalized_path = unquote_command_value(path_raw);
                if normalized_path.is_empty() {
                    app.push_system("Workspace path cannot be empty after `--path`.".to_string());
                    return Ok(true);
                }
                PathBuf::from(normalized_path)
            } else {
                std::env::current_dir()?
            };

            let existing_workspaces = manager.list_workspaces()?;
            let mut name = unquote_command_value(name_part);
            if name.is_empty() {
                name = next_workspace_name(&existing_workspaces, "New Workspace");
            }
            if name.is_empty() {
                app.push_system("Workspace name cannot be empty.".to_string());
                return Ok(true);
            }
            let existing = existing_workspaces
                .into_iter()
                .find(|w| w.name.eq_ignore_ascii_case(&name));
            if existing.is_some() {
                app.push_system(format!(
                    "Workspace `{name}` already exists. Use `/workspace switch {name}`."
                ));
                return Ok(true);
            }
            let created = manager.create_workspace(&name, default_model, Some(&root_path))?;
            apply_workspace_session_transition(
                app,
                session,
                created,
                name.clone(),
                Some(root_path.to_string_lossy().into_owned()),
                manager,
            )?;
            app.refresh_workspaces(manager)?;
            app.refresh_sessions(manager, session.workspace_id)?;
            if !explicit_path {
                app.push_system(format!(
                    "No `--path` provided, using current directory: {}",
                    root_path.display()
                ));
            }
            app.set_toast(ToastLevel::Success, format!("Workspace `{name}` created"));
        }
        "rename" => {
            let new_name_raw = prompt
                .strip_prefix("/workspace rename")
                .map(str::trim)
                .unwrap_or_default();
            let new_name = new_name_raw
                .strip_prefix("--to")
                .map(str::trim)
                .unwrap_or(new_name_raw)
                .trim();
            if new_name.is_empty() {
                app.push_system(
                    "Usage: /workspace rename <new name>\nRenames selected workspace in sidebar, otherwise active workspace."
                        .to_string(),
                );
                return Ok(true);
            }

            let workspaces = manager.list_workspaces()?;
            if workspaces.is_empty() {
                app.push_system(
                    "No workspaces found. Create one with `/workspace create --path <folder>`."
                        .to_string(),
                );
                return Ok(true);
            }

            let selected_workspace_id = match app.sidebar_item() {
                Some(SidebarItem::Workspace(index))
                | Some(SidebarItem::WorkspaceSessionsLabel(index))
                | Some(SidebarItem::WorkspaceSession(index, _)) => {
                    app.workspaces.get(index).map(|workspace| workspace.id)
                }
                _ => (session.workspace_id > 0).then_some(session.workspace_id),
            };
            let target = selected_workspace_id
                .and_then(|id| workspaces.iter().find(|workspace| workspace.id == id))
                .cloned();
            let Some(target_workspace) = target else {
                app.push_system(
                    "No workspace selected. Select one in sidebar, then run `/workspace rename <new name>`."
                        .to_string(),
                );
                return Ok(true);
            };

            if target_workspace.name.eq_ignore_ascii_case(new_name) {
                app.set_toast(ToastLevel::Info, "Workspace name unchanged");
                return Ok(true);
            }

            if workspaces.iter().any(|workspace| {
                workspace.id != target_workspace.id && workspace.name.eq_ignore_ascii_case(new_name)
            }) {
                app.push_system(format!(
                    "Workspace `{new_name}` already exists. Choose another name."
                ));
                return Ok(true);
            }

            manager.rename_workspace(target_workspace.id, new_name)?;
            if target_workspace.id == session.workspace_id {
                app.workspace_name = new_name.to_string();
            }
            refresh_workspaces_and_sync_active(app, manager, session.workspace_id)?;
            app.set_toast(
                ToastLevel::Success,
                format!(
                    "Workspace renamed: {} -> {}",
                    target_workspace.name, new_name
                ),
            );
        }
        "delete" => {
            let delete_args = parts.get(2..).unwrap_or(&[]);
            if matches!(delete_args.first(), Some(&"--cancel")) {
                if app.pending_workspace_delete.take().is_some() {
                    app.set_toast(ToastLevel::Info, "Workspace delete canceled");
                } else {
                    app.push_system("No pending workspace delete.".to_string());
                }
                return Ok(true);
            }
            if matches!(delete_args.first(), Some(&"--confirm")) {
                crate::app::runtime::confirm_delete::confirm_pending_workspace_delete(
                    app,
                    manager,
                    session,
                    default_model,
                )?;
                return Ok(true);
            }

            let workspaces = manager.list_workspaces()?;
            if workspaces.is_empty() {
                app.push_system(
                    "No workspaces to delete. Create one from the sidebar (+ New Workspace)."
                        .to_string(),
                );
                return Ok(true);
            }

            let target = if delete_args.is_empty() {
                let selected_workspace_id = match app.sidebar_item() {
                    Some(SidebarItem::Workspace(index))
                    | Some(SidebarItem::WorkspaceSessionsLabel(index))
                    | Some(SidebarItem::WorkspaceSession(index, _)) => {
                        app.workspaces.get(index).map(|workspace| workspace.id)
                    }
                    _ => Some(session.workspace_id),
                };
                selected_workspace_id.and_then(|id| workspaces.iter().find(|w| w.id == id).cloned())
            } else {
                let selector = delete_args.join(" ");
                if let Ok(id) = selector.trim().parse::<i64>() {
                    workspaces.iter().find(|w| w.id == id).cloned()
                } else {
                    workspaces
                        .iter()
                        .find(|w| w.name.eq_ignore_ascii_case(selector.trim()))
                        .cloned()
                }
            };
            let Some(target_workspace) = target else {
                app.push_system("Workspace not found for delete selection.".to_string());
                return Ok(true);
            };

            open_workspace_delete_modal(app, target_workspace.id, target_workspace.name.clone());
            app.set_toast(
                ToastLevel::Info,
                "Delete modal opened (Enter confirm, Esc cancel)",
            );
        }
        "where" => {
            if session.workspace_id <= 0 || app.active_workspace_id <= 0 {
                app.push_system(
                    "No active workspace. Create one from the sidebar (+ New Workspace)."
                        .to_string(),
                );
                return Ok(true);
            }
            let path = app
                .workspace_root_path
                .as_deref()
                .unwrap_or("(path not set)");
            app.push_system(format!(
                "Active workspace: {} (id: {})\nPath: {}",
                app.workspace_name, session.workspace_id, path
            ));
        }
        "set-path" => {
            let raw_set = prompt
                .strip_prefix("/workspace set-path")
                .map(str::trim)
                .unwrap_or_default();
            if raw_set.is_empty() {
                app.push_system("Usage: /workspace set-path <id|name> --path <folder>".to_string());
                return Ok(true);
            }
            let Some((selector_raw, path_raw)) = raw_set.split_once(" --path ") else {
                app.push_system("Usage: /workspace set-path <id|name> --path <folder>".to_string());
                return Ok(true);
            };

            let selector = selector_raw.trim();
            let selector = unquote_command_value(selector);
            let path_value = unquote_command_value(path_raw.trim());
            if selector.is_empty() || path_value.is_empty() {
                app.push_system("Usage: /workspace set-path <id|name> --path <folder>".to_string());
                return Ok(true);
            }

            let workspaces = manager.list_workspaces()?;
            let target = if let Ok(id) = selector.parse::<i64>() {
                workspaces.into_iter().find(|w| w.id == id)
            } else {
                workspaces
                    .into_iter()
                    .find(|w| w.name.eq_ignore_ascii_case(&selector))
            };
            let Some(workspace) = target else {
                app.push_system(format!(
                    "Workspace not found: {}. Use `/workspace list`.",
                    selector
                ));
                return Ok(true);
            };

            let normalized =
                manager.set_workspace_root_path(workspace.id, &PathBuf::from(path_value))?;
            if workspace.id == session.workspace_id {
                app.workspace_root_path = Some(normalized.clone());
            }
            app.set_toast(
                ToastLevel::Success,
                format!("Workspace path updated: {normalized}"),
            );
        }
        "export" => {
            if parts.len() < 3 {
                app.push_system("Usage: /workspace export <path>".to_string());
                return Ok(true);
            }
            let path = PathBuf::from(parts[2..].join(" "));
            manager.export_workspace_json(session.workspace_id, &path)?;
            app.set_toast(
                ToastLevel::Success,
                format!("Workspace exported to {}", path.display()),
            );
        }
        "import" => {
            if parts.len() < 3 {
                app.push_system("Usage: /workspace import <path>".to_string());
                return Ok(true);
            }
            let path = PathBuf::from(parts[2..].join(" "));
            let imported = manager.import_workspace_json(&path, default_model)?;
            let workspace = manager
                .list_workspaces()?
                .into_iter()
                .find(|w| w.id == imported.workspace_id);
            let workspace_name = workspace
                .as_ref()
                .map(|w| w.name.clone())
                .unwrap_or_else(|| String::from("Workspace"));
            let workspace_root = workspace.and_then(|w| w.root_path);
            apply_workspace_session_transition(
                app,
                session,
                imported,
                workspace_name,
                workspace_root,
                manager,
            )?;
            app.set_toast(
                ToastLevel::Success,
                format!("Workspace imported from {}", path.display()),
            );
        }
        _ => app.push_system(workspace_help_text()),
    }

    Ok(true)
}
