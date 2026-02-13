use std::path::Path;
use std::time::Duration;

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use tokio::io::AsyncWriteExt;

use super::error::MusicError;

async fn fetch_bytes_stream(
    http: &Client,
    url: &str,
    timeout: Duration,
) -> Result<(u16, impl futures_util::Stream<Item = Result<Bytes, reqwest::Error>>), MusicError> {
    let resp = http.get(url).timeout(timeout).send().await?;
    let status = resp.status().as_u16();
    Ok((status, resp.bytes_stream()))
}

pub async fn download_url_to_file(
    http: &Client,
    url: &str,
    out_path: &Path,
    timeout: Duration,
    retries: u32,
    overwrite: bool,
) -> Result<u64, MusicError> {
    let p = out_path;
    if p.as_os_str().is_empty() {
        return Err(MusicError::InvalidInput("empty out_path".to_string()));
    }

    if p.exists() && !overwrite {
        return Err(MusicError::Other("target exists".to_string()));
    }

    if let Some(parent) = p.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let tmp = p.with_extension(format!(
        "{}.part",
        p.extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
    ));

    let mut last_err: Option<String> = None;
    for attempt in 0..=retries {
        if attempt > 0 {
            // small backoff
            let d = match attempt {
                1 => 200,
                2 => 500,
                _ => 1200,
            };
            tokio::time::sleep(Duration::from_millis(d)).await;
        }

        // Clean tmp before retry (best-effort).
        let _ = tokio::fs::remove_file(&tmp).await;

        let (status, mut stream) = match fetch_bytes_stream(http, url, timeout).await {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(e.to_string());
                continue;
            }
        };

        if !(200..300).contains(&status) {
            last_err = Some(format!("http status {status}"));
            continue;
        }

        let mut f = tokio::fs::File::create(&tmp).await?;
        let mut bytes: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(MusicError::Http)?;
            bytes += chunk.len() as u64;
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
        drop(f);

        // Atomic-ish replace.
        if p.exists() {
            let _ = tokio::fs::remove_file(p).await;
        }
        tokio::fs::rename(&tmp, p).await?;
        return Ok(bytes);
    }

    Err(MusicError::Other(
        last_err.unwrap_or_else(|| "download failed".to_string()),
    ))
}

