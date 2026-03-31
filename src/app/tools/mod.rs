//! Tool registry for Olly - defines available tools and their schemas for the Ollama API.

pub mod executor;
pub mod registry;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Risk level classification for tools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolRiskLevel {
    ReadOnly,
    Mutating,
    Shell,
}

impl ToolRiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolRiskLevel::ReadOnly => "read_only",
            ToolRiskLevel::Mutating => "mutating",
            ToolRiskLevel::Shell => "shell",
        }
    }
}

impl std::fmt::Display for ToolRiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Complete tool descriptor including metadata and JSON schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub risk_level: ToolRiskLevel,
    pub parameters: Value,
}

impl ToolDescriptor {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        risk_level: ToolRiskLevel,
        parameters: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            risk_level,
            parameters,
        }
    }

    /// Convert to Ollama ToolDefinition for API requests
    pub fn to_tool_definition(&self) -> crate::ollama::chat::ToolDefinition {
        crate::ollama::chat::ToolDefinition::new(
            &self.name,
            &self.description,
            self.parameters.clone(),
        )
    }
}

/// Tool execution result returned to the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub status: ToolResultStatus,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultStatus {
    Success,
    Error,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            status: ToolResultStatus::Success,
            output: output.into(),
            error: None,
            error_type: None,
            truncated: None,
        }
    }

    pub fn error(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: ToolResultStatus::Error,
            output: String::new(),
            error: Some(message.into()),
            error_type: Some(error_type.into()),
            truncated: None,
        }
    }

    pub fn truncated(mut self) -> Self {
        self.truncated = Some(true);
        self
    }

    /// Format as JSON string for the tool message content
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"status":"error","error":"failed to serialize result"}"#.to_string()
        })
    }
}

/// Tool policy for execution control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPolicy {
    #[default]
    Auto,
    Confirm,
}

impl ToolPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolPolicy::Auto => "auto",
            ToolPolicy::Confirm => "confirm",
        }
    }
}

impl std::str::FromStr for ToolPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(ToolPolicy::Auto),
            "confirm" => Ok(ToolPolicy::Confirm),
            _ => Err(format!("unknown tool policy: {}", s)),
        }
    }
}

/// Global tools state
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolsState {
    pub enabled: bool,
    pub policy: ToolPolicy,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
}

impl ToolsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // If explicitly denied, reject
        if self.denied_tools.iter().any(|t| t == tool_name) {
            return false;
        }

        // If allowed list is empty, all non-denied tools are allowed
        if self.allowed_tools.is_empty() {
            return true;
        }

        // Otherwise, must be in allowed list
        self.allowed_tools.iter().any(|t| t == tool_name)
    }
}
