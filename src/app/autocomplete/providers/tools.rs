//! Autocomplete provider for /tools commands

/// Subcommands for /tools
const TOOLS_SUBCOMMANDS: &[&str] = &[
    "status", "enable", "disable", "policy", "list", "allow", "deny", "reset",
];

/// Provide autocomplete suggestions for /tools commands
pub(crate) fn tools_subcommands(token: &str) -> Vec<String> {
    TOOLS_SUBCOMMANDS
        .iter()
        .filter(|cmd| cmd.starts_with(token))
        .map(|cmd| format!("/tools {} ", cmd))
        .collect()
}

/// Provide suggestions for tool names (for allow/deny commands)
#[allow(dead_code)]
pub(crate) fn tool_names(token: &str, prefix: &str) -> Vec<String> {
    use crate::app::tools::registry::all_tools;

    all_tools()
        .iter()
        .filter(|tool| tool.name.starts_with(token))
        .map(|tool| format!("{} {} ", prefix, tool.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_subcommands_suggests_all_for_empty_token() {
        let suggestions = tools_subcommands("");
        assert_eq!(suggestions.len(), TOOLS_SUBCOMMANDS.len());
        assert!(suggestions.iter().all(|s| s.starts_with("/tools ")));
    }

    #[test]
    fn tools_subcommands_filters_by_prefix() {
        let suggestions = tools_subcommands("en");
        assert!(suggestions.iter().any(|s| s.contains("enable")));
        assert!(!suggestions.iter().any(|s| s.contains("disable")));
    }
}
