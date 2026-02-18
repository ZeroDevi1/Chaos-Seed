use regex::Regex;
use serde_json::Value;

use super::{BiliClient, BiliError, bili_check_code, header_map_with_cookie};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoId {
    pub aid: Option<String>,
    pub bvid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewPage {
    pub page_number: u32,
    pub cid: String,
    pub page_title: String,
    pub duration_s: Option<u32>,
    pub dimension: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewInfo {
    pub aid: String,
    pub bvid: String,
    pub title: String,
    pub desc: Option<String>,
    pub pic: Option<String>,
    pub owner_name: Option<String>,
    pub owner_mid: Option<String>,
    pub pub_time_unix_s: Option<i64>,
    pub pages: Vec<ViewPage>,
}

fn re_bvid() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(BV[0-9A-Za-z]{10})").unwrap())
}

fn re_aid() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bav(\d{1,20})\b").unwrap())
}

pub fn parse_video_id(input: &str) -> Result<VideoId, BiliError> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(BiliError::InvalidInput("empty input".to_string()));
    }

    if let Some(m) = re_bvid().captures(raw).and_then(|c| c.get(1)) {
        return Ok(VideoId {
            aid: None,
            bvid: Some(m.as_str().to_string()),
        });
    }
    if let Some(m) = re_aid().captures(raw).and_then(|c| c.get(1)) {
        return Ok(VideoId {
            aid: Some(m.as_str().to_string()),
            bvid: None,
        });
    }

    Err(BiliError::InvalidInput("unsupported input (expect BV/av/url)".to_string()))
}

pub async fn fetch_view_info(
    client: &BiliClient,
    id: &VideoId,
    cookie: Option<&str>,
) -> Result<ViewInfo, BiliError> {
    let base = client.endpoints.api_base.trim_end_matches('/');
    let url = if let Some(bv) = id.bvid.as_deref().filter(|s| !s.trim().is_empty()) {
        format!("{base}/x/web-interface/view?bvid={}", urlencoding::encode(bv))
    } else if let Some(aid) = id.aid.as_deref().filter(|s| !s.trim().is_empty()) {
        format!("{base}/x/web-interface/view?aid={}", urlencoding::encode(aid))
    } else {
        return Err(BiliError::InvalidInput("missing aid/bvid".to_string()));
    };

    let headers = header_map_with_cookie(cookie);
    let json: Value = client.http.get(url).headers(headers).send().await?.json().await?;
    bili_check_code(&json)?;

    let data = json
        .get("data")
        .ok_or_else(|| BiliError::Parse("missing data".to_string()))?;

    let aid = data.get("aid").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_default();
    let bvid = data.get("bvid").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if aid.is_empty() || bvid.is_empty() || title.is_empty() {
        return Err(BiliError::Parse("missing aid/bvid/title".to_string()));
    }
    let desc = data.get("desc").and_then(|v| v.as_str()).map(|s| s.to_string());
    let pic = data.get("pic").and_then(|v| v.as_str()).map(|s| s.to_string());
    let pub_time = data.get("pubdate").and_then(|v| v.as_i64());

    let (owner_name, owner_mid) = data
        .get("owner")
        .and_then(|v| v.as_object())
        .map(|o| {
            (
                o.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                o.get("mid").and_then(|v| v.as_i64()).map(|v| v.to_string()),
            )
        })
        .unwrap_or((None, None));

    let mut pages: Vec<ViewPage> = Vec::new();
    if let Some(arr) = data.get("pages").and_then(|v| v.as_array()) {
        for p in arr {
            let page_number = p.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let cid = p.get("cid").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default();
            let part = p.get("part").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if page_number == 0 || cid.is_empty() || part.is_empty() {
                continue;
            }
            let duration_s = p.get("duration").and_then(|v| v.as_u64()).map(|v| v as u32);
            let dimension = p.get("dimension").and_then(|v| v.as_object()).and_then(|d| {
                let w = d.get("width").and_then(|v| v.as_u64()).unwrap_or(0);
                let h = d.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
                if w > 0 && h > 0 {
                    Some(format!("{w}x{h}"))
                } else {
                    None
                }
            });
            pages.push(ViewPage {
                page_number,
                cid,
                page_title: part,
                duration_s,
                dimension,
            });
        }
    }

    if pages.is_empty() {
        return Err(BiliError::Parse("missing pages".to_string()));
    }

    Ok(ViewInfo {
        aid,
        bvid,
        title,
        desc,
        pic,
        owner_name,
        owner_mid,
        pub_time_unix_s: pub_time,
        pages,
    })
}

