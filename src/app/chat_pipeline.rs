use crate::{
    app::{
        format_tool_calls, refresh_model_context_window, tool_calls_preview, App, ToastLevel,
        AUTO_SCROLL_TARGET,
    },
    history,
    ollama::chat::{ChatMessage, ChatToolCall, ParsedStreamChunk},
    workspace::{manager::WorkspaceManager, WorkspaceSession},
};

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
    if !images_base64.is_empty() {
        if let Some(last_user) = request_messages.iter_mut().rev().find(|m| m.role == "user") {
            last_user.images = Some(images_base64.clone());
        }
    }
    app.pending_images.clear();

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

    let stream_result = crate::ollama::chat::chat_stream(
        client,
        &stream_url,
        &stream_model,
        &request_messages,
        |chunk: &ParsedStreamChunk| {
            if !chunk.content.is_empty() {
                streamed_any = true;
                let message = &mut app.messages[assistant_index];
                message.content.push_str(&chunk.content);
            }
            if !chunk.tool_calls.is_empty() {
                captured_tool_calls.extend(chunk.tool_calls.clone());
            }
            if let Some(value) = chunk.prompt_eval_count {
                prompt_eval_count = Some(value);
            }
            if let Some(value) = chunk.eval_count {
                eval_count = Some(value);
            }
            app.chat_scroll = AUTO_SCROLL_TARGET;
            let _ = terminal.draw(|f| crate::app::tui::draw(f, app));
        },
    )
    .await;

    app.is_loading = false;
    match stream_result {
        Ok(()) => {
            app.connection_status = crate::app::ConnectionStatus::Online;
            if !streamed_any {
                app.messages[assistant_index].content = String::from("(no response content)");
            }
            let tool_calls_json = if captured_tool_calls.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&captured_tool_calls)?)
            };
            app.messages[assistant_index].tool_calls_preview =
                tool_calls_preview(&captured_tool_calls);

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

            if !captured_tool_calls.is_empty() {
                let tool_summary = format_tool_calls(&captured_tool_calls);
                app.push_tool(
                    tool_summary.clone(),
                    tool_calls_preview(&captured_tool_calls),
                );
                let tool_calls_json = serde_json::to_string(&captured_tool_calls)?;
                let _ = history::append_message_with_meta(
                    manager,
                    session.conversation_id,
                    "tool",
                    &tool_summary,
                    None,
                    Some(&tool_calls_json),
                );
            }

            app.status = String::from("Ready");
        }
        Err(err) => {
            app.connection_status = crate::app::ConnectionStatus::Offline;
            app.messages[assistant_index].content = String::from("(request failed)");
            app.push_system(format!("Request failed: {err}"));
            app.set_toast(ToastLevel::Error, "Request failed");
            app.status = String::from("Error");
        }
    }

    Ok(())
}
