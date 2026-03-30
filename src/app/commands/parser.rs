#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand<'a> {
    raw: &'a str,
    parts: Vec<&'a str>,
}

pub fn parse_command(input: &str) -> ParsedCommand<'_> {
    ParsedCommand {
        raw: input,
        parts: input.split_whitespace().collect(),
    }
}

impl<'a> ParsedCommand<'a> {
    pub fn raw(&self) -> &'a str {
        self.raw
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    pub fn root(&self) -> Option<&'a str> {
        self.parts.first().copied()
    }

    pub fn parts(&self) -> &[&'a str] {
        &self.parts
    }
}

#[cfg(test)]
mod tests {
    use super::parse_command;

    #[test]
    fn parse_command_handles_empty_input() {
        let parsed = parse_command("");
        assert!(parsed.is_empty());
        assert_eq!(parsed.raw(), "");
        assert_eq!(parsed.root(), None);
        assert!(parsed.parts().is_empty());
    }

    #[test]
    fn parse_command_splits_whitespace_like_existing_behavior() {
        let parsed = parse_command("/workspace create New Workspace --path C:/Repo");
        assert_eq!(parsed.root(), Some("/workspace"));
        assert_eq!(
            parsed.parts(),
            vec![
                "/workspace",
                "create",
                "New",
                "Workspace",
                "--path",
                "C:/Repo"
            ]
        );
    }

    #[test]
    fn parse_command_keeps_current_quoted_token_behavior() {
        let parsed = parse_command("/session rename \"Main Conversation\" --to \"New Title\"");
        assert_eq!(
            parsed.parts(),
            vec![
                "/session",
                "rename",
                "\"Main",
                "Conversation\"",
                "--to",
                "\"New",
                "Title\""
            ]
        );
    }

    #[test]
    fn parse_command_supports_unknown_commands_without_filtering() {
        let parsed = parse_command("/totally-unknown value");
        assert_eq!(parsed.root(), Some("/totally-unknown"));
        assert_eq!(parsed.parts(), vec!["/totally-unknown", "value"]);
    }

    #[test]
    fn parse_command_keeps_to_option_tokens() {
        let parsed = parse_command("/session rename Main --to Better Title");
        assert_eq!(
            parsed.parts(),
            vec!["/session", "rename", "Main", "--to", "Better", "Title"]
        );
    }

    #[test]
    fn parse_command_keeps_path_option_tokens() {
        let parsed = parse_command("/workspace set-path 1 --path C:/Projects/Olly");
        assert_eq!(
            parsed.parts(),
            vec!["/workspace", "set-path", "1", "--path", "C:/Projects/Olly"]
        );
    }
}
