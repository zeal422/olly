use std::time::{Duration, SystemTime};

use serde::Deserialize;
use serde::Serialize;

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
