use serde_json::Value;

use super::{BiliClient, BiliError, bili_check_code, header_map_with_cookie};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleTrack {
    pub lang: String,
    pub lang_doc: String,
    pub url: String,
}

pub async fn fetch_subtitles(
    client: &BiliClient,
    bvid: &str,
    cid: &str,
    cookie: Option<&str>,
) -> Result<Vec<SubtitleTrack>, BiliError> {
    let bv = bvid.trim();
    let c = cid.trim();
    if bv.is_empty() || c.is_empty() {
        return Ok(vec![]);
    }

    let url = format!(
        "{}/x/player/v2?bvid={}&cid={}",
        client.endpoints.api_base.trim_end_matches('/'),
        urlencoding::encode(bv),
        urlencoding::encode(c)
    );
    let headers = header_map_with_cookie(cookie);
    let json: Value = client.http.get(url).headers(headers).send().await?.json().await?;
    bili_check_code(&json)?;

    let subs = json
        .pointer("/data/subtitle/subtitles")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out: Vec<SubtitleTrack> = Vec::new();
    for s in subs {
        let url = s.get("subtitle_url").and_then(|v| v.as_str()).unwrap_or("").trim();
        if url.is_empty() {
            continue;
        }
        let url = if url.starts_with("//") {
            format!("https:{url}")
        } else {
            url.to_string()
        };
        let lang = s.get("lan").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        let lang_doc = s
            .get("lan_doc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if lang.is_empty() {
            continue;
        }
        out.push(SubtitleTrack { lang, lang_doc, url });
    }
    Ok(out)
}

fn format_srt_time(sec: f64) -> String {
    let ms_total = (sec.max(0.0) * 1000.0).round() as u64;
    let ms = ms_total % 1000;
    let s_total = ms_total / 1000;
    let s = s_total % 60;
    let m_total = s_total / 60;
    let m = m_total % 60;
    let h = m_total / 60;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

pub fn bcc_json_to_srt(json: &Value) -> Result<String, BiliError> {
    let body = json
        .get("body")
        .and_then(|v| v.as_array())
        .ok_or_else(|| BiliError::Parse("bcc missing body".to_string()))?;
    let mut out = String::new();
    let mut idx: u32 = 1;
    for item in body {
        let from = item.get("from").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let to = item.get("to").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let content = item
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .replace("\r\n", "\n")
            .trim()
            .to_string();
        if content.is_empty() {
            continue;
        }
        out.push_str(&format!("{idx}\n"));
        out.push_str(&format!(
            "{} --> {}\n",
            format_srt_time(from),
            format_srt_time(to.max(from))
        ));
        out.push_str(&content);
        out.push('\n');
        out.push('\n');
        idx += 1;
    }
    Ok(out)
}

pub async fn download_subtitle_srt(
    client: &BiliClient,
    url: &str,
    cookie: Option<&str>,
) -> Result<String, BiliError> {
    let u = url.trim();
    if u.is_empty() {
        return Err(BiliError::InvalidInput("empty subtitle url".to_string()));
    }
    let headers = header_map_with_cookie(cookie);
    let json: Value = client.http.get(u).headers(headers).send().await?.json().await?;
    bcc_json_to_srt(&json)
}

