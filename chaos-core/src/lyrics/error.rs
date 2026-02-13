use thiserror::Error;

#[derive(Debug, Error)]
pub enum LyricsError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unsupported search term for this provider")]
    UnsupportedTerm,
}
