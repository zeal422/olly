//! Tools command handlers for Olly
//!
//! Commands:
//! - /tools status          - Show current tool settings
//! - /tools enable          - Enable tool execution
//! - /tools disable         - Disable tool execution (default)
//! - /tools policy auto     - Auto-execute read-only tools
//! - /tools policy confirm  - Confirm all shell tools
//! - /tools list            - List available tools
//! - /tools allow <name>    - Add tool to allowlist
//! - /tools deny <name>     - Add tool to denylist
//! - /tools reset           - Clear allow/deny lists

use crate::app::commands::parser::ParsedCommand;
use crate::app::tools::registry::{all_tools, get_tool};
use crate::app::tools::ToolPolicy;
use crate::app::{state::ToastLevel, App};

fn known_tools_hint() -> String {
    let mut names = all_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    names.sort();
    names.join(", ")
}

/// Handle /tools commands
pub fn handle_tools_command(app: &mut App, parsed: &ParsedCommand<'_>) {
    let parts = parsed.parts();

    // Handle empty command (just "/tools" or "/tools ") - show status
    if parts.is_empty() || parts.len() == 1 {
        show_tools_status(app);
        return;
    }

    match parts.get(1).copied() {
        Some("status") => show_tools_status(app),
        Some("enable") | Some("on") => enable_tools(app),
        Some("disable") | Some("off") => disable_tools(app),
        Some("policy") => handle_policy_command(app, parts),
        Some("list") => list_tools(app),
        Some("allow") => handle_allow_command(app, parts),
        Some("deny") => handle_deny_command(app, parts),
        Some("reset") => reset_tool_lists(app),
        Some(subcommand) => {
            app.push_system(format!("Unknown /tools subcommand: {}", subcommand));
            app.set_toast(ToastLevel::Error, "Unknown subcommand");
        }
        None => unreachable!(),
    }
}

/// Show current tool settings
fn show_tools_status(app: &mut App) {
    let status = if app.tools_state.enabled {
        "enabled"
    } else {
        "disabled (default)"
    };

    let policy = app.tools_state.policy.as_str();

    let allowed = if app.tools_state.allowed_tools.is_empty() {
        "all (not restricted)".to_string()
    } else {
        app.tools_state.allowed_tools.join(", ")
    };

    let denied = if app.tools_state.denied_tools.is_empty() {
        "none".to_string()
    } else {
        app.tools_state.denied_tools.join(", ")
    };

    let message = format!(
        "Tools Status:\n\
        - Status: {}\n\
        - Policy: {}\n\
        - Allowed: {}\n\
        - Denied: {}",
        status, policy, allowed, denied
    );

    app.push_system(message);
}

/// Enable tool execution
fn enable_tools(app: &mut App) {
    if app.workspace_root_path.is_none() {
        app.push_system("Cannot enable tools: no active workspace".to_string());
        app.set_toast(ToastLevel::Error, "Create or open a workspace first");
        return;
    }

    app.tools_state.enabled = true;
    app.push_system("Tools enabled".to_string());
    app.set_toast(ToastLevel::Info, "Tools enabled");
}

/// Disable tool execution
fn disable_tools(app: &mut App) {
    app.tools_state.enabled = false;
    app.push_system("Tools disabled".to_string());
    app.set_toast(ToastLevel::Info, "Tools disabled");
}

/// Handle policy subcommand
fn handle_policy_command(app: &mut App, parts: &[&str]) {
    if parts.len() < 3 {
        app.push_system(format!(
            "Current policy: {}\nUsage: /tools policy [auto|confirm]",
            app.tools_state.policy.as_str()
        ));
        return;
    }

    match parts[2] {
        "auto" => {
            app.tools_state.policy = ToolPolicy::Auto;
            app.push_system("Tool policy set to: auto".to_string());
            app.set_toast(ToastLevel::Info, "Policy: auto");
        }
        "confirm" => {
            app.tools_state.policy = ToolPolicy::Confirm;
            app.push_system("Tool policy set to: confirm".to_string());
            app.set_toast(ToastLevel::Info, "Policy: confirm");
        }
        _ => {
            app.push_system("Invalid policy. Use: auto or confirm".to_string());
            app.set_toast(ToastLevel::Error, "Invalid policy");
        }
    }
}

/// List available tools
fn list_tools(app: &mut App) {
    let tools = all_tools();

    let mut message = "Available Tools:\n".to_string();

    for tool in tools {
        let risk = tool.risk_level.as_str();
        let status = if app.tools_state.is_tool_allowed(&tool.name) {
            "[+]"
        } else {
            "[-]"
        };

        message.push_str(&format!(
            "{} {} ({}): {}\n",
            status, tool.name, risk, tool.description
        ));
    }

    message.push_str("\nUse /tools allow <name> or /tools deny <name> to configure");

    app.push_system(message);
}

/// Handle allow subcommand
fn handle_allow_command(app: &mut App, parts: &[&str]) {
    if parts.len() < 3 {
        app.push_system("Usage: /tools allow <tool_name>".to_string());
        return;
    }

    let tool_name = parts[2];
    if get_tool(tool_name).is_none() {
        app.push_system(format!(
            "Unknown tool '{}'. Available tools: {}",
            tool_name,
            known_tools_hint()
        ));
        app.set_toast(ToastLevel::Error, "Unknown tool name");
        return;
    }

    // Remove from denied if present
    app.tools_state.denied_tools.retain(|t| t != tool_name);

    // Add to allowed if not already there
    if !app
        .tools_state
        .allowed_tools
        .contains(&tool_name.to_string())
    {
        app.tools_state.allowed_tools.push(tool_name.to_string());
    }

    app.push_system(format!("Tool '{}' added to allowlist", tool_name));
    app.set_toast(ToastLevel::Info, format!("{} allowed", tool_name));
}

/// Handle deny subcommand
fn handle_deny_command(app: &mut App, parts: &[&str]) {
    if parts.len() < 3 {
        app.push_system("Usage: /tools deny <tool_name>".to_string());
        return;
    }

    let tool_name = parts[2];
    if get_tool(tool_name).is_none() {
        app.push_system(format!(
            "Unknown tool '{}'. Available tools: {}",
            tool_name,
            known_tools_hint()
        ));
        app.set_toast(ToastLevel::Error, "Unknown tool name");
        return;
    }

    // Remove from allowed if present
    app.tools_state.allowed_tools.retain(|t| t != tool_name);

    // Add to denied if not already there
    if !app
        .tools_state
        .denied_tools
        .contains(&tool_name.to_string())
    {
        app.tools_state.denied_tools.push(tool_name.to_string());
    }

    app.push_system(format!("Tool '{}' added to denylist", tool_name));
    app.set_toast(ToastLevel::Info, format!("{} denied", tool_name));
}

/// Reset allow/deny lists
fn reset_tool_lists(app: &mut App) {
    app.tools_state.allowed_tools.clear();
    app.tools_state.denied_tools.clear();
    app.push_system("Tool allow/deny lists cleared".to_string());
    app.set_toast(ToastLevel::Info, "Lists reset");
}

#[cfg(test)]
mod tests {
    use super::handle_tools_command;
    use crate::app::{commands::parser::parse_command, App};

    #[test]
    fn allow_unknown_tool_is_rejected_without_mutation() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        let parsed = parse_command("/tools allow typo_tool");
        handle_tools_command(&mut app, &parsed);

        assert!(app.tools_state.allowed_tools.is_empty());
        let system_message = app
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "System")
            .expect("system message should exist");
        assert!(system_message.content.contains("Unknown tool 'typo_tool'"));
    }

    #[test]
    fn deny_unknown_tool_is_rejected_without_mutation() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        let parsed = parse_command("/tools deny typo_tool");
        handle_tools_command(&mut app, &parsed);

        assert!(app.tools_state.denied_tools.is_empty());
        let system_message = app
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "System")
            .expect("system message should exist");
        assert!(system_message.content.contains("Unknown tool 'typo_tool'"));
    }

    #[test]
    fn allow_known_tool_updates_allowlist() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        let parsed = parse_command("/tools allow read_file");
        handle_tools_command(&mut app, &parsed);

        assert!(app
            .tools_state
            .allowed_tools
            .iter()
            .any(|name| name == "read_file"));
    }
}
