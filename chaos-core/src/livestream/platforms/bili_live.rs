use serde_json::Value;

use crate::danmaku::model::Site;

use super::super::client::{LivestreamConfig, LivestreamError};
use super::super::model::{LiveInfo, LiveManifest, PlaybackHints, ResolveOptions, StreamVariant};
use super::super::util::mbga;

fn get_i64(v: &Value, ptr: &str) -> Option<i64> {
    v.pointer(ptr).and_then(|v| v.as_i64())
}

fn get_bool(v: &Value, ptr: &str) -> Option<bool> {
    v.pointer(ptr).and_then(|v| v.as_bool())
}

fn get_str(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn make_variant_id(qn: i32, label: &str) -> String {
    format!("bili_live:{qn}:{label}")
}

async fn get_json(http: &reqwest::Client, url: &str) -> Result<Value, LivestreamError> {
    let resp = http.get(url).send().await?.error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

async fn get_text(http: &reqwest::Client, url: &str) -> Result<String, LivestreamError> {
    let resp = http.get(url).send().await?.error_for_status()?;
    Ok(resp.text().await?)
}

fn parse_room_playinfo_value(v: &Value) -> Result<Vec<StreamVariant>, LivestreamError> {
    if get_bool(v, "/data/encrypted").unwrap_or(false)
        && !get_bool(v, "/data/pwd_verified").unwrap_or(true)
    {
        return Err(LivestreamError::NeedPassword);
    }

    let qn_desc = v
        .pointer("/data/playurl_info/playurl/g_qn_desc")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LivestreamError::Parse("missing g_qn_desc".to_string()))?;

    let streams = v
        .pointer("/data/playurl_info/playurl/stream")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LivestreamError::Parse("missing stream".to_string()))?;

    // Find codec in priority order: http_stream/flv/avc, else http_hls/fmp4/avc.
    let codec = find_codec(streams, "http_stream", "flv", "avc")
        .or_else(|| find_codec(streams, "http_hls", "fmp4", "avc"))
        .ok_or_else(|| LivestreamError::Parse("no suitable codec".to_string()))?;

    let current_qn = codec
        .get("current_qn")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| LivestreamError::Parse("missing current_qn".to_string()))?
        as i32;

    let accept_qn: Vec<i32> = codec
        .get("accept_qn")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_i64().map(|n| n as i32))
        .collect();

    let base_url = codec
        .get("base_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LivestreamError::Parse("missing base_url".to_string()))?;

    let url_info = codec
        .get("url_info")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LivestreamError::Parse("missing url_info".to_string()))?;

    let mut urls: Vec<String> = url_info
        .iter()
        .filter_map(|ui| {
            let host = ui.get("host")?.as_str()?;
            let extra = ui.get("extra")?.as_str().unwrap_or("");
            Some(format!("{host}{base_url}{extra}"))
        })
        .collect();

    urls = mbga::sort_urls(&urls);

    let mut out: Vec<StreamVariant> = Vec::new();
    for item in qn_desc {
        let qn = item.get("qn").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
        let label = item
            .get("desc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if qn <= 0 || label.is_empty() {
            continue;
        }
        if !accept_qn.contains(&qn) {
            continue;
        }
        let mut v = StreamVariant {
            id: make_variant_id(qn, &label),
            label,
            quality: qn,
            rate: None,
            url: None,
            backup_urls: vec![],
        };
        if qn == current_qn && !urls.is_empty() {
            v.url = Some(urls[0].clone());
            v.backup_urls = urls[1..].to_vec();
        }
        out.push(v);
    }

    Ok(out)
}

fn find_codec<'a>(
    streams: &'a [Value],
    protocol: &str,
    format_name: &str,
    codec_name: &str,
) -> Option<&'a Value> {
    streams
        .iter()
        .find(|s| s.get("protocol_name").and_then(|v| v.as_str()) == Some(protocol))
        .and_then(|s| s.get("format")?.as_array())
        .and_then(|formats| {
            formats
                .iter()
                .find(|f| f.get("format_name").and_then(|v| v.as_str()) == Some(format_name))
        })
        .and_then(|f| f.get("codec")?.as_array())
        .and_then(|codecs| {
            codecs
                .iter()
                .find(|c| c.get("codec_name").and_then(|v| v.as_str()) == Some(codec_name))
        })
}

async fn fetch_room_play_info(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<Vec<StreamVariant>, LivestreamError> {
    let base = cfg.endpoints.bili_api_base.trim_end_matches('/');
    let url = format!(
        "{base}/xlive/web-room/v2/index/getRoomPlayInfo?room_id={rid}&protocol=0,1&format=0,1,2&codec=0,1&qn={qn}&platform=web&ptype=8&dolby=5"
    );
    let json = get_json(http, &url).await?;
    parse_room_playinfo_value(&json)
}

async fn fetch_play_url(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<Vec<StreamVariant>, LivestreamError> {
    let base = cfg.endpoints.bili_api_base.trim_end_matches('/');
    let url = format!("{base}/room/v1/Room/playUrl?cid={rid}&qn={qn}&platform=web");
    let json = get_json(http, &url).await?;

    let current_qn = get_i64(&json, "/data/current_qn")
        .ok_or_else(|| LivestreamError::Parse("missing data.current_qn".to_string()))?
        as i32;
    let qn_desc = json
        .pointer("/data/quality_description")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LivestreamError::Parse("missing data.quality_description".to_string()))?;
    let mut urls: Vec<String> = Vec::new();
    if let Some(durl) = json.pointer("/data/durl").and_then(|v| v.as_array()) {
        urls = durl
            .iter()
            .filter_map(|d| d.get("url").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
    }
    urls = mbga::sort_urls(&urls);

    let mut out: Vec<StreamVariant> = Vec::new();
    for item in qn_desc {
        let qn = item.get("qn").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
        let label = item
            .get("desc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if qn <= 0 || label.is_empty() {
            continue;
        }
        let mut v = StreamVariant {
            id: make_variant_id(qn, &label),
            label,
            quality: qn,
            rate: None,
            url: None,
            backup_urls: vec![],
        };
        if qn == current_qn && !urls.is_empty() {
            v.url = Some(urls[0].clone());
            v.backup_urls = urls[1..].to_vec();
        }
        out.push(v);
    }
    Ok(out)
}

async fn fetch_html_fallback(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<Vec<StreamVariant>, LivestreamError> {
    let base = cfg.endpoints.bili_live_base.trim_end_matches('/');
    let url = format!("{base}/{rid}");
    let text = get_text(http, &url).await?;
    let blob = text
        .split("<script>window.__NEPTUNE_IS_MY_WAIFU__=")
        .nth(1)
        .and_then(|s| s.split("</script>").next())
        .map(|s| s.trim())
        .ok_or_else(|| LivestreamError::Parse("missing __NEPTUNE_IS_MY_WAIFU__".to_string()))?;

    let json: Value = serde_json::from_str(blob)?;
    let room_init = json
        .get("roomInitRes")
        .ok_or_else(|| LivestreamError::Parse("missing roomInitRes".to_string()))?;

    // Some pages may not support the requested qn; still parse and return what we can.
    let mut vars = parse_room_playinfo_value(room_init)?;
    // Prefer the requested qn variant id format for resolve_variant convenience.
    for v in &mut vars {
        v.id = make_variant_id(v.quality, &v.label);
    }
    // If server didn't switch qn, leave it as-is.
    let _ = qn;
    Ok(vars)
}

async fn fetch_playinfo(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<Vec<StreamVariant>, LivestreamError> {
    match fetch_room_play_info(http, cfg, rid, qn).await {
        Ok(v) => Ok(v),
        Err(e @ LivestreamError::NeedPassword) => Err(e),
        Err(_) => match fetch_play_url(http, cfg, rid, qn).await {
            Ok(v) => Ok(v),
            Err(_) => fetch_html_fallback(http, cfg, rid, qn).await,
        },
    }
}

fn apply_drop_inaccessible(
    mut vars: Vec<StreamVariant>,
    opt: ResolveOptions,
) -> Vec<StreamVariant> {
    if !opt.drop_inaccessible_high_qualities {
        return vars;
    }
    let resolved_q = vars
        .iter()
        .filter(|v| v.url.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .map(|v| v.quality)
        .max();
    if let Some(q) = resolved_q {
        vars.retain(|v| v.quality <= q);
    }
    vars
}

pub async fn decode_manifest(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    room_id: &str,
    raw_input: &str,
    opt: ResolveOptions,
) -> Result<LiveManifest, LivestreamError> {
    let rid_input = room_id.trim();
    if rid_input.is_empty() {
        return Err(LivestreamError::InvalidInput("empty room id".to_string()));
    }

    let base = cfg.endpoints.bili_api_base.trim_end_matches('/');
    let url = format!("{base}/room/v1/Room/get_info?room_id={rid_input}");
    let json = get_json(http, &url).await?;
    let rid = get_i64(&json, "/data/room_id")
        .ok_or_else(|| LivestreamError::Parse("missing data.room_id".to_string()))?;

    let title = get_str(&json, "/data/title").unwrap_or_default();
    let is_living = get_i64(&json, "/data/live_status").unwrap_or(0) == 1;
    let cover = get_str(&json, "/data/user_cover").filter(|s| !s.trim().is_empty());

    let mut name: Option<String> = None;
    let mut avatar: Option<String> = None;
    let anchor_url = format!("{base}/live_user/v1/UserInfo/get_anchor_in_room?roomid={rid}");
    if let Ok(anchor) = get_json(http, &anchor_url).await {
        name = get_str(&anchor, "/data/info/uname").filter(|s| !s.trim().is_empty());
        avatar = get_str(&anchor, "/data/info/face").filter(|s| !s.trim().is_empty());
    }

    let info = LiveInfo {
        title,
        name,
        avatar,
        cover,
        is_living,
    };

    let vars = fetch_playinfo(http, cfg, rid, 30000).await?;
    let mut vars = apply_drop_inaccessible(vars, opt);
    vars.sort_by(|a, b| b.quality.cmp(&a.quality));

    Ok(LiveManifest {
        site: Site::BiliLive,
        room_id: rid.to_string(),
        raw_input: raw_input.to_string(),
        info,
        playback: PlaybackHints {
            referer: Some("https://live.bilibili.com/".to_string()),
            user_agent: None,
        },
        variants: vars,
    })
}

pub async fn resolve_variant(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    room_id: &str,
    variant_id: &str,
) -> Result<StreamVariant, LivestreamError> {
    let rid: i64 = room_id
        .trim()
        .parse()
        .map_err(|_| LivestreamError::InvalidInput("invalid room_id".to_string()))?;

    let vid = variant_id.trim();
    let (_, rest) = vid
        .split_once(':')
        .ok_or_else(|| LivestreamError::InvalidInput("invalid variant_id".to_string()))?;
    let (qn_str, _label) = rest
        .split_once(':')
        .ok_or_else(|| LivestreamError::InvalidInput("invalid variant_id".to_string()))?;
    let qn: i32 = qn_str
        .parse()
        .map_err(|_| LivestreamError::InvalidInput("invalid qn".to_string()))?;

    let vars = fetch_playinfo(http, cfg, rid, qn).await?;
    let mut v = vars
        .into_iter()
        .find(|v| v.quality == qn)
        .ok_or_else(|| LivestreamError::Parse("variant not found".to_string()))?;
    v.id = make_variant_id(qn, &v.label);
    Ok(v)
}
