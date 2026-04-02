use std::{env, fs, path::PathBuf};

use crate::app::state::WorkspaceEntry;

fn workspace_subcommand_suggestions() -> Vec<String> {
    vec![
        String::from("/workspace list"),
        String::from("/workspace switch "),
        String::from("/workspace create "),
        String::from("/workspace rename "),
        String::from("/workspace delete "),
        String::from("/workspace where"),
        String::from("/workspace set-path "),
        String::from("/workspace export "),
        String::from("/workspace import "),
    ]
}

fn filesystem_directory_suggestions(partial: &str) -> Vec<String> {
    let trimmed = partial.trim_start();
    let (base_dir, prefix) = if trimmed.is_empty() {
        (
            env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            String::new(),
        )
    } else {
        let partial_path = PathBuf::from(trimmed);
        if trimmed.ends_with('/') || trimmed.ends_with('\\') {
            (partial_path, String::new())
        } else {
            let parent = partial_path
                .parent()
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let prefix = partial_path
                .file_name()
                .and_then(|segment| segment.to_str())
                .unwrap_or_default()
                .to_string();
            (parent, prefix)
        }
    };

    let base_dir = if base_dir.as_os_str().is_empty() {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        base_dir
    };

    let mut suggestions = fs::read_dir(&base_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            let is_dir = entry.file_type().ok()?.is_dir();
            if !is_dir {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                return None;
            }
            let path = entry.path();
            let display = path.display().to_string();
            Some(if display.contains(' ') {
                format!("\"{display}\"")
            } else {
                display
            })
        })
        .collect::<Vec<_>>();

    suggestions.sort();
    suggestions
}

pub(crate) fn suggestions(
    raw: &str,
    token: &str,
    workspaces: &[WorkspaceEntry],
) -> Option<Vec<String>> {
    if token == "/workspace" {
        if raw.ends_with(' ') {
            return Some(workspace_subcommand_suggestions());
        }
        return Some(vec![String::from("/workspace ")]);
    }

    if token == "/workspaces" {
        return Some(vec![String::from("/workspace ")]);
    }

    let rest_raw = raw.strip_prefix("/workspace ")?;
    let rest = rest_raw.trim_start();
    if rest.is_empty() {
        return Some(workspace_subcommand_suggestions());
    }

    let parts = rest.split_whitespace().collect::<Vec<_>>();
    let sub = parts.first().copied().unwrap_or_default();
    let subcommands = [
        "list", "switch", "create", "rename", "delete", "export", "import", "where", "set-path",
    ];
    let has_sub_arg = rest.contains(' ');

    if !has_sub_arg {
        return Some(
            subcommands
                .into_iter()
                .filter(|candidate| candidate.starts_with(sub))
                .map(|candidate| match candidate {
                    "list" | "where" => format!("/workspace {candidate}"),
                    _ => format!("/workspace {candidate} "),
                })
                .collect(),
        );
    }

    if sub == "switch" || sub == "delete" {
        let arg_partial = rest[sub.len()..].trim();
        if sub == "delete" && arg_partial.is_empty() {
            return Some(vec![
                String::from("/workspace delete"),
                String::from("/workspace delete --confirm"),
                String::from("/workspace delete --cancel"),
            ]);
        }
        let mut suggestions = Vec::new();
        if sub == "delete" {
            suggestions.push(String::from("/workspace delete --confirm "));
            suggestions.push(String::from("/workspace delete --cancel"));
        }
        suggestions.extend(workspaces.iter().flat_map(|workspace| {
            let by_id = format!("/workspace {sub} {}", workspace.id);
            let by_name = format!("/workspace {sub} {}", workspace.name);
            [by_id, by_name]
        }));
        let mut suggestions = suggestions
            .into_iter()
            .filter(|candidate| {
                if arg_partial.is_empty() {
                    true
                } else {
                    candidate
                        .to_lowercase()
                        .starts_with(&format!("/workspace {sub} {}", arg_partial.to_lowercase()))
                }
            })
            .collect::<Vec<_>>();
        suggestions.sort();
        suggestions.dedup();
        return Some(suggestions);
    }

    if sub == "rename" {
        let arg_partial = rest[sub.len()..].trim();
        if arg_partial.is_empty() {
            return Some(vec![String::from("/workspace rename ")]);
        }
        return Some(vec![format!("/workspace rename {arg_partial}")]);
    }

    if sub == "set-path" {
        let arg_partial = rest[sub.len()..].trim();
        if let Some((selector_part, path_partial)) = arg_partial.split_once(" --path ") {
            let command_prefix = format!("/workspace set-path {} --path ", selector_part.trim());
            let path_suggestions = filesystem_directory_suggestions(path_partial);
            if path_suggestions.is_empty() {
                return Some(vec![command_prefix]);
            }
            return Some(
                path_suggestions
                    .into_iter()
                    .map(|path| format!("{command_prefix}{path}"))
                    .collect(),
            );
        }
        let mut suggestions = workspaces
            .iter()
            .flat_map(|workspace| {
                let by_id = format!("/workspace set-path {} --path ", workspace.id);
                let by_name = format!("/workspace set-path {} --path ", workspace.name);
                [by_id, by_name]
            })
            .filter(|candidate| {
                if arg_partial.is_empty() {
                    true
                } else {
                    candidate.to_lowercase().starts_with(&format!(
                        "/workspace set-path {}",
                        arg_partial.to_lowercase()
                    ))
                }
            })
            .collect::<Vec<_>>();
        suggestions.sort();
        suggestions.dedup();
        return Some(suggestions);
    }

    if sub == "create" {
        let arg_partial = rest[sub.len()..].trim();
        if let Some((name_part, path_partial)) = arg_partial.split_once(" --path ") {
            let command_prefix = format!("/workspace create {} --path ", name_part.trim());
            let path_suggestions = filesystem_directory_suggestions(path_partial);
            if path_suggestions.is_empty() {
                return Some(vec![command_prefix]);
            }
            return Some(
                path_suggestions
                    .into_iter()
                    .map(|path| format!("{command_prefix}{path}"))
                    .collect(),
            );
        }
        if !arg_partial.is_empty() && !arg_partial.contains(" --path ") {
            return Some(vec![format!("/workspace create {arg_partial} --path ")]);
        }
    }

    Some(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::suggestions;

    #[test]
    fn workspace_switch_suggests_ids_and_names() {
        let workspaces = vec![
            crate::app::WorkspaceEntry {
                id: 7,
                name: String::from("Main WS"),
                root_path: None,
                sessions: Vec::new(),
            },
            crate::app::WorkspaceEntry {
                id: 8,
                name: String::from("Backup"),
                root_path: None,
                sessions: Vec::new(),
            },
        ];
        let items = suggestions("/workspace switch ", "/workspace switch", &workspaces)
            .expect("provider response");
        assert!(items.contains(&String::from("/workspace switch 7")));
        assert!(items.contains(&String::from("/workspace switch Main WS")));
    }
}
