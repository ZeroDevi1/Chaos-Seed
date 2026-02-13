use thiserror::Error;

#[derive(Debug, Error)]
pub enum MusicError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("unsupported service: {0}")]
    UnsupportedService(String),
    #[error("not configured: {0}")]
    NotConfigured(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("other error: {0}")]
    Other(String),
}

