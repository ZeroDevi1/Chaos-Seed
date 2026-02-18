use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::live_directory::util::bili_wbi::BiliWbi;

use super::{
    BiliClient, BiliError, bili_check_code, header_map_with_cookie, merge_cookie_header,
};

#[derive(Debug, Clone)]
pub struct DashVideo {
    pub id: i32,
    pub base_url: String,
    pub backup_url: Vec<String>,
    pub codecs: String,
    pub codecid: i32,
    pub bandwidth: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DashAudio {
    pub id: i32,
    pub base_url: String,
    pub backup_url: Vec<String>,
    pub codecs: String,
    pub bandwidth: u64,
}

#[derive(Debug, Clone)]
pub struct PlayurlInfo {
    pub quality: i32,
    pub accept_quality: Vec<i32>,
    pub accept_description: Vec<String>,
    pub dash_videos: Vec<DashVideo>,
    pub dash_audios: Vec<DashAudio>,
}

fn now_unix_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn fetch_buvid_cookie(client: &BiliClient) -> Result<String, BiliError> {
    let base = client.endpoints.api_base.trim_end_matches('/');
    let url = format!("{base}/x/frontend/finger/spi");
    let json: Value = client
        .http
        .get(url)
        .headers(header_map_with_cookie(None))
        .send()
        .await?
        .json()
        .await?;
    bili_check_code(&json)?;
    let b3 = json.pointer("/data/b_3").and_then(|v| v.as_str()).unwrap_or("").trim();
    let b4 = json.pointer("/data/b_4").and_then(|v| v.as_str()).unwrap_or("").trim();
    if b3.is_empty() || b4.is_empty() {
        return Err(BiliError::Parse("buvid missing".to_string()));
    }
    Ok(format!("buvid3={b3}; buvid4={b4};"))
}

pub async fn ensure_buvid_cookie(client: &BiliClient) -> Result<String, BiliError> {
    if let Some(cached) = client.buvid_cookie_cached() {
        return Ok(cached);
    }
    let cookie = fetch_buvid_cookie(client).await?;
    client.set_buvid_cookie(cookie.clone());
    Ok(cookie)
}

async fn fetch_wbi_mixin_key(client: &BiliClient, buvid_cookie: &str) -> Result<String, BiliError> {
    let base = client.endpoints.api_base.trim_end_matches('/');
    let url = format!("{base}/x/web-interface/nav");
    let headers = header_map_with_cookie(Some(buvid_cookie));
    let json: Value = client.http.get(url).headers(headers).send().await?.json().await?;
    bili_check_code(&json)?;

    let img_url = json.pointer("/data/wbi_img/img_url").and_then(|v| v.as_str()).unwrap_or("").trim();
    let sub_url = json.pointer("/data/wbi_img/sub_url").and_then(|v| v.as_str()).unwrap_or("").trim();
    let img_key = img_url
        .split('/')
        .last()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .trim();
    let sub_key = sub_url
        .split('/')
        .last()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .trim();
    if img_key.is_empty() || sub_key.is_empty() {
        return Err(BiliError::Parse("wbi keys missing".to_string()));
    }
    let mixin = BiliWbi::mixin_key(&(img_key.to_string() + sub_key))
        .map_err(|e| BiliError::Parse(e.to_string()))?;
    Ok(mixin)
}

pub async fn ensure_wbi_mixin_key(client: &BiliClient) -> Result<String, BiliError> {
    if let Some(cached) = client.wbi_mixin_cached() {
        return Ok(cached);
    }
    let buvid = ensure_buvid_cookie(client).await?;
    let mixin = fetch_wbi_mixin_key(client, &buvid).await?;
    client.set_wbi_mixin(mixin.clone());
    Ok(mixin)
}

pub fn choose_qn_by_dfn_priority(accept_quality: &[i32], accept_description: &[String], dfn_priority: &str) -> Option<i32> {
    if accept_quality.is_empty() || accept_description.is_empty() {
        return None;
    }
    let desired: Vec<String> = dfn_priority
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if desired.is_empty() {
        return accept_quality.iter().max().copied();
    }

    for d in desired {
        let dl = d.to_ascii_lowercase();
        for (idx, desc) in accept_description.iter().enumerate() {
            let al = desc.trim().to_ascii_lowercase();
            let hit = al == dl || al.contains(&dl) || dl.contains(&al);
            if hit {
                if let Some(q) = accept_quality.get(idx) {
                    return Some(*q);
                }
            }
        }
    }
    accept_quality.iter().max().copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodecKind {
    Hevc,
    Av1,
    Avc,
    Other,
}

pub fn classify_video_codec(codecid: i32, codecs: &str) -> VideoCodecKind {
    let c = codecs.to_ascii_lowercase();
    if matches!(codecid, 12) || c.contains("hev1") || c.contains("hvc1") || c.contains("hevc") {
        return VideoCodecKind::Hevc;
    }
    if matches!(codecid, 13) || c.contains("av01") || c.contains("av1") {
        return VideoCodecKind::Av1;
    }
    if matches!(codecid, 7) || c.contains("avc1") || c.contains("avc") {
        return VideoCodecKind::Avc;
    }
    VideoCodecKind::Other
}

pub fn pick_dash_tracks(
    info: &PlayurlInfo,
    encoding_priority: &str,
) -> Result<(DashVideo, DashAudio), BiliError> {
    if info.dash_videos.is_empty() || info.dash_audios.is_empty() {
        return Err(BiliError::Parse("missing dash video/audio".to_string()));
    }

    let order: Vec<VideoCodecKind> = encoding_priority
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .map(|s| match s.as_str() {
            "hevc" => VideoCodecKind::Hevc,
            "av1" => VideoCodecKind::Av1,
            "avc" => VideoCodecKind::Avc,
            _ => VideoCodecKind::Other,
        })
        .collect();

    let mut chosen_video: Option<DashVideo> = None;
    for k in order {
        let mut best: Option<&DashVideo> = None;
        for v in &info.dash_videos {
            if classify_video_codec(v.codecid, &v.codecs) != k {
                continue;
            }
            if best.map(|b| v.bandwidth > b.bandwidth).unwrap_or(true) {
                best = Some(v);
            }
        }
        if let Some(v) = best {
            chosen_video = Some(v.clone());
            break;
        }
    }
    let chosen_video = chosen_video.unwrap_or_else(|| {
        info.dash_videos
            .iter()
            .max_by_key(|v| v.bandwidth)
            .expect("non-empty")
            .clone()
    });

    let chosen_audio = info
        .dash_audios
        .iter()
        .max_by_key(|a| a.bandwidth)
        .expect("non-empty")
        .clone();

    Ok((chosen_video, chosen_audio))
}

pub async fn fetch_playurl_dash(
    client: &BiliClient,
    bvid: &str,
    aid: &str,
    cid: &str,
    qn: i32,
    cookie: Option<&str>,
) -> Result<PlayurlInfo, BiliError> {
    let bv = bvid.trim();
    let a = aid.trim();
    let c = cid.trim();
    if bv.is_empty() || a.is_empty() || c.is_empty() {
        return Err(BiliError::InvalidInput("missing bvid/aid/cid".to_string()));
    }

    let buvid = ensure_buvid_cookie(client).await?;
    let mixin = ensure_wbi_mixin_key(client).await?;

    let mut params: Vec<(String, String)> = vec![
        ("bvid".to_string(), bv.to_string()),
        ("avid".to_string(), a.to_string()),
        ("cid".to_string(), c.to_string()),
        ("fnval".to_string(), "4048".to_string()),
        ("fnver".to_string(), "0".to_string()),
        ("fourk".to_string(), "1".to_string()),
        ("otype".to_string(), "json".to_string()),
        ("qn".to_string(), qn.to_string()),
        ("support_multi_audio".to_string(), "true".to_string()),
        ("from_client".to_string(), "BROWSER".to_string()),
    ];

    // When not logged-in, allow trial (some contents).
    if cookie.map(|s| s.trim().is_empty()).unwrap_or(true) {
        params.push(("try_look".to_string(), "1".to_string()));
    }

    let signed = BiliWbi::sign_query(&params, &mixin, now_unix_s());
    let url = format!(
        "{}/x/player/wbi/playurl",
        client.endpoints.api_base.trim_end_matches('/')
    );

    let merged_cookie = merge_cookie_header(Some(&buvid), cookie);
    let mut headers = header_map_with_cookie(merged_cookie.as_deref());
    headers.insert(reqwest::header::REFERER, reqwest::header::HeaderValue::from_static("https://www.bilibili.com/"));

    let resp = client
        .http
        .get(url)
        .headers(headers)
        .query(&signed)
        .send()
        .await?;
    let json: Value = resp.json().await?;
    bili_check_code(&json)?;
    let data = json.get("data").ok_or_else(|| BiliError::Parse("missing data".to_string()))?;

    let quality = data.get("quality").and_then(|v| v.as_i64()).unwrap_or(qn as i64) as i32;
    let accept_quality: Vec<i32> = data
        .get("accept_quality")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_i64().map(|v| v as i32))
                .collect()
        })
        .unwrap_or_default();
    let accept_description: Vec<String> = data
        .get("accept_description")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let mut dash_videos: Vec<DashVideo> = Vec::new();
    let mut dash_audios: Vec<DashAudio> = Vec::new();
    if let Some(dash) = data.get("dash") {
        if let Some(vs) = dash.get("video").and_then(|v| v.as_array()) {
            for v in vs {
                let id = v.get("id").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                let base_url = v.get("base_url").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
                if id == 0 || base_url.is_empty() {
                    continue;
                }
                let backup_url: Vec<String> = v
                    .get("backup_url")
                    .and_then(|x| x.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let codecs = v.get("codecs").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
                let codecid = v.get("codecid").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                let bandwidth = v.get("bandwidth").and_then(|x| x.as_u64()).unwrap_or(0);
                let width = v.get("width").and_then(|x| x.as_u64()).map(|v| v as u32);
                let height = v.get("height").and_then(|x| x.as_u64()).map(|v| v as u32);
                let frame_rate = v.get("frame_rate").and_then(|x| x.as_str()).map(|s| s.to_string());
                dash_videos.push(DashVideo {
                    id,
                    base_url,
                    backup_url,
                    codecs,
                    codecid,
                    bandwidth,
                    width,
                    height,
                    frame_rate,
                });
            }
        }
        if let Some(as_) = dash.get("audio").and_then(|v| v.as_array()) {
            for a in as_ {
                let id = a.get("id").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                let base_url = a.get("base_url").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
                if id == 0 || base_url.is_empty() {
                    continue;
                }
                let backup_url: Vec<String> = a
                    .get("backup_url")
                    .and_then(|x| x.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let codecs = a.get("codecs").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
                let bandwidth = a.get("bandwidth").and_then(|x| x.as_u64()).unwrap_or(0);
                dash_audios.push(DashAudio {
                    id,
                    base_url,
                    backup_url,
                    codecs,
                    bandwidth,
                });
            }
        }
    }

    if dash_videos.is_empty() || dash_audios.is_empty() {
        return Err(BiliError::Api("no dash tracks (need login or permission?)".to_string()));
    }

    Ok(PlayurlInfo {
        quality,
        accept_quality,
        accept_description,
        dash_videos,
        dash_audios,
    })
}
