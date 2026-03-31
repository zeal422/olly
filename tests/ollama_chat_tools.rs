use olly::ollama::chat::{
    parse_stream_line, ChatMessage, ChatToolCall, ToolDefinition, ToolFunction,
};
use serde_json::{json, Value};

/// Test that ChatMessage serializes correctly with all optional fields
#[test]
fn chat_message_full_serialization() {
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "Hello".to_string(),
        images: None,
        tool_calls: Some(vec![ChatToolCall {
            function: Some(ToolFunction {
                name: "read_file".to_string(),
                arguments: json!({"path": "test.rs"}),
            }),
        }]),
        tool_name: None,
        thinking: Some("Processing...".to_string()),
    };

    let json_str = serde_json::to_string(&message).expect("should serialize");
    let parsed: Value = serde_json::from_str(&json_str).expect("should be valid json");

    assert_eq!(parsed["role"], "assistant");
    assert_eq!(parsed["content"], "Hello");
    assert!(parsed["images"].is_null() || !parsed.as_object().unwrap().contains_key("images"));
    assert!(parsed["tool_calls"].is_array());
    assert_eq!(parsed["tool_calls"][0]["function"]["name"], "read_file");
    assert_eq!(parsed["thinking"], "Processing...");
}

/// Test backward compatibility - message without new fields should deserialize
#[test]
fn chat_message_backward_compatibility() {
    let legacy_json = r#"{"role":"user","content":"test message"}"#;
    let message: ChatMessage =
        serde_json::from_str(legacy_json).expect("should deserialize legacy");

    assert_eq!(message.role, "user");
    assert_eq!(message.content, "test message");
    assert!(message.images.is_none());
    assert!(message.tool_calls.is_none());
    assert!(message.tool_name.is_none());
    assert!(message.thinking.is_none());
}

/// Test message with tool role and tool_name
#[test]
fn chat_message_tool_role() {
    let message = ChatMessage::new_tool("read_file", "file contents here");

    assert_eq!(message.role, "tool");
    assert_eq!(message.content, "file contents here");
    assert_eq!(message.tool_name, Some("read_file".to_string()));

    let json_str = serde_json::to_string(&message).expect("should serialize");
    let parsed: Value = serde_json::from_str(&json_str).expect("should be valid json");

    assert_eq!(parsed["role"], "tool");
    assert_eq!(parsed["tool_name"], "read_file");
}

/// Test ToolDefinition serialization matches Ollama API format
#[test]
fn tool_definition_serialization() {
    let tool = ToolDefinition::new(
        "read_file",
        "Read the contents of a file",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path"
                }
            },
            "required": ["path"]
        }),
    );

    let json_str = serde_json::to_string(&tool).expect("should serialize");
    let parsed: Value = serde_json::from_str(&json_str).expect("should be valid json");

    assert_eq!(parsed["type"], "function");
    assert_eq!(parsed["function"]["name"], "read_file");
    assert_eq!(
        parsed["function"]["description"],
        "Read the contents of a file"
    );
    assert!(parsed["function"]["parameters"].is_object());
}

/// Test ChatRequest serialization with tools (using builder pattern)
#[test]
fn chat_request_with_tools_serialization() {
    // This tests the internal ChatRequest struct behavior
    // Since ChatRequest is private, we test via the public ToolDefinition
    let tools = vec![
        ToolDefinition::new("read_file", "Read file", json!({})),
        ToolDefinition::new("list_dir", "List directory", json!({})),
    ];

    let tools_json = serde_json::to_string(&tools).expect("should serialize tools array");
    let parsed: Value = serde_json::from_str(&tools_json).expect("should be valid json");

    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["function"]["name"], "read_file");
    assert_eq!(parsed[1]["function"]["name"], "list_dir");
}

/// Test parsing stream line with thinking field
#[test]
fn parse_stream_line_with_thinking() {
    let line = r#"{"message":{"role":"assistant","content":"Hello","thinking":"Planning response..."},"done":true}"#;

    let result = parse_stream_line(line).expect("should parse");
    let chunk = result.expect("should have chunk");

    assert_eq!(chunk.content, "Hello");
    assert_eq!(chunk.thinking, Some("Planning response...".to_string()));
    assert!(chunk.done);
}

/// Test parsing stream line with tool calls
#[test]
fn parse_stream_line_with_tool_calls() {
    let line = r#"{"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"read_file","arguments":{"path":"test.rs"}}}]},"done":false}"#;

    let result = parse_stream_line(line).expect("should parse");
    let chunk = result.expect("should have chunk");

    assert_eq!(chunk.content, "");
    assert_eq!(chunk.tool_calls.len(), 1);
    assert_eq!(
        chunk.tool_calls[0].function.as_ref().unwrap().name,
        "read_file"
    );
    assert!(!chunk.done);
}

/// Test backward compatible parse - missing fields should not fail
#[test]
fn parse_stream_line_backward_compatible() {
    // Minimal response without thinking or tool_calls
    let line = r#"{"message":{"role":"assistant","content":"Hello"},"done":true}"#;

    let result = parse_stream_line(line).expect("should parse");
    let chunk = result.expect("should have chunk");

    assert_eq!(chunk.content, "Hello");
    assert!(chunk.thinking.is_none());
    assert!(chunk.tool_calls.is_empty());
    assert!(chunk.done);
}

/// Test parse handles unknown fields gracefully (defensive parsing)
#[test]
fn parse_stream_line_unknown_fields() {
    let line = r#"{"message":{"role":"assistant","content":"Hello","unknown_field":"ignored"},"done":true,"extra_meta":"ignored"}"#;

    let result = parse_stream_line(line).expect("should parse despite unknown fields");
    let chunk = result.expect("should have chunk");

    assert_eq!(chunk.content, "Hello");
    assert!(chunk.done);
}

/// Test empty line parsing
#[test]
fn parse_stream_line_empty() {
    let result = parse_stream_line("").expect("should not error");
    assert!(result.is_none());
}

/// Test whitespace-only line parsing
#[test]
fn parse_stream_line_whitespace() {
    let result = parse_stream_line("   \n\t  ").expect("should not error");
    assert!(result.is_none());
}

/// Test ChatToolCall deserialization with missing function
#[test]
fn chat_tool_call_missing_function() {
    let json_str = r#"{"function":null}"#;
    let call: ChatToolCall = serde_json::from_str(json_str).expect("should deserialize");
    assert!(call.function.is_none());
}

/// Test ChatToolCall deserialization with full function
#[test]
fn chat_tool_call_full_function() {
    let json_str = r#"{"function":{"name":"test","arguments":{"key":"value"}}}"#;
    let call: ChatToolCall = serde_json::from_str(json_str).expect("should deserialize");

    let func = call.function.expect("should have function");
    assert_eq!(func.name, "test");
    assert_eq!(func.arguments, json!({"key":"value"}));
}
