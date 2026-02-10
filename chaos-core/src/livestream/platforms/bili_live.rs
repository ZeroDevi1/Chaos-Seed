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

fn parse_room_playinfo_value(
    v: &Value,
    requested_qn: Option<i32>,
) -> Result<Vec<StreamVariant>, LivestreamError> {
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
        // When resolving a specific qn, only bind URLs if the server actually switched to that qn.
        // Some rooms ignore `qn` in this endpoint and always return a low `current_qn`. Binding
        // blindly would create a "high label, low url" mismatch.
        let should_bind_url = match requested_qn {
            Some(r) => r == qn && current_qn == r,
            None => qn == current_qn,
        };
        if should_bind_url && !urls.is_empty() {
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
    parse_room_playinfo_value(&json, if qn > 0 { Some(qn) } else { None })
}

async fn fetch_room_play_info_list(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
) -> Result<Vec<StreamVariant>, LivestreamError> {
    // IMPORTANT: for quality enumeration, align with dart_simple_live and do NOT pass `qn`.
    // Some rooms appear to return a "collapsed" accept_qn list when `qn=0` is provided.
    let base = cfg.endpoints.bili_api_base.trim_end_matches('/');
    let url = format!(
        "{base}/xlive/web-room/v2/index/getRoomPlayInfo?room_id={rid}&protocol=0,1&format=0,1,2&codec=0,1&platform=web&ptype=8&dolby=5"
    );
    let json = get_json(http, &url).await?;
    parse_room_playinfo_value(&json, None)
}

async fn fetch_playinfo_list(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
) -> Result<Vec<StreamVariant>, LivestreamError> {
    match fetch_room_play_info_list(http, cfg, rid).await {
        Ok(v) => Ok(v),
        Err(e @ LivestreamError::NeedPassword) => Err(e),
        Err(_) => match fetch_play_url(http, cfg, rid, 0).await {
            Ok(v) => Ok(v),
            Err(_) => fetch_html_fallback(http, cfg, rid, 0).await,
        },
    }
}

async fn fetch_play_url(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<Vec<StreamVariant>, LivestreamError> {
    let requested_qn = qn;
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
        let desc_qn = item.get("qn").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
        let label = item
            .get("desc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if desc_qn <= 0 || label.is_empty() {
            continue;
        }
        let mut v = StreamVariant {
            id: make_variant_id(desc_qn, &label),
            label,
            quality: desc_qn,
            rate: None,
            url: None,
            backup_urls: vec![],
        };
        // Bind URLs only if the server actually switched to the requested qn.
        if requested_qn > 0 {
            if desc_qn == requested_qn && current_qn == requested_qn && !urls.is_empty() {
                v.url = Some(urls[0].clone());
                v.backup_urls = urls[1..].to_vec();
            }
        } else if desc_qn == current_qn && !urls.is_empty() {
            // No specific qn requested: attach to the actual current_qn (legacy behavior).
            v.url = Some(urls[0].clone());
            v.backup_urls = urls[1..].to_vec();
        }
        out.push(v);
    }
    Ok(out)
}

fn pick_variant_with_url(vars: Vec<StreamVariant>, qn: i32) -> Option<StreamVariant> {
    vars.into_iter().find(|v| {
        v.quality == qn && v.url.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false)
    })
}

async fn resolve_variant_for_qn(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<Option<StreamVariant>, LivestreamError> {
    match fetch_room_play_info(http, cfg, rid, qn).await {
        Ok(vars) => {
            if let Some(v) = pick_variant_with_url(vars, qn) {
                return Ok(Some(v));
            }
        }
        Err(e @ LivestreamError::NeedPassword) => return Err(e),
        Err(_) => {}
    }

    match fetch_play_url(http, cfg, rid, qn).await {
        Ok(vars) => {
            if let Some(v) = pick_variant_with_url(vars, qn) {
                return Ok(Some(v));
            }
        }
        Err(_) => {}
    }

    match fetch_html_fallback(http, cfg, rid, qn).await {
        Ok(vars) => Ok(pick_variant_with_url(vars, qn)),
        Err(e @ LivestreamError::NeedPassword) => Err(e),
        Err(_) => Ok(None),
    }
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
    let mut vars = parse_room_playinfo_value(room_init, if qn > 0 { Some(qn) } else { None })?;
    // Prefer the requested qn variant id format for resolve_variant convenience.
    for v in &mut vars {
        v.id = make_variant_id(v.quality, &v.label);
    }
    // If server didn't switch qn, leave it as-is.
    let _ = qn;
    Ok(vars)
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

    // First: list all available qualities (accept_qn + g_qn_desc).
    let mut vars = fetch_playinfo_list(http, cfg, rid).await?;

    // Second: resolve the best accessible quality (highest qn that returns a url) so we don't
    // end up with a single low-quality variant after `drop_inaccessible_high_qualities`.
    //
    // This mirrors dart_simple_live behavior: list qualities from accept_qn, then fetch URLs for
    // the chosen quality.
    {
        let mut qns: Vec<i32> = vars.iter().map(|v| v.quality).collect();
        qns.sort_by(|a, b| b.cmp(a));
        qns.dedup();

        // Cap attempts to avoid excessive requests on flaky networks.
        for qn in qns.into_iter().take(8) {
            let already_has_url = vars.iter().any(|v| {
                v.quality == qn && v.url.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
            });
            if already_has_url {
                break;
            }

            if let Ok(Some(rv)) = resolve_variant_for_qn(http, cfg, rid, qn).await {
                if let Some(dst) = vars.iter_mut().find(|v| v.quality == qn) {
                    dst.url = rv.url;
                    dst.backup_urls = rv.backup_urls;
                }
                break;
            }
        }
    }

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

    let mut v = resolve_variant_for_qn(http, cfg, rid, qn)
        .await?
        .ok_or_else(|| LivestreamError::Parse("requested quality not accessible".to_string()))?;
    v.id = make_variant_id(qn, &v.label);
    Ok(v)
}
