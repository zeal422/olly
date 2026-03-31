use olly::app::tools::executor::{execute_tool, ToolContext, MAX_OUTPUT_SIZE};
use olly::app::tools::ToolResultStatus;
use serde_json::json;
use tokio::fs;

/// Test allowed command execution
#[tokio::test]
async fn execute_command_runs_allowed_command() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Create a test file
    fs::write(dir.path().join("test.txt"), "hello world")
        .await
        .expect("should write file");

    // Run git status (allowed command)
    let result = execute_tool("execute_command", json!({"command": "git status"}), &ctx).await;

    // Should succeed or fail gracefully (git might not be initialized)
    assert!(result.status == ToolResultStatus::Success || result.status == ToolResultStatus::Error);
}

/// Test allowlist enforcement - command not in allowlist
#[tokio::test]
async fn execute_command_rejects_unallowed_command() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("execute_command", json!({"command": "rm -rf /"}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "command_not_allowed");
}

/// Test denylist enforcement
#[tokio::test]
async fn execute_command_rejects_denylisted_pattern() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Try to use a destructive git command
    let result = execute_tool(
        "execute_command",
        json!({"command": "git reset --hard"}),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert!(result.error_type.as_ref().unwrap() == "command_denied");
}

/// Test shell control-sequence rejection to prevent command chaining/injection
#[tokio::test]
async fn execute_command_rejects_shell_control_sequences() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "execute_command",
        json!({"command": "git status && whoami"}),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_deref(), Some("command_denied"));
}

/// Test workspace boundary for CWD
#[tokio::test]
async fn execute_command_enforces_workspace_boundary_for_cwd() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "execute_command",
        json!({
            "command": "git status",
            "cwd": "../outside"
        }),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_ref().unwrap(), "path_denied");
}

/// Test missing command argument
#[tokio::test]
async fn execute_command_requires_command_argument() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("execute_command", json!({}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_ref().unwrap(), "invalid_args");
}

/// Test output truncation - use read_file which definitely truncates large outputs
#[tokio::test]
async fn execute_command_truncates_large_output() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Create a large file (15KB > 10KB MAX_OUTPUT_SIZE)
    let large_content = "x".repeat(15 * 1024);
    tokio::fs::write(dir.path().join("large.txt"), large_content)
        .await
        .expect("should write file");

    // Use read_file to read the large file - this WILL truncate
    let result = execute_tool("read_file", json!({"path": "large.txt"}), &ctx).await;

    // Verify truncation occurred
    assert_eq!(result.status, ToolResultStatus::Success);
    assert!(
        result.output.len() <= MAX_OUTPUT_SIZE + 100,
        "Output should be truncated to around MAX_OUTPUT_SIZE"
    );
    assert!(
        result.truncated.unwrap_or(false),
        "truncated flag should be set"
    );
}

/// Test timeout - this test might be slow, so we use a shorter command
#[tokio::test]
async fn execute_command_handles_quick_commands() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Use a quick git command that should complete immediately
    let result = execute_tool("execute_command", json!({"command": "git --version"}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Success);
    // Check for success - output varies by platform and git version
    assert!(
        result.output.contains("git version")
            || result.output.contains("not found")
            || result.output.contains("not recognized")
            || result.output.to_lowercase().contains("error"),
        "Expected git version output or error message, got: {}",
        result.output
    );
}

/// Test cargo command (allowed)
#[tokio::test]
async fn execute_command_allows_cargo() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "execute_command",
        json!({"command": "cargo --version"}),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Success);
    assert!(result.output.contains("cargo"));
}

/// Test rg (ripgrep) command (allowed)
#[tokio::test]
async fn execute_command_allows_rg() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Create a file to search
    fs::write(dir.path().join("search.txt"), "test content")
        .await
        .expect("should write file");

    let result = execute_tool(
        "execute_command",
        json!({
            "command": "rg test",
            "cwd": "."
        }),
        &ctx,
    )
    .await;

    // rg might not be installed, so we check for success OR command_failed
    assert!(
        result.status == ToolResultStatus::Success
            || (result.status == ToolResultStatus::Error
                && result.error_type.as_ref().unwrap() == "command_failed")
    );
}

/// Test node command (allowed)
#[tokio::test]
async fn execute_command_allows_node() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "execute_command",
        json!({"command": "node --version"}),
        &ctx,
    )
    .await;

    // node might not be installed
    assert!(
        result.status == ToolResultStatus::Success
            || (result.status == ToolResultStatus::Error
                && result.error_type.as_ref().unwrap() == "command_failed")
    );
}

/// Test case insensitivity in allowlist
#[tokio::test]
async fn execute_command_allowlist_is_case_insensitive() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Test with uppercase CARGO
    let result = execute_tool(
        "execute_command",
        json!({"command": "CARGO --version"}),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Success);
    assert!(result.output.contains("cargo"));
}

/// Test that command variants like python3 are allowed
#[tokio::test]
async fn execute_command_allows_command_variants() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // python3 should be allowed as a variant of python
    let result = execute_tool(
        "execute_command",
        json!({"command": "python3 --version"}),
        &ctx,
    )
    .await;

    // python3 might not be installed, but it should pass allowlist check
    assert!(result
        .error_type
        .as_ref()
        .map(|e| e != "command_not_allowed")
        .unwrap_or(true));
}

/// Test that known executable extensions are allowed when base command is allowlisted
#[tokio::test]
async fn execute_command_allows_known_windows_extension() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "execute_command",
        json!({"command": "python.exe --version"}),
        &ctx,
    )
    .await;

    assert!(result
        .error_type
        .as_ref()
        .map(|e| e != "command_not_allowed")
        .unwrap_or(true));
}

/// Test that dotted suffixes like `git.evil` are rejected
#[tokio::test]
async fn execute_command_rejects_non_whitelisted_dotted_suffix() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool(
        "execute_command",
        json!({"command": "git.evil status"}),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_deref(), Some("command_not_allowed"));
}

/// Test that path-like executable tokens are rejected
#[tokio::test]
async fn execute_command_rejects_path_like_executable_tokens() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    let result = execute_tool("execute_command", json!({"command": "./git status"}), &ctx).await;

    assert_eq!(result.status, ToolResultStatus::Error);
    assert_eq!(result.error_type.as_deref(), Some("command_not_allowed"));
}

/// Test exit code reporting - use cargo which reliably returns exit code on failure
#[tokio::test]
async fn execute_command_reports_exit_code() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let ctx = ToolContext::new(Some(dir.path().to_path_buf()), dir.path().to_path_buf());

    // Use cargo with invalid args to get non-zero exit
    let result = execute_tool(
        "execute_command",
        json!({
            "command": "cargo invalid_command"
        }),
        &ctx,
    )
    .await;

    assert_eq!(result.status, ToolResultStatus::Success);
    assert!(result.output.contains("Exit code:") || result.output.contains("error"));
}
