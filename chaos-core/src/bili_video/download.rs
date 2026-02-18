use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::header::HeaderMap;
use reqwest::Client;
use tokio::io::AsyncWriteExt;

use super::BiliError;

pub type ProgressCb = Arc<dyn Fn(u64, Option<u64>) + Send + Sync + 'static>;

async fn fetch_with_range(
    http: &Client,
    url: &str,
    headers: &HeaderMap,
    range: Option<(u64, u64)>,
) -> Result<reqwest::Response, BiliError> {
    let mut req = http.get(url).headers(headers.clone());
    if let Some((start, end)) = range {
        req = req.header(reqwest::header::RANGE, format!("bytes={start}-{end}"));
    }
    Ok(req.send().await?.error_for_status()?)
}

fn parse_total_from_content_range(v: &str) -> Option<u64> {
    // bytes 0-0/1234
    let s = v.trim();
    let (_, rest) = s.split_once(' ')?;
    let (_, total) = rest.split_once('/')?;
    total.trim().parse::<u64>().ok()
}

pub async fn probe_size(http: &Client, url: &str, headers: &HeaderMap) -> Result<(Option<u64>, bool), BiliError> {
    let resp = http
        .get(url)
        .headers(headers.clone())
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await?;
    let status = resp.status().as_u16();
    if status == 206 {
        let total = resp
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_total_from_content_range);
        return Ok((total, true));
    }
    if status == 200 {
        let total = resp.content_length();
        return Ok((total, false));
    }
    Ok((None, false))
}

pub async fn download_to_file_single(
    http: &Client,
    url: &str,
    headers: &HeaderMap,
    out_path: &Path,
    retries: u32,
    overwrite: bool,
    cancel: Option<&Arc<AtomicBool>>,
    progress: Option<ProgressCb>,
) -> Result<u64, BiliError> {
    if out_path.exists() && !overwrite {
        return Err(BiliError::Io("target exists".to_string()));
    }
    if let Some(parent) = out_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let tmp = out_path.with_extension("part");
    let _ = tokio::fs::remove_file(&tmp).await;

    let mut last_err: Option<String> = None;
    for attempt in 0..=retries {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(BiliError::Io("canceled".to_string()));
            }
        }
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
        }
        let _ = tokio::fs::remove_file(&tmp).await;

        let resp = match fetch_with_range(http, url, headers, None).await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(e.to_string());
                continue;
            }
        };
        let total = resp.content_length();
        let mut stream = resp.bytes_stream();
        let mut f = tokio::fs::File::create(&tmp).await?;
        let mut downloaded: u64 = 0;
        while let Some(chunk) = stream.next().await {
            if let Some(c) = cancel {
                if c.load(Ordering::Relaxed) {
                    return Err(BiliError::Io("canceled".to_string()));
                }
            }
            let chunk: Bytes = chunk.map_err(|e| BiliError::Http(e.to_string()))?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            f.write_all(&chunk).await?;
            if let Some(cb) = progress.as_ref() {
                cb(downloaded, total);
            }
        }
        f.flush().await?;
        drop(f);

        if out_path.exists() {
            let _ = tokio::fs::remove_file(out_path).await;
        }
        tokio::fs::rename(&tmp, out_path).await?;
        return Ok(downloaded);
    }

    Err(BiliError::Io(
        last_err.unwrap_or_else(|| "download failed".to_string()),
    ))
}

async fn download_range_part(
    http: &Client,
    url: &str,
    headers: HeaderMap,
    start: u64,
    end: u64,
    part_path: PathBuf,
    retries: u32,
    cancel: Option<Arc<AtomicBool>>,
    progress_total: Arc<AtomicU64>,
    total_opt: Option<u64>,
    progress: Option<ProgressCb>,
) -> Result<(), BiliError> {
    let _ = tokio::fs::remove_file(&part_path).await;
    let mut last_err: Option<String> = None;
    for attempt in 0..=retries {
        if let Some(c) = cancel.as_ref() {
            if c.load(Ordering::Relaxed) {
                return Err(BiliError::Io("canceled".to_string()));
            }
        }
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
        }
        let _ = tokio::fs::remove_file(&part_path).await;

        let resp = match fetch_with_range(http, url, &headers, Some((start, end))).await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(e.to_string());
                continue;
            }
        };
        let mut stream = resp.bytes_stream();
        let mut f = tokio::fs::File::create(&part_path).await?;
        while let Some(chunk) = stream.next().await {
            if let Some(c) = cancel.as_ref() {
                if c.load(Ordering::Relaxed) {
                    return Err(BiliError::Io("canceled".to_string()));
                }
            }
            let chunk: Bytes = chunk.map_err(|e| BiliError::Http(e.to_string()))?;
            f.write_all(&chunk).await?;
            let total_now = progress_total.fetch_add(chunk.len() as u64, Ordering::Relaxed)
                .saturating_add(chunk.len() as u64);
            if let Some(cb) = progress.as_ref() {
                cb(total_now, total_opt);
            }
        }
        f.flush().await?;
        drop(f);
        return Ok(());
    }
    Err(BiliError::Io(
        last_err.unwrap_or_else(|| "range download failed".to_string()),
    ))
}

pub async fn download_to_file_ranged(
    http: &Client,
    url: &str,
    headers: &HeaderMap,
    out_path: &Path,
    concurrency: u32,
    retries: u32,
    overwrite: bool,
    cancel: Option<&Arc<AtomicBool>>,
    progress: Option<ProgressCb>,
) -> Result<u64, BiliError> {
    if out_path.exists() && !overwrite {
        return Err(BiliError::Io("target exists".to_string()));
    }
    if let Some(parent) = out_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let (total_opt, can_range) = probe_size(http, url, headers).await?;
    let conc = concurrency.clamp(1, 16);
    if !can_range || total_opt.unwrap_or(0) < 2 * 1024 * 1024 || conc <= 1 {
        return download_to_file_single(http, url, headers, out_path, retries, overwrite, cancel, progress).await;
    }
    let total = total_opt.unwrap();

    let tmp = out_path.with_extension("part");
    let _ = tokio::fs::remove_file(&tmp).await;
    let _ = tokio::fs::remove_file(out_path).await;

    let base = out_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("part");
    let part_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    let mut part_paths: Vec<PathBuf> = Vec::new();

    let chunk = (total / conc as u64).max(1024 * 512);
    let downloaded = Arc::new(AtomicU64::new(0));
    let progress_cb = progress.clone();

    let mut tasks = Vec::new();
    for i in 0..conc {
        let start = i as u64 * chunk;
        if start >= total {
            break;
        }
        let end = if i == conc - 1 {
            total - 1
        } else {
            ((i as u64 + 1) * chunk).saturating_sub(1).min(total - 1)
        };
        let part_path = part_dir.join(format!("{base}.part{i:02}"));
        part_paths.push(part_path.clone());

        let http = http.clone();
        let url = url.to_string();
        let headers = headers.clone();
        let cancel = cancel.cloned();
        let downloaded = downloaded.clone();
        let progress_cb = progress_cb.clone();

        tasks.push(tokio::spawn(async move {
            download_range_part(
                &http,
                &url,
                headers,
                start,
                end,
                part_path,
                retries,
                cancel,
                downloaded,
                Some(total),
                progress_cb,
            )
            .await
        }));
    }

    for t in tasks {
        t.await.map_err(|e| BiliError::Io(e.to_string()))??;
    }

    // Merge parts.
    {
        let mut out = tokio::fs::File::create(&tmp).await?;
        for p in &part_paths {
            let mut f = tokio::fs::File::open(p).await?;
            tokio::io::copy(&mut f, &mut out).await?;
            let _ = tokio::fs::remove_file(p).await;
        }
        out.flush().await?;
    }

    tokio::fs::rename(&tmp, out_path).await?;
    Ok(downloaded.load(Ordering::Relaxed))
}
