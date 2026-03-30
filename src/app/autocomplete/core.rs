use crate::app::{SessionEntry, WorkspaceEntry};

use super::providers;

pub(crate) fn suggestions(
    input: &str,
    models: &[String],
    workspaces: &[WorkspaceEntry],
    sessions: &[SessionEntry],
) -> Vec<String> {
    let raw = input.trim_start();
    if !raw.starts_with('/') || raw.contains('\n') {
        return Vec::new();
    }
    let token = raw.trim_end();

    if let Some(items) = providers::models::suggestions(raw, token, models) {
        return items;
    }

    if let Some(items) = providers::workspace::suggestions(raw, token, workspaces) {
        return items;
    }

    if let Some(items) = providers::session::suggestions(raw, token, sessions) {
        return items;
    }

    if token.contains(' ') {
        return Vec::new();
    }

    providers::misc::top_level(token)
}
