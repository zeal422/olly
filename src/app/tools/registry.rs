//! Tool registry - defines built-in tools and their JSON schemas

use serde_json::json;

use super::{ToolDescriptor, ToolRiskLevel};

/// Get all built-in tool descriptors
pub fn all_tools() -> Vec<ToolDescriptor> {
    vec![
        read_file(),
        list_dir(),
        grep_search(),
        find_by_name(),
        execute_command(),
    ]
}

/// Get tools filtered by risk level (e.g., exclude shell tools for safety)
pub fn tools_by_max_risk(max_risk: ToolRiskLevel) -> Vec<ToolDescriptor> {
    all_tools()
        .into_iter()
        .filter(|t| risk_level_ordinal(t.risk_level) <= risk_level_ordinal(max_risk))
        .collect()
}

fn risk_level_ordinal(risk: ToolRiskLevel) -> u8 {
    match risk {
        ToolRiskLevel::ReadOnly => 0,
        ToolRiskLevel::Mutating => 1,
        ToolRiskLevel::Shell => 2,
    }
}

/// Get a single tool by name
pub fn get_tool(name: &str) -> Option<ToolDescriptor> {
    all_tools().into_iter().find(|t| t.name == name)
}

/// Read file tool - read contents of a file with optional offset/limit
pub fn read_file() -> ToolDescriptor {
    ToolDescriptor::new(
        "read_file",
        "Read the contents of a file. Returns the file content as text. For binary files, returns a summary. Use offset and limit for large files.",
        ToolRiskLevel::ReadOnly,
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The relative or absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Optional line offset to start reading from (0-indexed)",
                    "minimum": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "Optional maximum number of lines to read",
                    "minimum": 1,
                    "maximum": 500
                }
            },
            "required": ["path"]
        }),
    )
}

/// List directory tool - list contents of a directory
pub fn list_dir() -> ToolDescriptor {
    ToolDescriptor::new(
        "list_dir",
        "List the contents of a directory. Returns files and subdirectories with their types.",
        ToolRiskLevel::ReadOnly,
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The relative or absolute path to the directory to list. Use '.' for current directory."
                }
            },
            "required": ["path"]
        }),
    )
}

/// Grep search tool - search for patterns in files
pub fn grep_search() -> ToolDescriptor {
    ToolDescriptor::new(
        "grep_search",
        "Search for a regex pattern in files. Returns matching lines with file paths and line numbers. Limited to first 50 matches.",
        ToolRiskLevel::ReadOnly,
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Optional directory or file to search in. Defaults to workspace root if not specified."
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Whether the search should be case sensitive. Defaults to false."
                }
            },
            "required": ["query"]
        }),
    )
}

/// Find by name tool - find files by glob pattern
pub fn find_by_name() -> ToolDescriptor {
    ToolDescriptor::new(
        "find_by_name",
        "Find files by name pattern. Supports glob patterns like '*.rs' or '**/Cargo.toml'. Returns matching file paths.",
        ToolRiskLevel::ReadOnly,
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match file names against. Examples: '*.rs', '**/Cargo.toml', 'src/**/*.rs'"
                },
                "path": {
                    "type": "string",
                    "description": "Optional directory to search in. Defaults to workspace root."
                }
            },
            "required": ["pattern"]
        }),
    )
}

/// Execute command tool - restricted shell command execution
pub fn execute_command() -> ToolDescriptor {
    ToolDescriptor::new(
        "execute_command",
        "Execute a shell command with restrictions. Only allowed commands: cargo, git, rg (ripgrep), python, node. Commands outside workspace require confirmation. Has 60s timeout and 10KB output limit.",
        ToolRiskLevel::Shell,
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute. Must start with an allowed command (cargo, git, rg, python, node). Examples: 'cargo test', 'git status', 'rg TODO'"
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory for the command. Defaults to workspace root."
                }
            },
            "required": ["command"]
        }),
    )
}
