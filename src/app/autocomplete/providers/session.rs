use crate::app::SessionEntry;

fn session_subcommand_suggestions() -> Vec<String> {
    vec![
        String::from("/session list"),
        String::from("/session new "),
        String::from("/session create "),
        String::from("/session switch "),
        String::from("/session rename "),
        String::from("/session delete"),
    ]
}

pub(crate) fn suggestions(
    raw: &str,
    token: &str,
    sessions: &[SessionEntry],
) -> Option<Vec<String>> {
    if token == "/session" {
        if raw.ends_with(' ') {
            return Some(session_subcommand_suggestions());
        }
        return Some(vec![String::from("/session ")]);
    }

    let rest_raw = raw.strip_prefix("/session ")?;
    let rest = rest_raw.trim_start();
    if rest.is_empty() {
        return Some(session_subcommand_suggestions());
    }

    let parts = rest.split_whitespace().collect::<Vec<_>>();
    let sub = parts.first().copied().unwrap_or_default();
    let subcommands = ["list", "new", "create", "switch", "rename", "delete"];
    let has_sub_arg = rest.contains(' ');

    if !has_sub_arg {
        return Some(
            subcommands
                .into_iter()
                .filter(|candidate| candidate.starts_with(sub))
                .map(|candidate| match candidate {
                    "list" => String::from("/session list"),
                    _ => format!("/session {candidate} "),
                })
                .collect(),
        );
    }

    if sub == "switch" {
        let arg_partial = rest[sub.len()..].trim();
        let mut suggestions = sessions
            .iter()
            .flat_map(|session| {
                let by_id = format!("/session switch {}", session.id);
                let by_name = format!("/session switch {}", session.title);
                [by_id, by_name]
            })
            .filter(|candidate| {
                if arg_partial.is_empty() {
                    true
                } else {
                    candidate
                        .to_lowercase()
                        .starts_with(&format!("/session switch {}", arg_partial.to_lowercase()))
                }
            })
            .collect::<Vec<_>>();
        suggestions.sort();
        suggestions.dedup();
        return Some(suggestions);
    }

    if sub == "rename" {
        let arg_partial = rest[sub.len()..].trim();
        let mut suggestions = sessions
            .iter()
            .flat_map(|session| {
                let by_id = format!("/session rename {} ", session.id);
                let by_name = format!("/session rename {} --to ", session.title);
                [by_id, by_name]
            })
            .filter(|candidate| {
                if arg_partial.is_empty() {
                    true
                } else {
                    candidate
                        .to_lowercase()
                        .starts_with(&format!("/session rename {}", arg_partial.to_lowercase()))
                }
            })
            .collect::<Vec<_>>();
        suggestions.sort();
        suggestions.dedup();
        return Some(suggestions);
    }

    if sub == "delete" {
        return Some(vec![String::from("/session delete")]);
    }

    Some(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::suggestions;

    #[test]
    fn session_delete_keeps_single_command_suggestion() {
        let sessions = vec![
            crate::app::SessionEntry {
                id: 1,
                title: String::from("Main"),
            },
            crate::app::SessionEntry {
                id: 2,
                title: String::from("Refactor"),
            },
        ];
        let items = suggestions("/session delete ", "/session delete", &sessions)
            .expect("provider response");
        assert_eq!(items, vec![String::from("/session delete")]);
    }
}
