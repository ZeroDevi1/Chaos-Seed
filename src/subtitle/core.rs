use std::path::{Path, PathBuf};
use std::time::Duration;

use super::client::{download_with_retries, ThunderClient, ThunderError};
use super::models::ThunderSubtitleItem;
use super::util::{ensure_unique_path, sanitize_component};

pub fn apply_filters(items: Vec<ThunderSubtitleItem>, min_score: Option<f64>, lang: Option<&str>) -> Vec<ThunderSubtitleItem> {
    let mut out = items;
    if let Some(ms) = min_score {
        out.retain(|i| i.score >= ms);
    }
    if let Some(lang) = lang {
        let lang = lang.trim();
        if !lang.is_empty() {
            out.retain(|i| i.languages.iter().any(|l| l == lang));
        }
    }
    out
}

pub async fn search_items(
    query: &str,
    limit: usize,
    min_score: Option<f64>,
    lang: Option<&str>,
    timeout: Duration,
) -> Result<Vec<ThunderSubtitleItem>, ThunderError> {
    let client = ThunderClient::new();
    let mut items = client.search(query, timeout).await?;
    items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let items = apply_filters(items, min_score, lang);
    Ok(items.into_iter().take(limit).collect())
}

pub async fn download_item(
    item: &ThunderSubtitleItem,
    out_dir: &Path,
    timeout: Duration,
    retries: u32,
    overwrite: bool,
) -> Result<PathBuf, ThunderError> {
    let client = ThunderClient::new();

    let safe_name = sanitize_component(&item.name, 120);
    let ext = sanitize_component(if item.ext.trim().is_empty() { "srt" } else { &item.ext }, 10);

    std::fs::create_dir_all(out_dir)?;
    let mut path = out_dir.join(format!("{}.{}", safe_name, ext));
    if !overwrite {
        path = ensure_unique_path(&path)?;
    }

    let data = download_with_retries(
        &client,
        item.url.as_str(),
        timeout,
        retries,
        Duration::from_millis(500),
    )
    .await?;

    std::fs::write(&path, data)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, score: f64, languages: &[&str]) -> ThunderSubtitleItem {
        ThunderSubtitleItem {
            gcid: "g".to_string(),
            cid: "c".to_string(),
            url: "u".to_string(),
            ext: "srt".to_string(),
            name: name.to_string(),
            duration: 0,
            languages: languages.iter().map(|s| s.to_string()).collect(),
            source: 0,
            score,
            fingerprintf_score: 0.0,
            extra_name: "".to_string(),
            mt: 0,
        }
    }

    #[test]
    fn filter_by_min_score_and_lang() {
        let items = vec![item("a", 1.0, &["zh"]), item("b", 9.0, &["en"]), item("c", 10.0, &["zh", "en"])];
        let out = apply_filters(items, Some(9.0), Some("zh"));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "c");
    }
}
