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

/// Tool definition sent to Ollama API in the `tools` array
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            images: None,
            tool_calls: None,
            tool_name: None,
            thinking: None,
        }
    }

    pub fn new_tool(tool_name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: content.into(),
            images: None,
            tool_calls: None,
            tool_name: Some(tool_name.into()),
            thinking: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<bool>,
}

impl<'a> ChatRequest<'a> {
    fn new(model: &'a str, messages: &'a [ChatMessage], stream: bool) -> Self {
        Self {
            model,
            messages,
            stream,
            tools: None,
            think: None,
        }
    }

    fn with_tools(mut self, tools: &'a [ToolDefinition]) -> Self {
        self.tools = Some(tools);
        self
    }

    fn with_think(mut self, think: bool) -> Self {
        self.think = Some(think);
        self
    }
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
    pub thinking: Option<String>,
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
    let tool_calls = message
        .as_ref()
        .and_then(|m| m.tool_calls.clone())
        .unwrap_or_default();
    let thinking = message.as_ref().and_then(|m| m.thinking.clone());
    Ok(Some(ParsedStreamChunk {
        content,
        done: parsed.done.unwrap_or(false),
        tool_calls,
        prompt_eval_count: parsed.prompt_eval_count,
        eval_count: parsed.eval_count,
        thinking,
    }))
}

pub async fn chat_once(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, OllamaError> {
    let req = ChatRequest::new(model, messages, false);
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

/// Chat once with optional tools and thinking support
pub async fn chat_once_with_tools(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
    think: Option<bool>,
) -> Result<(String, Vec<ChatToolCall>, Option<String>), OllamaError> {
    let mut req = ChatRequest::new(model, messages, false);
    if let Some(tools) = tools {
        req = req.with_tools(tools);
    }
    if let Some(think) = think {
        req = req.with_think(think);
    }
    let url = format!("{base_url}/api/chat");
    let response = client.post(url).json(&req).send().await?;
    if !response.status().is_success() {
        return Err(OllamaError::Api(format!(
            "status {}",
            response.status().as_u16()
        )));
    }

    let payload: ChatResponse = response.json().await?;
    let message = payload.message.unwrap_or_default();
    let content = message.content;
    let tool_calls = message.tool_calls.unwrap_or_default();
    let thinking = message.thinking;
    Ok((content, tool_calls, thinking))
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
    let req = ChatRequest::new(model, messages, true);
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

/// Stream chat with optional tools and thinking support
pub async fn chat_stream_with_tools<F>(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
    think: Option<bool>,
    mut on_chunk: F,
) -> Result<(), OllamaError>
where
    F: FnMut(&ParsedStreamChunk),
{
    let mut req = ChatRequest::new(model, messages, true);
    if let Some(tools) = tools {
        req = req.with_tools(tools);
    }
    if let Some(think) = think {
        req = req.with_think(think);
    }
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
