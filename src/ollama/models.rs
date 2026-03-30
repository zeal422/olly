use std::time::{Duration, SystemTime};

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use super::error::OllamaError;

#[derive(Debug, Deserialize)]
struct ModelTag {
    name: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelTag>,
}

pub async fn list_models(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Vec<String>, OllamaError> {
    let url = format!("{base_url}/api/tags");
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(OllamaError::Api(format!(
            "status {}",
            response.status().as_u16()
        )));
    }

    let payload: TagsResponse = response.json().await?;
    Ok(payload.models.into_iter().map(|m| m.name).collect())
}

#[derive(Debug, Serialize)]
struct ModelNameRequest<'a> {
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct PullModelRequest<'a> {
    name: &'a str,
    stream: bool,
}

pub async fn pull_model(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
) -> Result<(), OllamaError> {
    let url = format!("{base_url}/api/pull");
    let response = client
        .post(url)
        .json(&PullModelRequest {
            name: model,
            stream: false,
        })
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(OllamaError::Api(format!(
            "status {}",
            response.status().as_u16()
        )));
    }
    Ok(())
}

pub async fn delete_model(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
) -> Result<(), OllamaError> {
    let url = format!("{base_url}/api/delete");
    let response = client
        .delete(url)
        .json(&ModelNameRequest { name: model })
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(OllamaError::Api(format!(
            "status {}",
            response.status().as_u16()
        )));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ShowModelRequest<'a> {
    model: &'a str,
}

pub async fn model_context_window(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
) -> Result<Option<u32>, OllamaError> {
    let url = format!("{base_url}/api/show");
    let response = client
        .post(url)
        .json(&ShowModelRequest { model })
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(OllamaError::Api(format!(
            "status {}",
            response.status().as_u16()
        )));
    }
    let payload: Value = response.json().await?;
    Ok(parse_context_window_from_show_payload(&payload))
}

fn parse_context_window_from_show_payload(payload: &Value) -> Option<u32> {
    if let Some(model_info) = payload.get("model_info").and_then(Value::as_object) {
        let from_model_info = model_info
            .iter()
            .find_map(|(key, value)| {
                if key.to_ascii_lowercase().contains("context_length") {
                    value.as_u64()
                } else {
                    None
                }
            })
            .and_then(|v| u32::try_from(v).ok());
        if from_model_info.is_some() {
            return from_model_info;
        }
    }

    payload
        .get("parameters")
        .and_then(Value::as_str)
        .and_then(|parameters| {
            parameters.lines().find_map(|line| {
                let trimmed = line.trim();
                if !trimmed.to_ascii_lowercase().starts_with("num_ctx") {
                    return None;
                }
                trimmed
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<u32>().ok())
            })
        })
}

#[derive(Debug, Clone)]
pub struct ModelCache {
    ttl: Duration,
    models: Vec<String>,
    last_refresh: Option<SystemTime>,
}

impl ModelCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            models: Vec::new(),
            last_refresh: None,
        }
    }

    pub fn models(&self) -> &[String] {
        &self.models
    }

    pub fn set_models_at(&mut self, models: Vec<String>, refreshed_at: SystemTime) {
        self.models = models;
        self.last_refresh = Some(refreshed_at);
    }

    pub fn should_refresh(&self, now: SystemTime) -> bool {
        match self.last_refresh {
            Some(last_refresh) => match now.duration_since(last_refresh) {
                Ok(elapsed) => elapsed >= self.ttl,
                Err(_) => true,
            },
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_context_window_from_show_payload;
    use serde_json::json;

    #[test]
    fn parse_context_window_prefers_model_info_context_length() {
        let payload = json!({
            "model_info": {
                "llama.context_length": 131072,
                "llama.embedding_length": 4096
            },
            "parameters": "num_ctx 32768"
        });
        assert_eq!(
            parse_context_window_from_show_payload(&payload),
            Some(131_072)
        );
    }

    #[test]
    fn parse_context_window_falls_back_to_parameters_num_ctx() {
        let payload = json!({
            "parameters": "temperature 0.7\nnum_ctx 65536\nrepeat_penalty 1.1"
        });
        assert_eq!(
            parse_context_window_from_show_payload(&payload),
            Some(65_536)
        );
    }

    #[test]
    fn parse_context_window_returns_none_when_unavailable() {
        let payload = json!({
            "details": {
                "family": "qwen"
            }
        });
        assert_eq!(parse_context_window_from_show_payload(&payload), None);
    }
}
