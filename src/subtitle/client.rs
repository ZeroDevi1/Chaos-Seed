use std::time::Duration;

use thiserror::Error;

use super::models::{ThunderSubtitleItem, ThunderSubtitleResponse};

#[derive(Debug, Error)]
pub enum ThunderError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("download failed after {retries} retries: {message}")]
    DownloadFailed { retries: u32, message: String },
}

pub struct ThunderClient {
    base_url: String,
    http: reqwest::Client,
}

impl ThunderClient {
    pub fn new() -> Result<Self, ThunderError> {
        Self::with_base_url("https://api-shoulei-ssl.xunlei.com")
    }

    pub fn with_base_url(base_url: &str) -> Result<Self, ThunderError> {
        let http = reqwest::Client::builder()
            .user_agent("chaos-seed/0.1")
            .build()?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        })
    }

    pub async fn search(
        &self,
        query: &str,
        timeout: Duration,
    ) -> Result<Vec<ThunderSubtitleItem>, ThunderError> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/oracle/subtitle", self.base_url);
        let resp = self
            .http
            .get(url)
            .query(&[("name", q)])
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;

        let data: ThunderSubtitleResponse = resp.json().await?;
        if data.code != 0 || data.result != "ok" {
            return Ok(vec![]);
        }
        Ok(data.data)
    }

    pub async fn download_bytes(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<Vec<u8>, ThunderError> {
        let resp = self.http.get(url).timeout(timeout).send().await?;
        let resp = resp.error_for_status()?;
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }
}

pub async fn download_with_retries(
    client: &ThunderClient,
    url: &str,
    timeout: Duration,
    retries: u32,
    retry_sleep: Duration,
) -> Result<Vec<u8>, ThunderError> {
    let mut last_msg: Option<String> = None;
    for attempt in 0..=retries {
        match client.download_bytes(url, timeout).await {
            Ok(b) => return Ok(b),
            Err(e) => {
                last_msg = Some(e.to_string());
                if attempt >= retries {
                    break;
                }
                let sleep_dur = retry_sleep
                    .checked_mul(attempt + 1)
                    .unwrap_or_else(|| retry_sleep);
                tokio::time::sleep(sleep_dur).await;
            }
        }
    }

    Err(ThunderError::DownloadFailed {
        retries,
        message: last_msg.unwrap_or_else(|| "unknown error".to_string()),
    })
}
