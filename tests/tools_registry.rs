use olly::app::tools::registry;
use serde_json::Value;

/// Test all tools have valid JSON schema
#[test]
fn all_tools_produce_valid_json_schema() {
    let tools = registry::all_tools();
    assert!(!tools.is_empty(), "should have at least one tool");

    for tool in &tools {
        let schema_json = serde_json::to_string(&tool.parameters).expect("should serialize");
        let parsed: Value = serde_json::from_str(&schema_json).expect("should be valid json");

        // Validate basic JSON Schema structure
        assert!(
            parsed.get("type").is_some(),
            "{}: parameters should have 'type' field",
            tool.name
        );
        assert!(
            parsed.get("properties").is_some() || parsed.get("required").is_none(),
            "{}: should have properties if required is specified",
            tool.name
        );
    }
}

/// Test read_file tool schema matches expected structure
#[test]
fn read_file_tool_schema_snapshot() {
    let tool = registry::read_file();
    assert_eq!(tool.name, "read_file");

    let params = tool.parameters;
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["path"].is_object());
    assert_eq!(params["properties"]["path"]["type"], "string");
    assert!(params["required"]
        .as_array()
        .unwrap()
        .contains(&Value::String("path".to_string())));

    // Check optional parameters exist
    assert!(params["properties"]["offset"]["minimum"].as_i64().unwrap() >= 0);
    assert!(params["properties"]["limit"]["maximum"].as_i64().unwrap() <= 500);
}

/// Test list_dir tool schema
#[test]
fn list_dir_tool_schema_snapshot() {
    let tool = registry::list_dir();
    assert_eq!(tool.name, "list_dir");
    assert_eq!(tool.risk_level.to_string(), "read_only");

    let params = tool.parameters;
    assert_eq!(params["type"], "object");
    assert!(params["required"]
        .as_array()
        .unwrap()
        .contains(&Value::String("path".to_string())));
}

/// Test grep_search tool schema
#[test]
fn grep_search_tool_schema_snapshot() {
    let tool = registry::grep_search();
    assert_eq!(tool.name, "grep_search");

    let params = tool.parameters;
    assert_eq!(params["type"], "object");
    assert!(
        params["properties"]["case_sensitive"]["type"]
            .as_str()
            .unwrap()
            == "boolean"
    );
}

/// Test find_by_name tool schema
#[test]
fn find_by_name_tool_schema_snapshot() {
    let tool = registry::find_by_name();
    assert_eq!(tool.name, "find_by_name");

    let params = tool.parameters;
    assert_eq!(params["type"], "object");
    assert!(params["required"]
        .as_array()
        .unwrap()
        .contains(&Value::String("pattern".to_string())));
}

/// Test execute_command tool schema and risk level
#[test]
fn execute_command_tool_schema_snapshot() {
    let tool = registry::execute_command();
    assert_eq!(tool.name, "execute_command");
    assert_eq!(tool.risk_level.to_string(), "shell");

    let params = tool.parameters;
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["cwd"].is_object());
}

/// Test tool lookup by name
#[test]
fn get_tool_returns_correct_tool() {
    assert!(registry::get_tool("read_file").is_some());
    assert!(registry::get_tool("list_dir").is_some());
    assert!(registry::get_tool("grep_search").is_some());
    assert!(registry::get_tool("find_by_name").is_some());
    assert!(registry::get_tool("execute_command").is_some());
    assert!(registry::get_tool("nonexistent").is_none());
}

/// Test tools_by_max_risk filtering
#[test]
fn tools_by_max_risk_filters_correctly() {
    use olly::app::tools::ToolRiskLevel;

    // ReadOnly should only return read-only tools
    let read_only = registry::tools_by_max_risk(ToolRiskLevel::ReadOnly);
    for tool in &read_only {
        assert_eq!(tool.risk_level, ToolRiskLevel::ReadOnly);
    }

    // Shell should return all tools
    let all = registry::tools_by_max_risk(ToolRiskLevel::Shell);
    assert_eq!(all.len(), registry::all_tools().len());

    // Verify we have the expected distribution
    let read_only_count = read_only.len();
    let all_count = all.len();
    assert!(
        read_only_count < all_count,
        "should have some higher-risk tools"
    );
}

/// Test ToolDescriptor conversion to ToolDefinition
#[test]
fn tool_descriptor_to_definition_conversion() {
    use olly::app::tools::ToolDescriptor;
    use serde_json::json;

    let descriptor = ToolDescriptor::new(
        "test_tool",
        "A test tool",
        olly::app::tools::ToolRiskLevel::ReadOnly,
        json!({"type": "object", "properties": {}}),
    );

    let definition = descriptor.to_tool_definition();
    assert_eq!(definition.tool_type, "function");
    assert_eq!(definition.function.name, "test_tool");
    assert_eq!(definition.function.description, "A test tool");
}

/// Test ToolResult JSON serialization
#[test]
fn tool_result_json_serialization() {
    use olly::app::tools::ToolResult;

    let success = ToolResult::success("file contents here");
    let json_str = success.to_json_string();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["output"], "file contents here");

    let error = ToolResult::error("not_found", "File not found");
    let json_str = error.to_json_string();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "not_found");
}

/// Test ToolResult truncated flag
#[test]
fn tool_result_truncated_flag() {
    use olly::app::tools::ToolResult;

    let result = ToolResult::success("truncated content").truncated();
    let json_str = result.to_json_string();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed["truncated"], true);
}

/// Test ToolsState default is disabled
#[test]
fn tools_state_defaults_to_disabled() {
    use olly::app::tools::ToolsState;

    let state = ToolsState::default();
    assert!(!state.enabled);
}

/// Test ToolsState tool filtering
#[test]
fn tools_state_allows_and_denies_tools() {
    use olly::app::tools::ToolsState;

    let mut state = ToolsState::default();

    // Empty lists: all tools allowed
    assert!(state.is_tool_allowed("any_tool"));

    // Add to denied list
    state.denied_tools.push("dangerous".to_string());
    assert!(!state.is_tool_allowed("dangerous"));
    assert!(state.is_tool_allowed("safe"));

    // Add allowed list (restrictive mode)
    state.allowed_tools.push("read_file".to_string());
    state.allowed_tools.push("list_dir".to_string());
    assert!(state.is_tool_allowed("read_file"));
    assert!(state.is_tool_allowed("list_dir"));
    assert!(!state.is_tool_allowed("grep_search")); // not in allowed list
    assert!(!state.is_tool_allowed("dangerous")); // still denied
}
