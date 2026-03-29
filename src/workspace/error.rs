use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("db error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid context path: {0}")]
    InvalidContextPath(String),
    #[error("not found: {0}")]
    NotFound(String),
}
