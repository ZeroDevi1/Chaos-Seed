use serde_json::Value;
use tracing::{debug, warn};

use reqwest::header::{COOKIE, ORIGIN, REFERER, USER_AGENT};
use std::sync::{Mutex, OnceLock};

use crate::danmaku::model::Site;

use super::super::client::{LivestreamConfig, LivestreamError};
use super::super::model::{LiveInfo, LiveManifest, PlaybackHints, ResolveOptions, StreamVariant};
use super::super::util::mbga;

const BILI_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";
const BILI_REFERER: &str = "https://live.bilibili.com/";
const BILI_ORIGIN: &str = "https://live.bilibili.com";

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

fn buvid_cookie_cache() -> &'static Mutex<Option<String>> {
    static CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

async fn ensure_buvid_cookie(http: &reqwest::Client) -> Option<String> {
    // Cache semantics:
    // - None: never tried
    // - Some(""): tried but failed (do not retry to avoid excessive requests)
    // - Some(cookie): ok
    if let Ok(g) = buvid_cookie_cache().lock() {
        if let Some(cached) = g.clone() {
            return if cached.trim().is_empty() {
                None
            } else {
                Some(cached)
            };
        }
    }

    // Note: buvid endpoint lives on api.bilibili.com, not api.live.bilibili.com.
    let url = "https://api.bilibili.com/x/frontend/finger/spi";
    let json = http
        .get(url)
        .header(USER_AGENT, BILI_UA)
        .header(REFERER, BILI_REFERER)
        .header(ORIGIN, BILI_ORIGIN)
        .send()
        .await
        .ok()?
        .json::<Value>()
        .await
        .ok()?;

    if json.pointer("/code").and_then(|v| v.as_i64()).unwrap_or(-1) != 0 {
        let _ = buvid_cookie_cache().lock().map(|mut g| *g = Some(String::new()));
        return None;
    }

    let b3 = json
        .pointer("/data/b_3")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let b4 = json
        .pointer("/data/b_4")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if b3.is_empty() || b4.is_empty() {
        let _ = buvid_cookie_cache().lock().map(|mut g| *g = Some(String::new()));
        return None;
    }

    let cookie = format!("buvid3={b3}; buvid4={b4};");
    let _ = buvid_cookie_cache().lock().map(|mut g| *g = Some(cookie.clone()));
    Some(cookie)
}

async fn get_json(http: &reqwest::Client, url: &str) -> Result<Value, LivestreamError> {
    let mut req = http
        .get(url)
        .header(USER_AGENT, BILI_UA)
        .header(REFERER, BILI_REFERER)
        .header(ORIGIN, BILI_ORIGIN);
    if let Some(cookie) = ensure_buvid_cookie(http).await {
        req = req.header(COOKIE, cookie);
    }
    let resp = req.send().await?.error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

async fn get_text(http: &reqwest::Client, url: &str) -> Result<String, LivestreamError> {
    let mut req = http
        .get(url)
        .header(USER_AGENT, BILI_UA)
        .header(REFERER, BILI_REFERER)
        .header(ORIGIN, BILI_ORIGIN);
    if let Some(cookie) = ensure_buvid_cookie(http).await {
        req = req.header(COOKIE, cookie);
    }
    let resp = req.send().await?.error_for_status()?;
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

    // Choose the best codec branch for quality enumeration + URL binding.
    //
    // Background: some rooms return incomplete accept_qn on certain codec branches; scanning
    // all codec entries and choosing the best one helps avoid "only low quality available"
    // (blurry playback).
    let codec = pick_best_codec(streams)
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
        // 当请求指定 qn 时：只要 qn 匹配且 urls 非空就绑定 URL。
        //
        // 背景：在部分房间，接口会返回异常的 `current_qn`（并不等于请求的 qn），
        // 但返回的 URL 仍可能已经切换到目标清晰度；若严格要求 `current_qn == requested_qn`，
        // 会导致高画质永远绑定不到 URL，最终落回低清（看起来“很糊”）。
        //
        // 注意：当 `current_qn != requested_qn` 时，我们仍会继续在 `resolve_variant_for_qn`
        // 尝试更可靠的 fallback（v1 playUrl / html）；这里仅记录一次 warn 便于排查。
        let should_bind_url = match requested_qn {
            Some(r) => {
                if current_qn != r {
                    warn!(
                        "bili_live: server returned current_qn != requested_qn (current_qn={}, requested_qn={})",
                        current_qn, r
                    );
                }
                r == qn
            }
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

fn codec_accept_qn_max(codec: &Value) -> i32 {
    codec
        .get("accept_qn")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_i64().map(|n| n as i32))
        .max()
        .unwrap_or(-1)
}

fn pick_best_codec<'a>(streams: &'a [Value]) -> Option<&'a Value> {
    // Ranking rules (higher priority first):
    // 1) protocol: http_stream (flv) > http_hls (fmp4) > others
    // 2) format: flv/fmp4 preferred over others
    // 3) accept_qn max: higher is better
    // 4) codec_name: prefer avc for compatibility when quality ties
    let mut best: Option<(&Value, (u8, u8, i32, u8))> = None;

    for s in streams {
        let protocol = s
            .get("protocol_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let protocol_rank: u8 = match protocol {
            "http_stream" => 0,
            "http_hls" => 1,
            _ => 2,
        };

        let Some(formats) = s.get("format").and_then(|v| v.as_array()) else {
            continue;
        };
        for f in formats {
            let format_name = f
                .get("format_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let format_rank: u8 = match format_name {
                "flv" | "fmp4" => 0,
                _ => 1,
            };

            let Some(codecs) = f.get("codec").and_then(|v| v.as_array()) else {
                continue;
            };
            for c in codecs {
                let base_url = c.get("base_url").and_then(|v| v.as_str()).unwrap_or("").trim();
                let url_info_len = c
                    .get("url_info")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                if base_url.is_empty() || url_info_len == 0 {
                    continue;
                }

                let max_accept = codec_accept_qn_max(c);
                let codec_name = c.get("codec_name").and_then(|v| v.as_str()).unwrap_or("");
                let codec_rank: u8 = match codec_name {
                    "avc" => 0,
                    _ => 1,
                };

                let key = (protocol_rank, format_rank, max_accept, codec_rank);
                let better = match best.as_ref() {
                    None => true,
                    Some((_, bk)) => {
                        if key.0 != bk.0 {
                            key.0 < bk.0
                        } else if key.1 != bk.1 {
                            key.1 < bk.1
                        } else if key.2 != bk.2 {
                            key.2 > bk.2
                        } else {
                            key.3 < bk.3
                        }
                    }
                };

                if better {
                    best = Some((c, key));
                }
            }
        }
    }

    best.map(|(c, _)| c)
}

fn extract_v2_codec_current_qn_and_urls(v: &Value) -> Result<(i32, Vec<String>), LivestreamError> {
    let streams = v
        .pointer("/data/playurl_info/playurl/stream")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LivestreamError::Parse("missing stream".to_string()))?;

    // Choose the best codec branch for quality enumeration + URL binding.
    //
    // Background: some rooms return incomplete accept_qn on certain codec branches; scanning
    // all codec entries and choosing the best one helps avoid "only low quality available"
    // (blurry playback).
    let codec = pick_best_codec(streams)
        .ok_or_else(|| LivestreamError::Parse("no suitable codec".to_string()))?;

    let current_qn = codec
        .get("current_qn")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| LivestreamError::Parse("missing current_qn".to_string()))?
        as i32;

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
    Ok((current_qn, urls))
}

struct RoomPlayInfoV2 {
    vars: Vec<StreamVariant>,
    #[allow(dead_code)]
    current_qn: i32,
    urls: Vec<String>,
}

async fn fetch_room_play_info(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<RoomPlayInfoV2, LivestreamError> {
    let base = cfg.endpoints.bili_api_base.trim_end_matches('/');
    let url = format!(
        "{base}/xlive/web-room/v2/index/getRoomPlayInfo?room_id={rid}&protocol=0,1&format=0,1,2&codec=0,1&qn={qn}&platform=web&ptype=8&dolby=5"
    );
    let json = get_json(http, &url).await?;
    let (current_qn, urls) = extract_v2_codec_current_qn_and_urls(&json)?;
    let vars = parse_room_playinfo_value(&json, if qn > 0 { Some(qn) } else { None })?;
    Ok(RoomPlayInfoV2 {
        vars,
        current_qn,
        urls,
    })
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
        // 当请求指定 qn 时：只要 qn 匹配且 urls 非空就绑定 URL（current_qn 异常时会记录 warn）。
        if requested_qn > 0 {
            if desc_qn == requested_qn {
                if current_qn != requested_qn {
                    warn!(
                        "bili_live(playUrl): server returned current_qn != requested_qn (current_qn={}, requested_qn={})",
                        current_qn, requested_qn
                    );
                }
                if !urls.is_empty() {
                    v.url = Some(urls[0].clone());
                    v.backup_urls = urls[1..].to_vec();
                }
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
        v.quality == qn
            && v.url
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
    })
}

async fn resolve_variant_for_qn(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    rid: i64,
    qn: i32,
) -> Result<Option<StreamVariant>, LivestreamError> {
    // v2 (getRoomPlayInfo)：
    // - 若 `current_qn == requested qn` 且已绑定到 URL：直接返回。
    // - 若 `current_qn != requested qn`：认为 server 的 qn 回传不可靠，继续尝试 v1 playUrl / html fallback；
    //   若 fallback 全失败，再使用 v2 的 URL 作为最后手段（避免“高 qn 永远拿不到可播放地址”）。
    let mut v2_last_resort: Option<(Vec<StreamVariant>, Vec<String>)> = None;
    match fetch_room_play_info(http, cfg, rid, qn).await {
        Ok(info) => {
            if let Some(v) = pick_variant_with_url(info.vars.clone(), qn) {
                if info.current_qn == qn {
                    return Ok(Some(v));
                }
                // current_qn 异常：继续走 fallback；v2 URL 留作最后兜底。
                warn!(
                    "bili_live: v2 current_qn != requested qn (current_qn={}, requested_qn={}); will try fallback",
                    info.current_qn, qn
                );
            }
            if !info.urls.is_empty() {
                v2_last_resort = Some((info.vars, info.urls));
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
        Ok(vars) => {
            if let Some(v) = pick_variant_with_url(vars, qn) {
                return Ok(Some(v));
            }
        }
        Err(e @ LivestreamError::NeedPassword) => return Err(e),
        Err(_) => {}
    }

    if let Some((vars, urls)) = v2_last_resort {
        if let Some(mut v) = vars.into_iter().find(|v| v.quality == qn) {
            v.url = Some(urls[0].clone());
            v.backup_urls = urls[1..].to_vec();
            return Ok(Some(v));
        }
    }

    Ok(None)
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
                v.quality == qn
                    && v.url
                        .as_ref()
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false)
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

    let max_qn = vars.iter().map(|v| v.quality).max().unwrap_or(-1);
    let resolved_qn = vars
        .iter()
        .filter(|v| v.url.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false))
        .map(|v| v.quality)
        .max();
    debug!("bili_live: decode_manifest rid={} max_qn={} resolved_qn={:?}", rid, max_qn, resolved_qn);

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
