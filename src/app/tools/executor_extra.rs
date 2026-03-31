
/// Command timeout in seconds
const COMMAND_TIMEOUT_SECS: u64 = 60;

/// Allowed command roots (must start with one of these)
const ALLOWLIST: &[&str] = &["cargo", "git", "rg", "python", "node"];

/// Denied command patterns (case-insensitive substring match)
const DENYLIST: &[&str] = &[
    "rm ", "rm\t",
    "del ", "del\t",
    "rmdir", "rd ",
    "format", "fdisk", "dd if=",
    "mkfs", "mkswap", "swapon",
    "git reset --hard", "git clean -fd",
    "git push --force", "git push -f",
    "> /dev/null", ">/dev/null",
    "2>&1 |", "| sh", "| bash",
    "curl |", "wget |",
    "sudo ", "su -", "doas ",
    "chown ", "chmod 777", "chmod -R 777",
];

/// Execute command tool implementation with security controls
async fn execute_command(args: Value, context: &ToolContext) -> ToolResult {
    let Some(command_str) = args.get("command").and_then(|c| c.as_str()) else {
        return ToolResult::error("invalid_args", "Missing required 'command' argument");
    };

    // Get working directory
    let cwd_str = args.get("cwd").and_then(|c| c.as_str()).unwrap_or(".");
    let cwd = context.resolve_path(cwd_str);

    // Check workspace boundary for CWD
    if let Some(ref root) = context.workspace_root {
        if !context.is_within_workspace(&cwd) {
            return ToolResult::error(
                "path_denied",
                format!("Working directory '{}' is outside workspace root '{}'", cwd.display(), root.display()),
            );
        }
    }

    // Trim command
    let command_str = command_str.trim();

    // Check allowlist
    let first_token = command_str.split_whitespace().next().unwrap_or("");
    let is_allowed = ALLOWLIST.iter().any(|&allowed| {
        first_token.eq_ignore_ascii_case(allowed) ||
        first_token.to_lowercase().starts_with(&format!("{}.", allowed))
    });

    if !is_allowed {
        return ToolResult::error(
            "command_not_allowed",
            format!(
                "Command '{}' is not in allowlist. Allowed commands: {}",
                first_token,
                ALLOWLIST.join(", ")
            ),
        );
    }

    // Check denylist
    let command_lower = command_str.to_lowercase();
    for pattern in DENYLIST {
        if command_lower.contains(pattern) {
            return ToolResult::error(
                "command_denied",
                format!("Command contains denied pattern: '{}'", pattern.trim()),
            );
        }
    }

    // Execute command with timeout
    let output = match tokio::time::timeout(
        std::time::Duration::from_secs(COMMAND_TIMEOUT_SECS),
        run_command(command_str, &cwd)
    ).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return ToolResult::error("command_failed", format!("Failed to execute command: {}", e));
        }
        Err(_) => {
            return ToolResult::error("timeout", format!("Command timed out after {} seconds", COMMAND_TIMEOUT_SECS));
        }
    };

    // Combine stdout and stderr
    let mut combined = String::new();
    if !output.stdout.is_empty() {
        combined.push_str(&output.stdout);
    }
    if !output.stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str("[stderr]: ");
        combined.push_str(&output.stderr);
    }

    // Apply size limit
    let was_truncated = combined.len() > MAX_OUTPUT_SIZE;
    if was_truncated {
        combined.truncate(MAX_OUTPUT_SIZE);
        combined.push_str("\n\n[Output truncated: exceeded size limit]");
    }

    // Include exit code if non-zero
    let result = if output.status.success() {
        combined
    } else {
        format!("[Exit code: {}]\n{}",
            output.status.code().unwrap_or(-1),
            combined
        )
    };

    let mut tool_result = ToolResult::success(result);
    if was_truncated {
        tool_result = tool_result.truncated();
    }
    tool_result
}

/// Run a shell command and capture output
async fn run_command(command: &str, cwd: &Path) -> Result<std::process::Output, std::io::Error> {
    // Use cmd on Windows, sh on Unix
    #[cfg(windows)]
    let mut cmd = tokio::process::Command::new("cmd");
    #[cfg(windows)]
    cmd.arg("/C").arg(command);

    #[cfg(not(windows))]
    let mut cmd = tokio::process::Command::new("sh");
    #[cfg(not(windows))]
    cmd.arg("-c").arg(command);

    cmd.current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let output = cmd.output().await?;
    Ok(output)
}
