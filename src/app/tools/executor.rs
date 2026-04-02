//! Tool executor - dispatches tool calls to implementations

use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;
use tokio::fs;

use super::ToolResult;

/// Maximum output size for tool results (10KB)
pub const MAX_OUTPUT_SIZE: usize = 10 * 1024;

/// Maximum lines to return for read_file
pub const MAX_READ_LINES: usize = 500;

/// Maximum matches for grep_search
pub const MAX_GREP_MATCHES: usize = 50;

/// Truncate a string to at most `max_bytes` bytes, ensuring we land on a valid UTF-8 character boundary.
/// Returns true if truncation occurred, false otherwise.
pub(crate) fn utf8_truncate(s: &mut String, max_bytes: usize) -> bool {
    if s.len() <= max_bytes {
        return false;
    }
    let mut truncate_at = max_bytes;
    while truncate_at > 0 && !s.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }
    s.truncate(truncate_at);
    true
}

/// Context for tool execution
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub workspace_root: Option<PathBuf>,
    pub current_dir: PathBuf,
}

impl ToolContext {
    pub fn new(workspace_root: Option<PathBuf>, current_dir: PathBuf) -> Self {
        Self {
            workspace_root,
            current_dir,
        }
    }

    /// Resolve a path relative to workspace root or current directory
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(ref root) = self.workspace_root {
            root.join(path)
        } else {
            self.current_dir.join(path)
        }
    }

    /// Check if a path is within workspace boundaries
    pub fn is_within_workspace(&self, path: &Path) -> bool {
        let Some(ref root) = self.workspace_root else {
            return false;
        };

        // Try canonicalization first for existing paths
        if let Ok(canonical_path) = path.canonicalize() {
            if let Ok(canonical_root) = root.canonicalize() {
                return canonical_path.starts_with(&canonical_root);
            }
        }

        // For non-existent paths, clean up the path and compare
        let cleaned_path = self.clean_path(path);
        let cleaned_root = self.clean_path(root);
        cleaned_path.starts_with(&cleaned_root)
    }

    /// Clean up a path by resolving . and .. components
    /// Handles Windows UNC paths and root prefixes correctly
    fn clean_path(&self, path: &Path) -> PathBuf {
        use std::path::Component;

        let mut result = PathBuf::new();
        let mut components = path.components().peekable();

        // Handle Windows prefix (UNC paths like \\?\ or drive letters like C:)
        if let Some(Component::Prefix(prefix)) = components.peek() {
            result.push(prefix.as_os_str());
            components.next();
        }

        // Process remaining components
        let mut stack: Vec<std::ffi::OsString> = Vec::new();
        for component in components {
            match component {
                Component::Normal(name) => {
                    stack.push(name.to_os_string());
                }
                Component::ParentDir => {
                    // Preserve excess ".." when stack is empty or top is also ".."
                    // This ensures path escape attempts are honestly represented
                    if stack.last().is_some_and(|s| s != "..") {
                        stack.pop();
                    } else {
                        stack.push(std::ffi::OsString::from(".."));
                    }
                }
                Component::CurDir => {}
                Component::RootDir => {
                    // Reset stack and explicitly preserve root separator in result
                    stack.clear();
                    result.push(std::path::Component::RootDir.as_os_str());
                }
                Component::Prefix(_) => {
                    // Already handled above, skip any subsequent prefixes
                }
            }
        }

        // Build final path
        for comp in stack {
            result.push(comp);
        }

        result
    }
}

/// Execute a tool call and return the result
pub async fn execute_tool(tool_name: &str, arguments: Value, context: &ToolContext) -> ToolResult {
    match tool_name {
        "read_file" => read_file(arguments, context).await,
        "list_dir" => list_dir(arguments, context).await,
        "grep_search" => grep_search(arguments, context).await,
        "find_by_name" => find_by_name(arguments, context).await,
        "execute_command" => execute_command(arguments, context).await,
        _ => ToolResult::error("unknown_tool", format!("Unknown tool: {}", tool_name)),
    }
}

/// Read file tool implementation
async fn read_file(args: Value, context: &ToolContext) -> ToolResult {
    let Some(path_str) = args.get("path").and_then(|p| p.as_str()) else {
        return ToolResult::error("invalid_args", "Missing required 'path' argument");
    };

    let path = context.resolve_path(path_str);

    // Check workspace boundary - deny if no workspace or path outside workspace
    if !context.is_within_workspace(&path) {
        let root_msg = context
            .workspace_root
            .as_ref()
            .map(|r| format!("'{}'", r.display()))
            .unwrap_or_else(|| "(no workspace set)".to_string());
        return ToolResult::error(
            "path_denied",
            format!(
                "Path '{}' is outside workspace root {}",
                path.display(),
                root_msg
            ),
        );
    }

    // Check if file exists
    let metadata = match fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) => {
            return ToolResult::error(
                "not_found",
                format!("Cannot access '{}': {}", path.display(), e),
            );
        }
    };

    // Reject directories
    if metadata.is_dir() {
        return ToolResult::error(
            "is_directory",
            format!("'{}' is a directory, not a file", path.display()),
        );
    }

    // Handle binary files - check if content looks like binary
    let content = match fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return ToolResult::error(
                "read_error",
                format!("Failed to read '{}': {}", path.display(), e),
            );
        }
    };

    // Check for binary content (null bytes or high ratio of non-printable chars)
    if is_binary_content(&content) {
        let content_len = content.len();
        return ToolResult::success(format!("[Binary file: {} bytes]", content_len));
    }

    // Convert to string
    let content_len = content.len();
    let text = match String::from_utf8(content) {
        Ok(t) => t,
        Err(_) => {
            return ToolResult::success(format!("[Non-UTF8 file: {} bytes]", content_len));
        }
    };

    // Apply offset and limit
    let offset = args.get("offset").and_then(|o| o.as_u64()).unwrap_or(0) as usize;
    let limit = args
        .get("limit")
        .and_then(|l| l.as_u64())
        .map(|l| l.min(MAX_READ_LINES as u64) as usize)
        .unwrap_or(MAX_READ_LINES);

    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    if offset >= total_lines {
        return ToolResult::success("[Empty: offset beyond file length]");
    }

    let end = (offset + limit).min(total_lines);
    let selected_lines = &lines[offset..end];

    let result = selected_lines.join("\n");
    let truncated = end < total_lines;

    let mut output = result;
    if truncated {
        output.push_str(&format!(
            "\n\n[Truncated: {} of {} lines shown]",
            end - offset,
            total_lines
        ));
    }

    // Apply overall size limit
    if output.len() > MAX_OUTPUT_SIZE {
        utf8_truncate(&mut output, MAX_OUTPUT_SIZE);
        output.push_str("\n\n[Output truncated: exceeded size limit]");
        return ToolResult::success(output).truncated();
    }

    ToolResult::success(output)
}

/// Check if content appears to be binary
fn is_binary_content(content: &[u8]) -> bool {
    if content.is_empty() {
        return false;
    }

    // Check for null bytes
    if content.contains(&0) {
        return true;
    }

    // Check ratio of non-printable characters
    let non_printable = content
        .iter()
        .filter(|&&b| b < 0x20 && b != 0x0a && b != 0x0d && b != 0x09)
        .count();

    let ratio = non_printable as f64 / content.len() as f64;
    ratio > 0.3 // More than 30% non-printable
}

/// List directory tool implementation
async fn list_dir(args: Value, context: &ToolContext) -> ToolResult {
    let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");

    let path = context.resolve_path(path_str);

    // Check workspace boundary - deny if no workspace or path outside workspace
    if !context.is_within_workspace(&path) {
        let root_msg = context
            .workspace_root
            .as_ref()
            .map(|r| format!("'{}'", r.display()))
            .unwrap_or_else(|| "(no workspace set)".to_string());
        return ToolResult::error(
            "path_denied",
            format!(
                "Path '{}' is outside workspace root {}",
                path.display(),
                root_msg
            ),
        );
    }

    // Check if path exists and is directory
    let metadata = match fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) => {
            return ToolResult::error(
                "not_found",
                format!("Cannot access '{}': {}", path.display(), e),
            );
        }
    };

    if !metadata.is_dir() {
        return ToolResult::error(
            "not_directory",
            format!("'{}' is not a directory", path.display()),
        );
    }

    // Read directory entries
    let mut entries = match fs::read_dir(&path).await {
        Ok(e) => e,
        Err(e) => {
            return ToolResult::error(
                "read_error",
                format!("Failed to read directory '{}': {}", path.display(), e),
            );
        }
    };

    let mut results = Vec::new();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        let entry_meta = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };

        let type_str = if entry_meta.is_dir() {
            "dir"
        } else if entry_meta.is_file() {
            "file"
        } else if entry_meta.is_symlink() {
            "symlink"
        } else {
            "other"
        };

        results.push(format!("{}: {}", type_str, name));
    }

    results.sort();

    let output = if results.is_empty() {
        "[Empty directory]".to_string()
    } else {
        results.join("\n")
    };

    if output.len() > MAX_OUTPUT_SIZE {
        let mut truncated = output;
        utf8_truncate(&mut truncated, MAX_OUTPUT_SIZE);
        truncated.push_str("\n\n[Output truncated: exceeded size limit]");
        return ToolResult::success(truncated).truncated();
    }

    ToolResult::success(output)
}

/// Grep search tool implementation
async fn grep_search(args: Value, context: &ToolContext) -> ToolResult {
    let Some(query) = args.get("query").and_then(|q| q.as_str()) else {
        return ToolResult::error("invalid_args", "Missing required 'query' argument");
    };

    let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|c| c.as_bool())
        .unwrap_or(false);

    let search_path = context.resolve_path(path_str);

    // Check workspace boundary - deny if no workspace or path outside workspace
    if !context.is_within_workspace(&search_path) {
        let root_msg = context
            .workspace_root
            .as_ref()
            .map(|r| format!("'{}'", r.display()))
            .unwrap_or_else(|| "(no workspace set)".to_string());
        return ToolResult::error(
            "path_denied",
            format!(
                "Path '{}' is outside workspace root {}",
                search_path.display(),
                root_msg
            ),
        );
    }

    // Compile regex
    let regex_builder = if case_sensitive {
        Regex::new(query)
    } else {
        Regex::new(&format!("(?i){}", query))
    };

    let regex = match regex_builder {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::error(
                "invalid_regex",
                format!("Invalid regex pattern '{}': {}", query, e),
            );
        }
    };

    // Determine search scope
    let files_to_search = if search_path.is_file() {
        vec![search_path]
    } else if search_path.is_dir() {
        match collect_text_files(&search_path, context).await {
            Ok(files) => files,
            Err(e) => {
                return ToolResult::error("read_error", format!("Failed to collect files: {}", e));
            }
        }
    } else {
        return ToolResult::error(
            "not_found",
            format!("Path '{}' does not exist", search_path.display()),
        );
    };

    let mut matches = Vec::new();
    let mut files_searched = 0;

    for file_path in files_to_search {
        // Skip binary files
        if let Ok(bytes) = fs::read(&file_path).await {
            if is_binary_content(&bytes) {
                continue;
            }
        }

        if let Ok(content) = fs::read_to_string(&file_path).await {
            files_searched += 1;

            // Process lines using split which is easier for borrow checker
            let lines: Vec<(usize, String)> = content
                .lines()
                .enumerate()
                .map(|(i, line)| (i, line.to_string()))
                .collect();

            for (line_num, line) in lines {
                if regex.is_match(&line) {
                    let relative_path = if let Some(ref root) = context.workspace_root {
                        file_path.strip_prefix(root).unwrap_or(&file_path)
                    } else {
                        &file_path
                    };

                    matches.push(format!(
                        "{}:{}: {}",
                        relative_path.display(),
                        line_num + 1,
                        line.chars().take(200).collect::<String>()
                    ));

                    if matches.len() >= MAX_GREP_MATCHES {
                        break;
                    }
                }
            }
        }

        if matches.len() >= MAX_GREP_MATCHES {
            break;
        }
    }

    let mut output = if matches.is_empty() {
        format!("No matches found (searched {} files)", files_searched)
    } else {
        let truncated = matches.len() >= MAX_GREP_MATCHES;
        let mut result = matches.join("\n");
        if truncated {
            result.push_str(&format!(
                "\n\n[Truncated: showing first {} matches]",
                MAX_GREP_MATCHES
            ));
        }
        result
    };

    if output.len() > MAX_OUTPUT_SIZE {
        utf8_truncate(&mut output, MAX_OUTPUT_SIZE);
        output.push_str("\n\n[Output truncated: exceeded size limit]");
        return ToolResult::success(output).truncated();
    }

    ToolResult::success(output)
}

/// Collect text files recursively (limited depth) - iterative to avoid async recursion issues
async fn collect_text_files(
    dir: &Path,
    context: &ToolContext,
) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    let mut dirs_to_process = vec![(dir.to_path_buf(), 0)]; // (path, depth)

    while let Some((current_dir, depth)) = dirs_to_process.pop() {
        if depth >= 5 {
            continue; // Depth limit reached
        }

        let mut entries = fs::read_dir(&current_dir).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !context.is_within_workspace(&path) {
                continue;
            }

            // Skip hidden files and common non-source directories
            if let Some(name) = path.file_name() {
                let name = name.to_string_lossy();
                if name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || name == "__pycache__"
                {
                    continue;
                }
            }

            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                if is_likely_text_file(&path) {
                    files.push(path);
                }
            } else if metadata.is_dir() {
                dirs_to_process.push((path, depth + 1));
            }
        }
    }

    Ok(files)
}

/// Check if a file is likely text based on extension
fn is_likely_text_file(path: &Path) -> bool {
    let text_extensions = [
        "txt",
        "rs",
        "md",
        "toml",
        "json",
        "yaml",
        "yml",
        "py",
        "js",
        "ts",
        "jsx",
        "tsx",
        "html",
        "css",
        "scss",
        "sass",
        "c",
        "cpp",
        "h",
        "hpp",
        "go",
        "java",
        "kt",
        "swift",
        "rb",
        "php",
        "sh",
        "bash",
        "zsh",
        "vim",
        "lua",
        "sql",
        "xml",
        "ini",
        "cfg",
        "conf",
        "Dockerfile",
        "Makefile",
        "cmake",
        "gradle",
        "properties",
        "env",
        "lock",
    ];

    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        return text_extensions.contains(&ext.as_str());
    }

    // No extension - check if it's a known file name
    if let Some(name) = path.file_name() {
        let name = name.to_string_lossy();
        return name == "Dockerfile" || name == "Makefile" || name == "README" || name == "LICENSE";
    }

    true // Default to including unknown files
}

/// Find by name tool implementation
async fn find_by_name(args: Value, context: &ToolContext) -> ToolResult {
    let Some(pattern) = args.get("pattern").and_then(|p| p.as_str()) else {
        return ToolResult::error("invalid_args", "Missing required 'pattern' argument");
    };

    let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
    let search_path = context.resolve_path(path_str);

    // Check workspace boundary - deny if no workspace or path outside workspace
    if !context.is_within_workspace(&search_path) {
        let root_msg = context
            .workspace_root
            .as_ref()
            .map(|r| format!("'{}'", r.display()))
            .unwrap_or_else(|| "(no workspace set)".to_string());
        return ToolResult::error(
            "path_denied",
            format!(
                "Path '{}' is outside workspace root {}",
                search_path.display(),
                root_msg
            ),
        );
    }

    if !search_path.exists() {
        return ToolResult::error(
            "not_found",
            format!("Path '{}' does not exist", search_path.display()),
        );
    }

    // Parse glob pattern
    let glob_pattern = match glob::Pattern::new(pattern) {
        Ok(p) => p,
        Err(e) => {
            return ToolResult::error(
                "invalid_pattern",
                format!("Invalid glob pattern '{}': {}", pattern, e),
            );
        }
    };

    let mut matches = Vec::new();

    match find_matching_files(&search_path, &glob_pattern, context, &mut matches).await {
        Ok(_) => {}
        Err(e) => {
            return ToolResult::error("search_error", format!("Failed to search: {}", e));
        }
    }

    matches.sort();
    matches.dedup();

    let output = if matches.is_empty() {
        format!("No files matching '{}' found", pattern)
    } else {
        matches.join("\n")
    };

    if output.len() > MAX_OUTPUT_SIZE {
        let mut truncated = output;
        utf8_truncate(&mut truncated, MAX_OUTPUT_SIZE);
        truncated.push_str(&format!(
            "\n\n[Truncated: showing first {} matches]",
            matches.len()
        ));
        return ToolResult::success(truncated).truncated();
    }

    ToolResult::success(output)
}

/// Recursively find files matching pattern - iterative to avoid async recursion issues
async fn find_matching_files(
    start_dir: &Path,
    pattern: &glob::Pattern,
    context: &ToolContext,
    matches: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    let mut dirs_to_process = vec![(start_dir.to_path_buf(), 0)]; // (path, depth)

    while let Some((current_dir, depth)) = dirs_to_process.pop() {
        if depth >= 5 || matches.len() >= 1000 {
            continue; // Limits reached
        }

        let mut entries = fs::read_dir(&current_dir).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !context.is_within_workspace(&path) {
                continue;
            }

            // Skip hidden files and common directories
            if let Some(name) = path.file_name() {
                let name = name.to_string_lossy();
                if name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || name == "__pycache__"
                {
                    continue;
                }
            }

            let metadata = entry.metadata().await?;
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            if pattern.matches(&file_name) {
                let relative_path = if let Some(ref root) = context.workspace_root {
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .display()
                        .to_string()
                } else {
                    path.display().to_string()
                };
                // Normalize path separators to forward slashes for consistency
                let normalized = relative_path.replace('\\', "/");
                matches.push(normalized);
            }

            if metadata.is_dir() {
                dirs_to_process.push((path, depth + 1));
            }
        }
    }

    Ok(())
}

/// Command timeout in seconds
const COMMAND_TIMEOUT_SECS: u64 = 60;

/// Allowed command roots (must start with one of these)
const ALLOWLIST: &[&str] = &["cargo", "git", "rg", "python", "node"];

/// Forbidden shell control sequences to block command chaining/injection attempts.
const FORBIDDEN_SHELL_SEQUENCES: &[&str] = &["&&", "||", ";", "`", "$(", "\n", "\r"];

/// Denied command patterns (case-insensitive substring match)
const DENYLIST: &[&str] = &[
    "rm ",
    "rm\t",
    "del ",
    "del\t",
    "rmdir",
    "rd ",
    "format",
    "fdisk",
    "dd if=",
    "mkfs",
    "mkswap",
    "swapon",
    "git reset --hard",
    "git clean -fd",
    "git push --force",
    "git push -f",
    "> /dev/null",
    ">/dev/null",
    "2>&1 |",
    "| sh",
    "| bash",
    "curl |",
    "wget |",
    "sudo ",
    "su -",
    "doas ",
    "chown ",
    "chmod 777",
    "chmod -R 777",
];

fn normalize_executable_token(token: &str) -> String {
    let file_name = token
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase();

    file_name
        .trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat")
        .to_string()
}

fn is_allowed_executable(token: &str) -> bool {
    let trimmed = token.trim_matches('"').trim_matches('\'');
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.starts_with('.') {
        return false;
    }

    let base_name = std::path::Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str());
    let Some(base_name) = base_name else {
        return false;
    };

    let normalized = normalize_executable_token(base_name);
    ALLOWLIST.iter().any(|&allowed| {
        normalized == allowed
            || (normalized.starts_with(allowed)
                && normalized.len() > allowed.len()
                && normalized
                    .chars()
                    .nth(allowed.len())
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false))
    })
}

fn parse_command_tokens(command: &str) -> Result<Vec<String>, ToolResult> {
    let parsed = shlex::split(command).unwrap_or_else(|| {
        command
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    });
    if parsed.is_empty() {
        return Err(ToolResult::error(
            "invalid_args",
            "Command is empty after parsing",
        ));
    }
    Ok(parsed)
}

fn contains_forbidden_shell_sequence(command: &str) -> Option<&'static str> {
    FORBIDDEN_SHELL_SEQUENCES
        .iter()
        .copied()
        .find(|pattern| command.contains(pattern))
}

/// Execute command tool implementation with security controls
async fn execute_command(args: Value, context: &ToolContext) -> ToolResult {
    let Some(command_str) = args.get("command").and_then(|c| c.as_str()) else {
        return ToolResult::error("invalid_args", "Missing required 'command' argument");
    };

    // Get working directory
    let cwd_str = args.get("cwd").and_then(|c| c.as_str()).unwrap_or(".");
    let cwd = context.resolve_path(cwd_str);

    // Check workspace boundary for CWD - deny if no workspace or path outside workspace
    if !context.is_within_workspace(&cwd) {
        let root_msg = context
            .workspace_root
            .as_ref()
            .map(|r| format!("'{}'", r.display()))
            .unwrap_or_else(|| "(no workspace set)".to_string());
        return ToolResult::error(
            "path_denied",
            format!(
                "Working directory '{}' is outside workspace root {}",
                cwd.display(),
                root_msg
            ),
        );
    }

    // Trim command
    let command_str = command_str.trim();
    if command_str.is_empty() {
        return ToolResult::error("invalid_args", "Command cannot be empty");
    }

    if let Some(pattern) = contains_forbidden_shell_sequence(command_str) {
        return ToolResult::error(
            "command_denied",
            format!("Command contains forbidden shell control sequence: '{pattern}'"),
        );
    }

    let tokens = match parse_command_tokens(command_str) {
        Ok(tokens) => tokens,
        Err(err) => return err,
    };
    let executable = &tokens[0];
    let command_args = &tokens[1..];

    if !is_allowed_executable(executable) {
        return ToolResult::error(
            "command_not_allowed",
            format!(
                "Command '{}' is not in allowlist. Allowed commands: {}",
                executable,
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
        run_command(executable, command_args, &cwd),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return ToolResult::error(
                "command_failed",
                format!("Failed to execute command: {}", e),
            );
        }
        Err(_) => {
            return ToolResult::error(
                "timeout",
                format!("Command timed out after {} seconds", COMMAND_TIMEOUT_SECS),
            );
        }
    };

    // Combine stdout and stderr
    let mut combined = String::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.is_empty() {
        combined.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str("[stderr]: ");
        combined.push_str(&stderr);
    }

    // Include exit code if non-zero (do this BEFORE truncation so exit code is always visible)
    let mut result = if output.status.success() {
        combined.clone()
    } else {
        format!(
            "[Exit code: {}]\n{}",
            output.status.code().unwrap_or(-1),
            combined
        )
    };

    // Apply size limit
    let was_truncated = result.len() > MAX_OUTPUT_SIZE;
    if was_truncated {
        utf8_truncate(&mut result, MAX_OUTPUT_SIZE);
        result.push_str("\n\n[Output truncated: exceeded size limit]");
    }

    let mut tool_result = ToolResult::success(result);
    if was_truncated {
        tool_result = tool_result.truncated();
    }
    tool_result
}

/// Run a shell command and capture output
async fn run_command(
    executable: &str,
    args: &[String],
    cwd: &Path,
) -> Result<std::process::Output, std::io::Error> {
    let mut cmd = tokio::process::Command::new(executable);
    cmd.args(args);
    cmd.current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let output = cmd.output().await?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_truncate_preserves_char_boundaries() {
        // ASCII only - truncation occurs
        let mut s = String::from("hello world");
        assert!(utf8_truncate(&mut s, 5));
        assert_eq!(s, "hello");

        // Multi-byte UTF-8: emoji (4 bytes each)
        let mut s = String::from("hello 👋🌍");
        assert!(utf8_truncate(&mut s, 10));
        // 10 bytes cuts in middle of 🌍 (which is 4 bytes starting at byte 8)
        // Should truncate to boundary before 🌍
        assert_eq!(s, "hello 👋");

        // CJK characters (3 bytes each)
        let mut s = String::from("你好世界"); // 12 bytes total, 4 chars
        assert!(!utf8_truncate(&mut s, 12)); // exact fit
        assert_eq!(s, "你好世界");
        assert!(utf8_truncate(&mut s, 10)); // truncate to nearest boundary
                                            // 10 is mid-char of 4th char (starts at 9), backs up to 9 => 3 chars
        assert_eq!(s, "你好世");

        // Mixed: accented + 4-byte emoji
        let mut s = String::from("hello 😀"); // 'hello' (5) + space (1) + 😀 (4) = 10 bytes
        assert!(!utf8_truncate(&mut s, 10)); // exact fit (10 bytes)
        assert_eq!(s, "hello 😀");
        assert!(utf8_truncate(&mut s, 9)); // truncate into middle of emoji
                                           // Should back up to boundary after space => "hello " (6 bytes)
        assert_eq!(s, "hello ");
        assert!(std::str::from_utf8(s.as_bytes()).is_ok());
    }

    #[test]
    fn utf8_truncate_returns_true_when_truncated() {
        let mut s = String::from("1234567890");
        assert!(utf8_truncate(&mut s, 5));
        assert_eq!(s, "12345");
    }

    #[test]
    fn utf8_truncate_returns_false_when_no_truncation() {
        let mut s = String::from("hello");
        assert!(!utf8_truncate(&mut s, 10));
        assert_eq!(s, "hello");
    }

    #[test]
    fn clean_path_preserves_excess_parent_dirs() {
        let ctx = ToolContext::new(None, PathBuf::from("/workspace"));
        let path = Path::new("../../../outside");
        let cleaned = ctx.clean_path(path);
        // Should preserve all three ".." components plus "outside"
        let components: Vec<_> = cleaned.components().collect();
        assert_eq!(components.len(), 4);
        let parent_count = components
            .iter()
            .filter(|c| matches!(c, std::path::Component::ParentDir))
            .count();
        assert_eq!(parent_count, 3);
    }

    #[test]
    fn clean_path_does_not_drop_unresolved_dots() {
        let ctx = ToolContext::new(None, PathBuf::from("/workspace"));
        let path = Path::new("../../etc/passwd");
        let cleaned = ctx.clean_path(path);
        // Should preserve both ".." and not drop to "etc/passwd"
        let has_parent = cleaned
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
        assert!(has_parent);
    }

    #[test]
    fn clean_path_handles_root_separator_on_windows_style() {
        // On Windows, a path like "C:\\workspace" should keep the backslash after drive letter
        // We'll test with a Path that has a root directory component
        let ctx = ToolContext::new(None, PathBuf::from("."));
        // Create a path that results in RootDir component after Prefix
        // On Unix, "/home" has RootDir then Normal(home)
        // On Windows, "C:\\home" has Prefix(C:), RootDir, Normal(home)
        let path = Path::new("/home/user");
        let cleaned = ctx.clean_path(path);
        // The result should start with a root separator (i.e., be an absolute path)
        assert!(cleaned.has_root());
        // On Unix, it should be "/home/user"
        #[cfg(unix)]
        assert_eq!(cleaned, PathBuf::from("/home/user"));
        // The test passes on Windows when run there; we can't easily simulate,
        // but the fix adds RootDir to result when encountered.
    }

    #[cfg(windows)]
    #[test]
    fn clean_path_preserves_windows_drive_root_separator() {
        let ctx = ToolContext::new(None, PathBuf::from("."));
        let path = Path::new("C:\\workspace\\file.txt");
        let cleaned = ctx.clean_path(path);
        // On Windows, should retain backslash after drive letter: "C:\workspace\file.txt"
        let s = cleaned.to_string_lossy();
        // Should contain ":\" (colon followed by backslash)
        assert!(s.contains(":\\"));
        // Should start with drive letter, colon, backslash
        assert!(s.starts_with("C:\\"));
    }
}
