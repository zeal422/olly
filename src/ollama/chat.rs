use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::OllamaError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatToolCall {
    #[serde(default)]
    pub function: Option<ToolFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            images: None,
            tool_calls: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
    done: Option<bool>,
    prompt_eval_count: Option<u32>,
    eval_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedStreamChunk {
    pub content: String,
    pub done: bool,
    pub tool_calls: Vec<ChatToolCall>,
    pub prompt_eval_count: Option<u32>,
    pub eval_count: Option<u32>,
}

pub fn parse_stream_line(line: &str) -> Result<Option<ParsedStreamChunk>, OllamaError> {
    if line.trim().is_empty() {
        return Ok(None);
    }
    let parsed: ChatResponse = serde_json::from_str(line)?;
    let message = parsed.message;
    let content = message
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let tool_calls = message.and_then(|m| m.tool_calls).unwrap_or_default();
    Ok(Some(ParsedStreamChunk {
        content,
        done: parsed.done.unwrap_or(false),
        tool_calls,
        prompt_eval_count: parsed.prompt_eval_count,
        eval_count: parsed.eval_count,
    }))
}

pub async fn chat_once(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, OllamaError> {
    let req = ChatRequest {
        model,
        messages,
        stream: false,
    };
    let url = format!("{base_url}/api/chat");
    let response = client.post(url).json(&req).send().await?;
    if !response.status().is_success() {
        return Err(OllamaError::Api(format!(
            "status {}",
            response.status().as_u16()
        )));
    }

    let payload: ChatResponse = response.json().await?;
    Ok(payload.message.map(|m| m.content).unwrap_or_default())
}

pub async fn chat_stream<F>(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: &[ChatMessage],
    mut on_chunk: F,
) -> Result<(), OllamaError>
where
    F: FnMut(&ParsedStreamChunk),
{
    let req = ChatRequest {
        model,
        messages,
        stream: true,
    };
    let url = format!("{base_url}/api/chat");
    let response = client.post(url).json(&req).send().await?;
    if !response.status().is_success() {
        return Err(OllamaError::Api(format!(
            "status {}",
            response.status().as_u16()
        )));
    }

    let mut stream = response.bytes_stream();
    let mut pending = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        pending.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_index) = pending.find('\n') {
            let line = pending[..newline_index].to_string();
            pending.drain(..=newline_index);
            if let Some(parsed) = parse_stream_line(&line)? {
                on_chunk(&parsed);
                if parsed.done {
                    return Ok(());
                }
            }
        }
    }

    if !pending.trim().is_empty() {
        if let Some(parsed) = parse_stream_line(&pending)? {
            on_chunk(&parsed);
        }
    }

    Ok(())
}
