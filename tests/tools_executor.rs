use olly::app::tools::executor::{execute_tool, ToolContext};
use olly::app::tools::ToolResultStatus;
use serde_json::json;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use tokio::fs;

/// Create a temp directory with test files
async fn setup_test_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("should create temp dir");

    // Create test files
    fs::write(dir.path().join("test.txt"), "Hello World\nLine 2\nLine 3")
        .await
        .expect("should write test file");

    fs::write(
        dir.path().join("code.rs"),
        "fn main() {\n    println!(\"Hello\");\n}",
    )
    .await
    .expect("should write rust file");

    // Create subdirectory
    fs::create_dir(dir.path().join("subdir"))
        .await
        .expect("should create subdir");

    fs::write(dir.path().join("subdir/nested.txt"), "Nested content")
        .await
        .expect("should write nested file");

    dir
}

/// Test read_file with valid file
#[tokio::test]
async fn read_file_reads_content() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("read_file", json!({"path": "test.txt"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("Hello World"));
    assert!(result.output.contains("Line 2"));
}

/// Test read_file with offset and limit
#[tokio::test]
async fn read_file_respects_offset_and_limit() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "read_file",
        json!({"path": "test.txt", "offset": 1, "limit": 1}),
        &ctx,
    )
    .await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(!result.output.contains("Hello World")); // First line skipped
    assert!(result.output.contains("Line 2"));
    assert!(!result.output.contains("Line 3")); // Beyond limit
}

/// Test read_file returns error for non-existent file
#[tokio::test]
async fn read_file_error_for_missing_file() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("read_file", json!({"path": "nonexistent.txt"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "not_found");
}

/// Test read_file returns error for directory
#[tokio::test]
async fn read_file_error_for_directory() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("read_file", json!({"path": "subdir"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "is_directory");
}

/// Test read_file enforces workspace boundary
#[tokio::test]
async fn read_file_enforces_workspace_boundary() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Try to read outside workspace
    let result = execute_tool("read_file", json!({"path": "../outside.txt"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "path_denied");
}

/// Test list_dir lists contents
#[tokio::test]
async fn list_dir_lists_files_and_directories() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("list_dir", json!({"path": "."}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("file: test.txt"));
    assert!(result.output.contains("file: code.rs"));
    assert!(result.output.contains("dir: subdir"));
}

/// Test list_dir returns error for non-existent directory
#[tokio::test]
async fn list_dir_error_for_missing_directory() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("list_dir", json!({"path": "nonexistent"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "not_found");
}

/// Test list_dir returns error for file
#[tokio::test]
async fn list_dir_error_for_file() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("list_dir", json!({"path": "test.txt"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "not_directory");
}

/// Test grep_search finds matches
#[tokio::test]
async fn grep_search_finds_matches() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("grep_search", json!({"query": "Hello"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("test.txt:1: Hello World"));
    assert!(result.output.contains("code.rs:2:     println!(\"Hello\")"));
}

/// Test grep_search with case sensitivity
#[tokio::test]
async fn grep_search_case_insensitive() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "grep_search",
        json!({"query": "hello", "case_sensitive": false}),
        &ctx,
    )
    .await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("test.txt:1:")); // Matches "Hello"
}

/// Test grep_search with no matches
#[tokio::test]
async fn grep_search_no_matches() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("grep_search", json!({"query": "xyz123"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("No matches found"));
}

/// Test grep_search enforces workspace boundary
#[tokio::test]
async fn grep_search_enforces_workspace_boundary() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "grep_search",
        json!({"query": "test", "path": "../outside"}),
        &ctx,
    )
    .await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "path_denied");
}

/// Test find_by_name finds files
#[tokio::test]
async fn find_by_name_finds_files() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("find_by_name", json!({"pattern": "*.txt"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("test.txt"));
    assert!(result.output.contains("subdir/nested.txt"));
    assert!(!result.output.contains("code.rs"));
}

/// Test find_by_name with specific name
#[tokio::test]
async fn find_by_name_specific_file() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("find_by_name", json!({"pattern": "code.rs"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("code.rs"));
}

/// Test find_by_name with no matches
#[tokio::test]
async fn find_by_name_no_matches() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("find_by_name", json!({"pattern": "*.py"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("No files matching '*.py' found"));
}

/// Test find_by_name enforces workspace boundary
#[tokio::test]
async fn find_by_name_enforces_workspace_boundary() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "find_by_name",
        json!({"pattern": "*", "path": "../outside"}),
        &ctx,
    )
    .await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "path_denied");
}

/// Test read_file with binary content
#[tokio::test]
async fn read_file_handles_binary() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Write binary content with null bytes
    let binary_content = vec![0u8, 1, 2, 0, 3, 4];
    fs::write(dir.path().join("binary.bin"), binary_content)
        .await
        .expect("should write binary file");

    let result = execute_tool("read_file", json!({"path": "binary.bin"}), &ctx).await;

    assert!(result.status == ToolResultStatus::Success);
    assert!(result.output.contains("[Binary file:"));
}

/// Test unknown tool returns error
#[tokio::test]
async fn unknown_tool_returns_error() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("nonexistent_tool", json!({}), &ctx).await;

    assert!(result.status == ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "unknown_tool");
}

/// Test missing required argument returns error
#[tokio::test]
async fn missing_required_argument_returns_error() {
    let dir = setup_test_dir().await;
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("read_file", json!({}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_ref().unwrap(), "invalid_args");
}

/// Test ToolContext without workspace root denies all file operations
#[tokio::test]
async fn no_workspace_denies_file_operations() {
    let dir = setup_test_dir().await;
    // Create context with no workspace root
    let ctx = ToolContext::new(None, dir.path().to_path_buf());

    let result = execute_tool("read_file", json!({"path": "test.txt"}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_ref().unwrap(), "path_denied");
}

#[cfg(unix)]
#[tokio::test]
async fn grep_search_does_not_follow_symlink_outside_workspace() {
    let workspace = tempfile::tempdir().expect("should create workspace dir");
    let outside = tempfile::tempdir().expect("should create outside dir");

    fs::write(workspace.path().join("local.txt"), "local content")
        .await
        .expect("should write local file");
    fs::write(
        outside.path().join("secret.txt"),
        "TOP_SECRET_PATTERN_12345 outside",
    )
    .await
    .expect("should write outside file");
    unix_fs::symlink(outside.path(), workspace.path().join("outside_link"))
        .expect("should create symlink");

    let ctx = ToolContext::new(
        Some(workspace.path().to_path_buf()),
        workspace.path().to_path_buf(),
    );
    let result = execute_tool(
        "grep_search",
        json!({"query":"TOP_SECRET_PATTERN_12345", "path":"."}),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Success);
    assert!(!result.output.contains("TOP_SECRET_PATTERN_12345"));
    assert!(!result.output.contains("outside_link/secret.txt"));
}

#[cfg(unix)]
#[tokio::test]
async fn find_by_name_does_not_follow_symlink_outside_workspace() {
    let workspace = tempfile::tempdir().expect("should create workspace dir");
    let outside = tempfile::tempdir().expect("should create outside dir");

    fs::write(workspace.path().join("local.txt"), "local content")
        .await
        .expect("should write local file");
    fs::write(outside.path().join("secret.txt"), "outside content")
        .await
        .expect("should write outside file");
    unix_fs::symlink(outside.path(), workspace.path().join("outside_link"))
        .expect("should create symlink");

    let ctx = ToolContext::new(
        Some(workspace.path().to_path_buf()),
        workspace.path().to_path_buf(),
    );
    let result = execute_tool("find_by_name", json!({"pattern":"*.txt", "path":"."}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Success);
    assert!(result.output.contains("local.txt"));
    assert!(!result.output.contains("outside_link/secret.txt"));
}
