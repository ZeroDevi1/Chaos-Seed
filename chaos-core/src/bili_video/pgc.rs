use serde_json::Value;

use super::{BiliClient, BiliError, bili_check_code, header_map_with_cookie};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgcEpisode {
    pub ep_id: String,
    pub aid: String,
    pub cid: String,
    pub title: String,
    pub long_title: Option<String>,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgcSeason {
    pub season_id: String,
    pub title: String,
    pub cover: Option<String>,
    pub episodes: Vec<PgcEpisode>,
}

fn parse_season_json(json: &Value) -> Result<PgcSeason, BiliError> {
    bili_check_code(json)?;
    let root = json
        .get("result")
        .or_else(|| json.get("data"))
        .ok_or_else(|| BiliError::Parse("missing result/data".to_string()))?;

    let season_id = root
        .get("season_id")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .or_else(|| root.get("season_id").and_then(|v| v.as_str()).map(|s| s.trim().to_string()))
        .unwrap_or_default();

    let title = root
        .get("season_title")
        .and_then(|v| v.as_str())
        .or_else(|| root.get("title").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim()
        .to_string();

    let cover = root.get("cover").and_then(|v| v.as_str()).map(|s| s.to_string());

    let mut episodes: Vec<PgcEpisode> = Vec::new();
    if let Some(arr) = root.get("episodes").and_then(|v| v.as_array()) {
        for e in arr {
            let ep_id = e
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .or_else(|| e.get("id").and_then(|v| v.as_str()).map(|s| s.trim().to_string()))
                .unwrap_or_default();
            let aid = e
                .get("aid")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default();
            let cid = e
                .get("cid")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default();
            if ep_id.is_empty() || aid.is_empty() || cid.is_empty() {
                continue;
            }
            let t = e.get("share_copy").and_then(|v| v.as_str()).or_else(|| e.get("title").and_then(|v| v.as_str())).unwrap_or("").trim().to_string();
            let lt = e.get("long_title").and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            let cover = e.get("cover").and_then(|v| v.as_str()).map(|s| s.to_string());
            let title = if !t.is_empty() { t } else { lt.clone().unwrap_or_else(|| ep_id.clone()) };
            episodes.push(PgcEpisode {
                ep_id,
                aid,
                cid,
                title,
                long_title: lt,
                cover,
            });
        }
    }

    if episodes.is_empty() {
        return Err(BiliError::Parse("missing episodes".to_string()));
    }

    Ok(PgcSeason {
        season_id,
        title,
        cover,
        episodes,
    })
}

pub async fn fetch_pgc_season_by_ep_id(
    client: &BiliClient,
    ep_id: &str,
    cookie: Option<&str>,
) -> Result<PgcSeason, BiliError> {
    let id = ep_id.trim();
    if id.is_empty() {
        return Err(BiliError::InvalidInput("missing ep_id".to_string()));
    }
    let base = client.endpoints.api_base.trim_end_matches('/');
    let url = format!("{base}/pgc/view/web/season?ep_id={}", urlencoding::encode(id));
    let json: Value = client.http.get(url).headers(header_map_with_cookie(cookie)).send().await?.json().await?;
    parse_season_json(&json)
}

pub async fn fetch_pgc_season_by_season_id(
    client: &BiliClient,
    season_id: &str,
    cookie: Option<&str>,
) -> Result<PgcSeason, BiliError> {
    let id = season_id.trim();
    if id.is_empty() {
        return Err(BiliError::InvalidInput("missing season_id".to_string()));
    }
    let base = client.endpoints.api_base.trim_end_matches('/');
    let url = format!("{base}/pgc/view/web/season?season_id={}", urlencoding::encode(id));
    let json: Value = client.http.get(url).headers(header_map_with_cookie(cookie)).send().await?.json().await?;
    parse_season_json(&json)
}

