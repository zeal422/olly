#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRoute {
    Commands,
    Models,
    Search,
    Export,
    WorkspacesAlias,
    Workspace,
    Session,
    Checkpoint,
    Attach,
    ClearAttachments,
    Model,
    Url,
    Unknown,
}

pub fn route_command(root: &str) -> CommandRoute {
    match root {
        "/commands" => CommandRoute::Commands,
        "/models" => CommandRoute::Models,
        "/search" => CommandRoute::Search,
        "/export" => CommandRoute::Export,
        "/workspaces" => CommandRoute::WorkspacesAlias,
        "/workspace" => CommandRoute::Workspace,
        "/session" => CommandRoute::Session,
        "/checkpoint" => CommandRoute::Checkpoint,
        "/attach" => CommandRoute::Attach,
        "/clear-attachments" => CommandRoute::ClearAttachments,
        "/model" => CommandRoute::Model,
        "/url" => CommandRoute::Url,
        _ => CommandRoute::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{route_command, CommandRoute};

    #[test]
    fn route_command_maps_known_roots() {
        assert_eq!(route_command("/commands"), CommandRoute::Commands);
        assert_eq!(route_command("/workspace"), CommandRoute::Workspace);
        assert_eq!(route_command("/session"), CommandRoute::Session);
        assert_eq!(route_command("/url"), CommandRoute::Url);
    }

    #[test]
    fn route_command_maps_unknown_root() {
        assert_eq!(route_command("/does-not-exist"), CommandRoute::Unknown);
    }
}
