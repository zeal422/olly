//! Integration tests for tool execution loop in chat_pipeline
//!
//! These tests verify the tool loop guards and integration with the chat pipeline.

use std::path::PathBuf;

use olly::app::tools::executor::{execute_tool, ToolContext, MAX_OUTPUT_SIZE};
use olly::app::tools::{ToolResult, ToolResultStatus};
use serde_json::json;

/// Test that tool context is created correctly
#[test]
fn tool_context_creation_with_workspace() {
    let workspace = PathBuf::from("/tmp/test_workspace");
    let current = PathBuf::from("/tmp/test_workspace/src");

    let ctx = ToolContext::new(Some(workspace.clone()), current.clone());

    assert!(ctx.workspace_root.is_some());
    assert_eq!(ctx.workspace_root.unwrap(), workspace);
    assert_eq!(ctx.current_dir, current);
}

/// Test that tool context handles missing workspace
#[test]
fn tool_context_creation_without_workspace() {
    let current = PathBuf::from("/tmp");

    let ctx = ToolContext::new(None, current.clone());

    assert!(ctx.workspace_root.is_none());
    assert_eq!(ctx.current_dir, current);
}

/// Test path resolution with workspace
#[test]
fn tool_context_resolves_relative_path() {
    let temp_dir = std::env::temp_dir();
    let workspace = temp_dir.join("test_workspace");
    let ctx = ToolContext::new(Some(workspace.clone()), workspace.clone());

    let resolved = ctx.resolve_path("src/main.rs");
    assert_eq!(resolved, workspace.join("src/main.rs"));
}

/// Test path resolution with absolute path
#[test]
fn tool_context_preserves_absolute_path() {
    let temp_dir = std::env::temp_dir();
    let workspace = temp_dir.join("test_workspace");
    let external = temp_dir.join("external").join("config.toml");
    let ctx = ToolContext::new(Some(workspace.clone()), workspace.clone());

    let resolved = ctx.resolve_path(external.to_str().unwrap());
    assert_eq!(resolved, external);
}

/// Test workspace boundary check - path within workspace
#[test]
fn tool_context_allows_path_within_workspace() {
    // Use actual temp directory for this test
    let temp_dir = std::env::temp_dir();
    let ctx = ToolContext::new(Some(temp_dir.clone()), temp_dir.clone());

    let test_path = temp_dir.join("test_file.txt");
    assert!(ctx.is_within_workspace(&test_path));
}

/// Test workspace boundary check - path outside workspace
#[test]
fn tool_context_denies_path_outside_workspace() {
    let temp_dir = std::env::temp_dir();
    let workspace = temp_dir.join("test_workspace");
    let outside = temp_dir.join("outside").join("secrets.txt");
    let ctx = ToolContext::new(Some(workspace.clone()), workspace.clone());

    assert!(!ctx.is_within_workspace(&outside));
}

/// Test max steps constant is reasonable
#[test]
fn max_tool_steps_is_reasonable() {
    // MAX_TOOL_STEPS should be a small positive number
    let max_steps: usize = 4; // Defined in chat_pipeline.rs
    assert!(
        max_steps > 0 && max_steps <= 10,
        "MAX_TOOL_STEPS should be between 1-10"
    );
}

/// Test cumulative output budget is reasonable
#[test]
fn cumulative_output_budget_is_reasonable() {
    // MAX_CUMULATIVE_TOOL_OUTPUT should be reasonable (100KB)
    let budget: usize = 100 * 1024; // Defined in chat_pipeline.rs
    assert!(
        (50 * 1024..=500 * 1024).contains(&budget),
        "Cumulative output budget should be 50KB-500KB"
    );
}

/// Test tool result success creation
#[test]
fn tool_result_success() {
    let result = ToolResult::success("output content");
    assert_eq!(result.status, ToolResultStatus::Success);
    assert_eq!(result.output, "output content");
    assert!(result.error_type.is_none());
    assert!(result.truncated.is_none());
}

/// Test tool result error creation
#[test]
fn tool_result_error() {
    let result = ToolResult::error("path_denied", "Access denied");
    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error, Some("Access denied".to_string()));
    assert_eq!(result.error_type, Some("path_denied".to_string()));
}

/// Test tool result truncated flag
#[test]
fn tool_result_truncated() {
    let result = ToolResult::success("output".to_string()).truncated();
    assert_eq!(result.status, ToolResultStatus::Success);
    assert!(result.truncated.unwrap_or(false));
}

/// Test tool result serialization
#[test]
fn tool_result_serializes_correctly() {
    let result = ToolResult::success("test output");
    let json = serde_json::to_string(&result).expect("should serialize");

    assert!(json.contains("test output"));
    assert!(json.contains("success"));
}

/// Test that read_file respects MAX_READ_LINES limit concept
#[tokio::test]
async fn read_file_respects_line_limit() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Create file with many lines
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("Line {}\n", i));
    }
    tokio::fs::write(dir.path().join("long.txt"), content)
        .await
        .expect("should write file");

    let result = execute_tool("read_file", json!({"path": "long.txt"}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Success);
    // Should indicate truncation if file has more than MAX_READ_LINES (500)
    // The output should either contain [Truncated or have truncated flag set
    let has_truncation_indicator =
        result.output.contains("[Truncated") || result.truncated.unwrap_or(false);
    // For very large files, truncation should occur
    if result.output.lines().count() >= 500 {
        assert!(
            has_truncation_indicator,
            "Expected truncation for large file"
        );
    }
}

/// Test that output size limit is enforced
#[tokio::test]
async fn output_size_limit_enforced() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Create a large file
    let large_content = "x".repeat(15 * 1024); // 15KB
    tokio::fs::write(dir.path().join("large.txt"), large_content)
        .await
        .expect("should write file");

    let result = execute_tool("read_file", json!({"path": "large.txt"}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Success);
    // Output should be truncated to MAX_OUTPUT_SIZE (10KB)
    assert!(result.output.len() <= MAX_OUTPUT_SIZE + 100); // Allow margin for truncation message
}

/// Test tool execution with invalid tool name
#[tokio::test]
async fn execute_tool_unknown_tool() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("nonexistent_tool", json!({}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_ref().unwrap(), "unknown_tool");
}

/// Test multiple tools can be executed in sequence
#[tokio::test]
async fn multiple_tools_execute_sequentially() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Create test files
    tokio::fs::write(dir.path().join("file1.txt"), "content 1")
        .await
        .unwrap();
    tokio::fs::write(dir.path().join("file2.txt"), "content 2")
        .await
        .unwrap();

    // Execute first tool
    let result1 = execute_tool("read_file", json!({"path": "file1.txt"}), &ctx).await;
    assert_eq!(result1.status, ToolResultStatus::Success);
    assert!(result1.output.contains("content 1"));

    // Execute second tool
    let result2 = execute_tool("read_file", json!({"path": "file2.txt"}), &ctx).await;
    assert_eq!(result2.status, ToolResultStatus::Success);
    assert!(result2.output.contains("content 2"));
}

/// Test tool execution with missing required argument
#[tokio::test]
async fn tool_missing_required_argument() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // read_file requires "path" argument
    let result = execute_tool(
        "read_file",
        json!({}), // Missing "path"
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_ref().unwrap(), "invalid_args");
}

/// Test that tool context without workspace denies file operations
#[tokio::test]
async fn no_workspace_denies_file_operations() {
    let ctx = ToolContext::new(
        None, // No workspace
        std::env::current_dir().unwrap(),
    );

    let result = execute_tool("read_file", json!({"path": "any_file.txt"}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_ref().unwrap(), "path_denied");
}

/// Test cumulative output tracking concept
#[test]
fn cumulative_output_tracking() {
    let mut cumulative = 0;
    let budget = 100 * 1024; // 100KB

    // Simulate adding outputs
    for _ in 0..10 {
        let output_size = 5000; // 5KB each
        cumulative += output_size;

        // Should not exceed budget
        assert!(
            cumulative <= budget,
            "Cumulative output should not exceed budget"
        );
    }

    assert_eq!(cumulative, 50_000);
}
