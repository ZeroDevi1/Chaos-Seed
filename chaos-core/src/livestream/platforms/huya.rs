use serde_json::Value;

use crate::danmaku::model::Site;

use super::super::client::{LivestreamConfig, LivestreamError};
use super::super::model::{LiveInfo, LiveManifest, PlaybackHints, ResolveOptions, StreamVariant};
use super::super::util::huya_url;

fn get_i64(v: &Value, ptr: &str) -> Option<i64> {
    v.pointer(ptr).and_then(|v| v.as_i64())
}

fn get_str(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn variant_id(bitrate: i32, label: &str) -> String {
    format!("huya:{bitrate}:{label}")
}

async fn get_text(
    http: &reqwest::Client,
    url: &str,
    ua: Option<&str>,
) -> Result<String, LivestreamError> {
    let mut req = http.get(url);
    if let Some(ua) = ua {
        req = req.header(reqwest::header::USER_AGENT, ua);
    }
    let resp = req.send().await?.error_for_status()?;
    Ok(resp.text().await?)
}

async fn get_json(
    http: &reqwest::Client,
    url: &str,
    ua: Option<&str>,
) -> Result<Value, LivestreamError> {
    let mut req = http.get(url);
    if let Some(ua) = ua {
        req = req.header(reqwest::header::USER_AGENT, ua);
    }
    let resp = req.send().await?.error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

fn pick_title(room_name: &str, intro: &str, fallback: &str) -> String {
    if !room_name.trim().is_empty() {
        room_name.to_string()
    } else if !intro.trim().is_empty() {
        intro.to_string()
    } else {
        fallback.to_string()
    }
}

fn extract_profile_room_id(html: &str) -> Option<i64> {
    // Works for both JSON (`"profileRoom": 123`) and JS object (`profileRoom: 123`).
    let re = regex::Regex::new(r#"profileRoom["']?\s*[:=]\s*"?(\d+)"?"#).ok()?;
    let caps = re.captures(html)?;
    caps.get(1)?.as_str().parse::<i64>().ok()
}

fn parse_bitrate_info(bit_rate_info: &str) -> Vec<(String, i32)> {
    let s = bit_rate_info.trim();
    if s.is_empty() {
        return vec![];
    }
    let json: Value = match serde_json::from_str(s) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let arr = match json.as_array() {
        Some(v) => v,
        None => return vec![],
    };
    let mut out = Vec::new();
    for it in arr {
        let name = it
            .get("sDisplayName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let br = it.get("iBitRate").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
        if !name.is_empty() && br >= 0 {
            out.push((name, br));
        }
    }
    out
}

fn parse_stream_infos(v: &Value) -> Vec<(String, u32, String, String, String)> {
    // (stream_name, presenter_uid, flv_url, flv_suffix, anti_code)
    let mut out = Vec::new();
    if let Some(arr) = v
        .pointer("/data/stream/baseSteamInfoList")
        .and_then(|v| v.as_array())
    {
        for it in arr {
            let s_stream_name = it
                .get("sStreamName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let presenter_uid = it
                .get("lPresenterUid")
                .and_then(|v| v.as_i64())
                .or_else(|| it.get("lPresenterUid").and_then(|v| v.as_str()?.parse::<i64>().ok()))
                .unwrap_or(0)
                .clamp(0, i64::from(u32::MAX)) as u32;
            let s_flv_url = it
                .get("sFlvUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let s_suffix = it
                .get("sFlvUrlSuffix")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let s_anti = it
                .get("sFlvAntiCode")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !s_stream_name.is_empty()
                && !s_flv_url.is_empty()
                && !s_suffix.is_empty()
                && !s_anti.is_empty()
                && presenter_uid > 0
            {
                out.push((s_stream_name, presenter_uid, s_flv_url, s_suffix, s_anti));
            }
        }
    }
    out
}

pub async fn decode_manifest(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    room_id: &str,
    raw_input: &str,
    _opt: ResolveOptions,
) -> Result<LiveManifest, LivestreamError> {
    let rid = if let Ok(n) = room_id.trim().parse::<i64>() {
        n
    } else {
        let base = cfg.endpoints.huya_base.trim_end_matches('/');
        let url = format!("{base}/{}", room_id.trim_matches('/'));
        let html = get_text(http, &url, None).await?;
        extract_profile_room_id(&html)
            .ok_or_else(|| LivestreamError::Parse("huya: missing profileRoom".to_string()))?
    };

    // Huya's mp endpoint is more stable for stream extraction.
    let mp_base = cfg.endpoints.huya_mp_base.trim_end_matches('/');
    let mp_url = format!("{mp_base}/cache.php?m=Live&do=profileRoom&roomid={rid}");
    let ua_iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0 Mobile/15E148 Safari/604.1";
    let json = get_json(http, &mp_url, Some(ua_iphone)).await?;

    let live_data = json
        .pointer("/data/liveData")
        .ok_or_else(|| LivestreamError::Parse("huya: missing data.liveData".to_string()))?;

    let room_name = get_str(live_data, "/roomName").unwrap_or_default();
    let intro = get_str(live_data, "/introduction").unwrap_or_default();
    let nick = get_str(live_data, "/nick").filter(|s| !s.trim().is_empty());
    let avatar = get_str(live_data, "/avatar180").filter(|s| !s.trim().is_empty());
    let cover = get_str(live_data, "/screenshot").filter(|s| !s.trim().is_empty());
    let title = pick_title(&room_name, &intro, nick.as_deref().unwrap_or(""));

    let live_status = get_str(&json, "/data/liveStatus").unwrap_or_default();
    let is_living = live_status == "ON";

    let canonical_rid = get_i64(live_data, "/profileRoom")
        .or_else(|| get_str(live_data, "/profileRoom").and_then(|s| s.parse::<i64>().ok()))
        .unwrap_or(rid);

    let info = LiveInfo {
        title,
        name: nick,
        avatar,
        cover,
        is_living,
    };

    let mut variants: Vec<StreamVariant> = Vec::new();
    if is_living {
        let bit_rate_info_str = get_str(live_data, "/bitRateInfo").unwrap_or_default();
        let brs = parse_bitrate_info(&bit_rate_info_str);

        let mut streams = parse_stream_infos(&json);
        // Prefer non-txdirect hosts first (ported from IINA+ `HuyaInfoMP.videos`).
        streams.sort_by_key(|(_, _, flv_url, _, _)| flv_url.contains("txdirect.flv.huya.com"));
        let now_ms = (cfg.env.now_ms)();

        for (label, bitrate) in brs {
            let mut urls: Vec<String> = streams
                .iter()
                .filter_map(|(stream_name, presenter_uid, flv_url, suffix, anti)| {
                    huya_url::format(
                        stream_name,
                        flv_url,
                        suffix,
                        anti,
                        *presenter_uid,
                        now_ms,
                        if bitrate > 0 { Some(bitrate) } else { None },
                    )
                })
                .collect();
            if urls.is_empty() {
                continue;
            }

            let url = urls.remove(0);
            let quality = if bitrate == 0 { 9_999_999 } else { bitrate };
            variants.push(StreamVariant {
                id: variant_id(bitrate, &label),
                label,
                quality,
                rate: None,
                url: Some(url),
                backup_urls: urls,
            });
        }

        variants.sort_by(|a, b| b.quality.cmp(&a.quality));
    }

    Ok(LiveManifest {
        site: Site::Huya,
        room_id: canonical_rid.to_string(),
        raw_input: raw_input.to_string(),
        info,
        playback: PlaybackHints {
            referer: Some("https://www.huya.com/".to_string()),
            user_agent: Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.3.1 Safari/605.1.15".to_string()),
        },
        variants,
    })
}

pub async fn resolve_variant(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    room_id: &str,
    variant_id: &str,
) -> Result<StreamVariant, LivestreamError> {
    // Huya is usually one-step; re-run decode and return the matching variant.
    let man = decode_manifest(http, cfg, room_id, room_id, ResolveOptions::default()).await?;
    let vid = variant_id.trim();
    let (_, rest) = vid
        .split_once(':')
        .ok_or_else(|| LivestreamError::InvalidInput("invalid variant_id".to_string()))?;
    let (bitrate_str, _) = rest
        .split_once(':')
        .ok_or_else(|| LivestreamError::InvalidInput("invalid variant_id".to_string()))?;
    let bitrate: i32 = bitrate_str
        .parse()
        .map_err(|_| LivestreamError::InvalidInput("invalid bitrate".to_string()))?;
    man.variants
        .into_iter()
        .find(|v| v.id.starts_with(&format!("huya:{bitrate}:")))
        .ok_or_else(|| LivestreamError::Parse("variant not found".to_string()))
}
