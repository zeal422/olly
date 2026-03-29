use thiserror::Error;

#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("api error: {0}")]
    Api(String),
}
