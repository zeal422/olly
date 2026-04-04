use std::time::{Duration, Instant};

use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::{
    app::{refresh_model_context_window, state::ToastLevel, tool_calls_preview, App},
    history,
    ollama::chat::{chat_stream_with_tools, ChatMessage, ChatToolCall, ParsedStreamChunk},
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

use crate::app::tools::{
    executor::{execute_tool, ToolContext},
    registry::{all_tools, get_tool},
    ToolPolicy, ToolResult, ToolResultStatus, ToolRiskLevel, ToolsState,
};

/// Maximum number of tool execution steps per user message
const MAX_TOOL_STEPS: usize = 4;

/// Maximum cumulative tool output size (100KB).
/// This is intentionally 10x the per-tool MAX_OUTPUT_SIZE (10KB) to allow
/// roughly 10 full-sized tool outputs within a single conversation turn.
/// This balances preventing runaway output with allowing meaningful multi-tool workflows.
const MAX_CUMULATIVE_TOOL_OUTPUT: usize = 100 * 1024;

/// Timeout for interactive tool confirmation when policy is `confirm`.
const TOOL_CONFIRM_TIMEOUT_SECS: u64 = 60;

fn build_advertised_tools(
    tools_state: &ToolsState,
    workspace_root: &Option<std::path::PathBuf>,
) -> Vec<crate::ollama::chat::ToolDefinition> {
    if !tools_state.enabled || workspace_root.is_none() {
        return Vec::new();
    }

    all_tools()
        .into_iter()
        .filter(|tool| {
            if !tools_state.is_tool_allowed(&tool.name) {
                return false;
            }
            true
        })
        .map(|tool| tool.to_tool_definition())
        .collect()
}

fn is_shell_tool(tool_name: &str) -> bool {
    get_tool(tool_name)
        .map(|tool| matches!(tool.risk_level, ToolRiskLevel::Shell))
        .unwrap_or(false)
}

fn model_tool_message_content(result: &ToolResult) -> String {
    match result.status {
        ToolResultStatus::Success => result.output.clone(),
        ToolResultStatus::Error => result.to_json_string(),
    }
}

fn requires_confirmation(policy: ToolPolicy, tool_name: &str) -> bool {
    matches!(policy, ToolPolicy::Confirm) && is_shell_tool(tool_name)
}

fn should_retry_without_tools(tools_used: bool, stream_failed: bool) -> bool {
    tools_used && stream_failed
}

fn emit_tool_status_notice(app: &mut App, tool_name: &str, result: &ToolResult) {
    let summary = if matches!(result.status, ToolResultStatus::Error) {
        let error_type = result.error_type.as_deref().unwrap_or("tool_error");
        let error_message = result.error.as_deref().unwrap_or("Tool execution failed");
        format!("{tool_name}: [{error_type}] {error_message}")
    } else {
        format!("{tool_name}: success")
    };
    app.set_toast(ToastLevel::Info, summary);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolConfirmationDecision {
    Approve,
    Cancel,
}

async fn wait_for_tool_confirmation(
    app: &mut App,
    terminal: &mut crate::app::runtime::AppTerminal,
    tool_name: &str,
    tool_args: &serde_json::Value,
    step: usize,
    total_steps: usize,
) -> Result<ToolConfirmationDecision, Box<dyn std::error::Error>> {
    app.pending_tool_confirmation = Some(crate::app::state::PendingToolConfirmation {
        tool_call_id: format!("tool-confirm-{step}-{tool_name}"),
        tool_name: tool_name.to_string(),
        tool_args: tool_args.to_string(),
        expires_at: Instant::now() + Duration::from_secs(TOOL_CONFIRM_TIMEOUT_SECS),
        step,
        total_steps,
    });

    loop {
        if app.should_quit() {
            app.pending_tool_confirmation = None;
            return Ok(ToolConfirmationDecision::Cancel);
        }

        app.tick();
        terminal.draw(|f| crate::app::tui::draw(f, app))?;

        if app.pending_tool_confirmation.is_none() {
            app.set_toast(ToastLevel::Info, "Tool confirmation timed out");
            return Ok(ToolConfirmationDecision::Cancel);
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    terminal.clear()?;
                    crate::app::platform::hide_console_scrollbars();
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Enter => {
                        app.pending_tool_confirmation = None;
                        return Ok(ToolConfirmationDecision::Approve);
                    }
                    KeyCode::Esc => {
                        app.pending_tool_confirmation = None;
                        return Ok(ToolConfirmationDecision::Cancel);
                    }
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.request_quit();
                        app.pending_tool_confirmation = None;
                        return Ok(ToolConfirmationDecision::Cancel);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

pub(crate) async fn send_prompt(
    app: &mut App,
    manager: &WorkspaceManager,
    session: &mut WorkspaceSession,
    client: &reqwest::Client,
    terminal: &mut crate::app::runtime::AppTerminal,
    prompt: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if !app.has_active_session() || session.conversation_id <= 0 {
        app.push_system(
            "No active workspace. Create one from the sidebar (+ New Workspace) or run `/workspace create ...`."
                .to_string(),
        );
        app.set_toast(ToastLevel::Info, "Create or switch to a workspace first");
        return Ok(());
    }

    if app.model_context_window.is_none() {
        refresh_model_context_window(app, client).await;
    }

    let image_count = app.pending_images.len();
    let images_base64 = app
        .pending_images
        .iter()
        .map(|i| i.base64.clone())
        .collect::<Vec<_>>();
    let images_json = if images_base64.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&images_base64)?)
    };

    app.push_user(prompt.clone(), image_count);
    if let Err(err) = history::append_message_with_meta(
        manager,
        session.conversation_id,
        "user",
        &prompt,
        images_json.as_deref(),
        None,
    ) {
        app.push_system(format!("Persist failed: {err}"));
        app.set_toast(ToastLevel::Error, "Failed to save user message");
    }
    let _ = terminal.draw(|f| crate::app::tui::draw(f, app));

    let mut request_messages: Vec<ChatMessage> = app.to_ollama_messages();

    // Prepend system context with workspace information
    if let Some(ref workspace_path) = app.workspace_root_path {
        let system_context = format!(
            "You are working in the workspace: {}\nPlatform: {}\nDate: {}",
            workspace_path,
            std::env::consts::OS,
            Local::now().format("%Y-%m-%d")
        );
        request_messages.insert(0, ChatMessage::new("system", system_context));
    }

    if !images_base64.is_empty() {
        if let Some(last_user) = request_messages.iter_mut().rev().find(|m| m.role == "user") {
            last_user.images = Some(images_base64.clone());
        }
    }
    app.pending_images.clear();

    // Get workspace root path for tool context
    let workspace_root = app
        .workspace_root_path
        .as_ref()
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists());

    // Create tool execution context
    let tool_context = ToolContext::new(
        workspace_root.clone(),
        workspace_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
    );

    let tools = build_advertised_tools(&app.tools_state, &workspace_root);
    let tools_enabled = !tools.is_empty();

    app.push_assistant(String::new(), None);
    let assistant_index = app.messages.len() - 1;
    app.status = String::from("Receiving...");
    app.is_loading = true;
    let _ = terminal.draw(|f| crate::app::tui::draw(f, app));
    let stream_url = app.url.clone();
    let stream_model = app.model.clone();
    let mut streamed_any = false;
    let mut captured_tool_calls: Vec<ChatToolCall> = Vec::new();
    let mut prompt_eval_count: Option<u32> = None;
    let mut eval_count: Option<u32> = None;

    // Tool execution loop with max steps guard
    let mut tool_step = 0;
    let mut cumulative_output_size = 0;
    let mut tool_loop_messages = request_messages.clone();

    loop {
        // Check for cancellation
        if app.should_quit() {
            app.push_system("Tool execution cancelled by user".to_string());
            break;
        }

        // Check max steps guard
        if tool_step >= MAX_TOOL_STEPS {
            app.push_system(format!(
                "Tool execution stopped: max steps ({}) reached",
                MAX_TOOL_STEPS
            ));
            break;
        }

        // Check cumulative output budget
        if cumulative_output_size >= MAX_CUMULATIVE_TOOL_OUTPUT {
            app.push_system("Tool execution stopped: output budget exceeded".to_string());
            break;
        }

        let mut current_tool_calls: Vec<ChatToolCall> = Vec::new();
        let step_content_start = app.messages[assistant_index].content.len();
        let step_streamed_before = streamed_any;
        let step_prompt_eval_before = prompt_eval_count;
        let step_eval_before = eval_count;

        app.status = format!("Receiving (step {}/{})...", tool_step + 1, MAX_TOOL_STEPS);
        let _ = terminal.draw(|f| crate::app::tui::draw(f, app));

        // Use tool-enabled streaming if tools are enabled
        let mut stream_result = if tools_enabled && !tools.is_empty() {
            chat_stream_with_tools(
                client,
                &stream_url,
                &stream_model,
                &tool_loop_messages,
                Some(tools.as_slice()),
                Some(false),
                |chunk: &ParsedStreamChunk| {
                    if !chunk.content.is_empty() {
                        streamed_any = true;
                        let message = &mut app.messages[assistant_index];
                        message.content.push_str(&chunk.content);
                    }
                    if !chunk.tool_calls.is_empty() {
                        current_tool_calls.extend(chunk.tool_calls.clone());
                    }
                    if let Some(value) = chunk.prompt_eval_count {
                        prompt_eval_count = Some(value);
                    }
                    if let Some(value) = chunk.eval_count {
                        eval_count = Some(value);
                    }
                    app.chat_scroll = 0;
                    let _ = terminal.draw(|f| crate::app::tui::draw(f, app));
                },
            )
            .await
        } else {
            crate::ollama::chat::chat_stream(
                client,
                &stream_url,
                &stream_model,
                &tool_loop_messages,
                |chunk: &ParsedStreamChunk| {
                    if !chunk.content.is_empty() {
                        streamed_any = true;
                        let message = &mut app.messages[assistant_index];
                        message.content.push_str(&chunk.content);
                    }
                    if let Some(value) = chunk.prompt_eval_count {
                        prompt_eval_count = Some(value);
                    }
                    if let Some(value) = chunk.eval_count {
                        eval_count = Some(value);
                    }
                    app.chat_scroll = 0;
                    let _ = terminal.draw(|f| crate::app::tui::draw(f, app));
                },
            )
            .await
        };

        if should_retry_without_tools(tools_enabled, stream_result.is_err()) {
            app.messages[assistant_index]
                .content
                .truncate(step_content_start);
            streamed_any = step_streamed_before;
            prompt_eval_count = step_prompt_eval_before;
            eval_count = step_eval_before;
            current_tool_calls.clear();

            app.push_system(
                "Tool request failed; retrying once without tools for compatibility.".to_string(),
            );
            app.status = String::from("Retrying without tools...");
            let _ = terminal.draw(|f| crate::app::tui::draw(f, app));

            stream_result = crate::ollama::chat::chat_stream(
                client,
                &stream_url,
                &stream_model,
                &tool_loop_messages,
                |chunk: &ParsedStreamChunk| {
                    if !chunk.content.is_empty() {
                        streamed_any = true;
                        let message = &mut app.messages[assistant_index];
                        message.content.push_str(&chunk.content);
                    }
                    if let Some(value) = chunk.prompt_eval_count {
                        prompt_eval_count = Some(value);
                    }
                    if let Some(value) = chunk.eval_count {
                        eval_count = Some(value);
                    }
                    app.chat_scroll = 0;
                    let _ = terminal.draw(|f| crate::app::tui::draw(f, app));
                },
            )
            .await;
        }

        if let Err(err) = stream_result {
            app.connection_status = crate::app::state::ConnectionStatus::Offline;
            app.messages[assistant_index].content = String::from("(request failed)");
            app.push_system(format!("Request failed: {err}"));
            app.set_toast(ToastLevel::Error, "Request failed");
            app.status = String::from("Error");
            app.is_loading = false;
            return Ok(());
        }

        // If no tool calls, we're done
        if current_tool_calls.is_empty() {
            break;
        }

        // Execute tools and prepare for next iteration
        tool_step += 1;
        captured_tool_calls.extend(current_tool_calls.clone());

        // Add assistant message with tool calls BEFORE tool results for proper LLM context ordering
        let assistant_msg = ChatMessage {
            role: "assistant".to_string(),
            content: app.messages[assistant_index].content.clone(),
            images: None,
            tool_calls: Some(current_tool_calls.clone()),
            tool_name: None,
            thinking: None,
        };
        tool_loop_messages.push(assistant_msg.clone());

        // Persist intermediate assistant tool-call message to history (BUG #3)
        let tool_calls_json = serde_json::to_string(&current_tool_calls)
            .map_err(|err| app.push_system(format!("Failed to serialize tool_calls: {err}")))
            .ok();
        if let Some(json) = &tool_calls_json {
            if let Err(err) = history::append_message_with_meta(
                manager,
                session.conversation_id,
                "assistant",
                &assistant_msg.content,
                None,
                Some(json),
            ) {
                app.push_system(format!("Failed to persist assistant tool-call step: {err}"));
                app.set_toast(
                    crate::app::state::ToastLevel::Error,
                    "Failed to save assistant tool-call step",
                );
            }
        }

        // Clear assistant content for next iteration
        app.messages[assistant_index].content.clear();

        // Execute each tool call
        for tool_call in &current_tool_calls {
            if app.should_quit() {
                break;
            }

            app.status = format!(
                "Executing: {}...",
                tool_call
                    .function
                    .as_ref()
                    .map(|f| &f.name)
                    .unwrap_or(&"unknown".to_string())
            );
            let _ = terminal.draw(|f| crate::app::tui::draw(f, app));

            let result = if let Some(ref func) = tool_call.function {
                // Enforce tool safety controls (F2 fix)
                if !app.tools_state.enabled {
                    ToolResult::error(
                        "tools_disabled",
                        "Tool execution is disabled. Enable with /tools enable",
                    )
                } else if !app.tools_state.is_tool_allowed(&func.name) {
                    ToolResult::error(
                        "tool_denied",
                        format!("Tool '{}' is not in the allowed list", func.name),
                    )
                } else if requires_confirmation(app.tools_state.policy, &func.name) {
                    app.status = format!("Awaiting confirmation: {}...", func.name);
                    let _ = terminal.draw(|f| crate::app::tui::draw(f, app));
                    match wait_for_tool_confirmation(
                        app,
                        terminal,
                        &func.name,
                        &func.arguments,
                        tool_step,
                        MAX_TOOL_STEPS,
                    )
                    .await?
                    {
                        ToolConfirmationDecision::Approve => {
                            execute_tool(&func.name, func.arguments.clone(), &tool_context).await
                        }
                        ToolConfirmationDecision::Cancel => ToolResult::error(
                            "confirmation_cancelled",
                            format!("User cancelled tool '{}'", func.name),
                        ),
                    }
                } else {
                    execute_tool(&func.name, func.arguments.clone(), &tool_context).await
                }
            } else {
                ToolResult::error("invalid_tool", "Tool call missing function")
            };

            let model_tool_content = model_tool_message_content(&result);
            cumulative_output_size += model_tool_content.len();

            // Add tool result as a message
            let tool_name = tool_call
                .function
                .as_ref()
                .map(|f| f.name.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let tool_result_msg = ChatMessage {
                role: "tool".to_string(),
                content: model_tool_content.clone(),
                images: None,
                tool_calls: None,
                tool_name: Some(tool_name.clone()),
                thinking: None,
            };
            tool_loop_messages.push(tool_result_msg);

            // Keep full tool content in history/replay context.
            app.push_tool(model_tool_content.clone(), Some(tool_name.clone()));

            // Display compact UX feedback without persisting synthetic system messages.
            emit_tool_status_notice(app, &tool_name, &result);

            // Persist tool result
            let tool_calls_json = serde_json::to_string(std::slice::from_ref(tool_call))?;
            let _ = history::append_message_with_meta(
                manager,
                session.conversation_id,
                "tool",
                &model_tool_content,
                None,
                Some(&tool_calls_json),
            );
        }
    }

    app.is_loading = false;
    app.connection_status = crate::app::state::ConnectionStatus::Online;

    if !streamed_any && captured_tool_calls.is_empty() {
        app.messages[assistant_index].content = String::from("(no response content)");
    }

    let tool_calls_json = if captured_tool_calls.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&captured_tool_calls)?)
    };
    app.messages[assistant_index].tool_calls_preview = tool_calls_preview(&captured_tool_calls);

    if let Err(err) = history::append_message_with_meta(
        manager,
        session.conversation_id,
        "assistant",
        &app.messages[assistant_index].content,
        None,
        tool_calls_json.as_deref(),
    ) {
        app.push_system(format!("Persist failed: {err}"));
        app.set_toast(ToastLevel::Error, "Failed to save assistant message");
    }

    if let Some(tokens) = prompt_eval_count {
        app.prompt_tokens_used = Some(tokens);
        app.peak_prompt_tokens = app.peak_prompt_tokens.max(tokens);
        app.persist_active_session_context_snapshot();
    }
    if let Some(tokens) = eval_count {
        app.last_completion_tokens = Some(tokens);
        app.persist_active_session_context_snapshot();
    }

    app.status = String::from("Ready");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_advertised_tools, emit_tool_status_notice, model_tool_message_content,
        requires_confirmation, should_retry_without_tools,
    };
    use crate::app::tools::{ToolPolicy, ToolResult, ToolRiskLevel, ToolsState};
    use crate::app::App;

    #[test]
    fn build_advertised_tools_disabled_returns_empty() {
        let state = ToolsState::default();
        let workspace_root = Some(std::path::PathBuf::from("."));
        let tools = build_advertised_tools(&state, &workspace_root);
        assert!(tools.is_empty());
    }

    #[test]
    fn build_advertised_tools_requires_workspace_root() {
        let state = ToolsState {
            enabled: true,
            ..ToolsState::default()
        };
        let tools = build_advertised_tools(&state, &None);
        assert!(tools.is_empty());
    }

    #[test]
    fn build_advertised_tools_confirm_policy_keeps_shell_tools() {
        let state = ToolsState {
            enabled: true,
            policy: ToolPolicy::Confirm,
            ..ToolsState::default()
        };
        let workspace_root = Some(std::path::PathBuf::from("."));
        let tools = build_advertised_tools(&state, &workspace_root);
        assert!(tools
            .iter()
            .any(|tool| tool.function.name == "execute_command"));
    }

    #[test]
    fn build_advertised_tools_auto_policy_includes_shell_tools() {
        let state = ToolsState {
            enabled: true,
            policy: ToolPolicy::Auto,
            ..ToolsState::default()
        };
        let workspace_root = Some(std::path::PathBuf::from("."));
        let tools = build_advertised_tools(&state, &workspace_root);
        assert!(tools
            .iter()
            .any(|tool| tool.function.name == "execute_command"));
    }

    #[test]
    fn model_tool_message_content_for_error_is_non_empty_json() {
        let result = ToolResult::error("tool_denied", "Tool is denied");
        let content = model_tool_message_content(&result);
        assert!(!content.is_empty());
        assert!(content.contains("tool_denied"));
        assert!(content.contains("Tool is denied"));
    }

    #[test]
    fn shell_tool_is_classified_as_risky_in_registry() {
        let execute_command = crate::app::tools::registry::get_tool("execute_command")
            .expect("execute_command tool should exist");
        assert!(matches!(execute_command.risk_level, ToolRiskLevel::Shell));
    }

    #[test]
    fn requires_confirmation_for_shell_tool_in_confirm_policy() {
        assert!(requires_confirmation(
            ToolPolicy::Confirm,
            "execute_command"
        ));
        assert!(!requires_confirmation(ToolPolicy::Confirm, "read_file"));
        assert!(!requires_confirmation(ToolPolicy::Auto, "execute_command"));
    }

    #[test]
    fn retry_without_tools_requires_tools_and_failure() {
        assert!(should_retry_without_tools(true, true));
        assert!(!should_retry_without_tools(true, false));
        assert!(!should_retry_without_tools(false, false));
    }

    #[test]
    fn tool_status_notice_uses_toast_not_system_message() {
        let mut app = App::new(
            String::from("http://localhost:11434"),
            String::from("llama3.2:latest"),
        );
        let before_messages = app.messages.len();
        let result = ToolResult::success("ok");

        emit_tool_status_notice(&mut app, "read_file", &result);

        assert_eq!(app.messages.len(), before_messages);
        assert!(app.toast.is_some());
    }
}
