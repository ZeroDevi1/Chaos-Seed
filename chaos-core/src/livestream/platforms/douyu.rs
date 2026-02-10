use serde_json::Value;

use crate::danmaku::model::Site;

use super::super::client::{EnvConfig, LivestreamConfig, LivestreamError};
use super::super::model::{LiveInfo, LiveManifest, PlaybackHints, ResolveOptions, StreamVariant};
use super::super::util::{brace_extract, douyu_auth};

fn get_i64(v: &Value, ptr: &str) -> Option<i64> {
    v.pointer(ptr).and_then(|v| v.as_i64())
}

fn get_str(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn make_variant_id(rate: i32, label: &str) -> String {
    format!("douyu:{rate}:{label}")
}

async fn get_text(http: &reqwest::Client, url: &str) -> Result<String, LivestreamError> {
    let resp = http.get(url).send().await?.error_for_status()?;
    Ok(resp.text().await?)
}

async fn get_json(http: &reqwest::Client, url: &str) -> Result<Value, LivestreamError> {
    let resp = http.get(url).send().await?.error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

async fn post_form_json(
    http: &reqwest::Client,
    url: &str,
    form: &std::collections::BTreeMap<&str, String>,
) -> Result<Value, LivestreamError> {
    let resp = http.post(url).form(form).send().await?.error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

fn unescape_backslash_quotes(mut s: String) -> String {
    // Swift does it twice; keep same behavior.
    for _ in 0..2 {
        if s.contains("\\\"") {
            s = s.replace("\\\"", "\"");
        }
    }
    s
}

fn parse_room_id_from_html(html: &str) -> Result<(i64, bool), LivestreamError> {
    if let Some(obj) = brace_extract::extract_balanced_object_after_marker(html, "\\\"roomInfo\\\"")
        .or_else(|| brace_extract::extract_balanced_object_after_marker(html, "\"roomInfo\""))
        .or_else(|| brace_extract::extract_balanced_object_after_marker(html, "roomInfo"))
    {
        let obj = unescape_backslash_quotes(obj);
        let json: Value = serde_json::from_str(&obj)?;
        let rid = get_i64(&json, "/room/room_id")
            .ok_or_else(|| LivestreamError::Parse("douyu: missing room.room_id".to_string()))?;
        let is_living = get_i64(&json, "/room/show_status").unwrap_or(0) == 1;
        return Ok((rid, is_living));
    }

    // Fallback: regex scan (handles pages where the blob isn't a clean JSON object).
    let rid_re = regex::Regex::new(r#"room_id\\?"?\s*[:=]\s*"?(\d+)"?"#)
        .map_err(|e| LivestreamError::Parse(format!("douyu: regex error: {e}")))?;
    let show_re = regex::Regex::new(r#"show_status\\?"?\s*[:=]\s*"?(\d+)"?"#)
        .map_err(|e| LivestreamError::Parse(format!("douyu: regex error: {e}")))?;
    let rid = rid_re
        .captures(html)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .ok_or_else(|| LivestreamError::Parse("douyu: missing room_id".to_string()))?;
    let is_living = show_re
        .captures(html)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or(0)
        == 1;
    Ok((rid, is_living))
}

fn parse_betard_info(json: &Value) -> Result<LiveInfo, LivestreamError> {
    let title = get_str(json, "/room/room_name").unwrap_or_default();
    let name = get_str(json, "/room/nickname").filter(|s| !s.trim().is_empty());
    let avatar = get_str(json, "/room/avatar/big").filter(|s| !s.trim().is_empty());
    let cover = get_str(json, "/room/room_pic").filter(|s| !s.trim().is_empty());
    let is_living = match json.pointer("/room/show_status") {
        Some(v) => match (v.as_i64(), v.as_str()) {
            (Some(n), _) => n == 1,
            (_, Some(s)) => s == "1",
            _ => false,
        },
        None => false,
    };
    Ok(LiveInfo {
        title,
        name,
        avatar,
        cover,
        is_living,
    })
}

fn stable_uuid_like(env: &EnvConfig) -> String {
    // Deterministic across tests via seeded RNG.
    let a = env.rng.lock().expect("rng").u64(..);
    format!("{a:016x}")
}

fn build_play_urls(
    cfg: &LivestreamConfig,
    env: &EnvConfig,
    rtmp_url: &str,
    rtmp_live: &str,
    p2p_meta: Option<&Value>,
    cdn_hosts: &[String],
) -> (String, Vec<String>) {
    let flv_url = format!("{}/{}", rtmp_url.trim_end_matches('/'), rtmp_live);

    let mut p2p_urls: Vec<String> = Vec::new();
    let Some(meta) = p2p_meta else {
        return (flv_url, p2p_urls);
    };

    let domain = meta
        .get("xp2p_domain")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if domain.is_empty() {
        return (flv_url, p2p_urls);
    }

    let delay = meta
        .get("xp2p_txDelay")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let secret = meta
        .get("xp2p_txSecret")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let time = meta
        .get("xp2p_txTime")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let replaced = rtmp_live.replace("flv", "xs");
    let mut parts: Vec<String> = replaced.split('&').map(|s| s.to_string()).collect();
    parts.push(format!("delay={delay}"));
    parts.push(format!("txSecret={secret}"));
    parts.push(format!("txTime={time}"));
    parts.push(format!("uuid={}", stable_uuid_like(env)));
    let xs_string = format!("{domain}/live/{}", parts.join("&"));

    for h in cdn_hosts {
        if h.trim().is_empty() {
            continue;
        }
        p2p_urls.push(format!(
            "{}://{h}/{xs_string}",
            cfg.endpoints.douyu_p2p_scheme
        ));
    }

    (flv_url, p2p_urls)
}

fn build_cdn_url(cfg: &LivestreamConfig, xp2p_domain: &str, rtmp_live: &str) -> Option<String> {
    let prefix = rtmp_live.split('.').next().unwrap_or("").trim();
    if prefix.is_empty() || xp2p_domain.trim().is_empty() {
        return None;
    }
    Some(format!(
        "{}://{}/{}.xs",
        cfg.endpoints.douyu_cdn_scheme,
        xp2p_domain.trim(),
        prefix
    ))
}

async fn fetch_encryption(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    did: &str,
) -> Result<douyu_auth::DouyuEncryption, LivestreamError> {
    let base = cfg.endpoints.douyu_base.trim_end_matches('/');
    let url = format!("{base}/wgapi/livenc/liveweb/websec/getEncryption?did={did}");
    let json = get_json(http, &url).await?;
    let err = get_i64(&json, "/error").unwrap_or(-1);
    if err != 0 {
        return Err(LivestreamError::Parse(
            "douyu: encryption error".to_string(),
        ));
    }
    let data = json
        .get("data")
        .ok_or_else(|| LivestreamError::Parse("douyu: missing data".to_string()))?;
    Ok(douyu_auth::DouyuEncryption {
        key: get_str(data, "/key").unwrap_or_default(),
        rand_str: get_str(data, "/rand_str").unwrap_or_default(),
        enc_time: get_i64(data, "/enc_time").unwrap_or(0) as i32,
        enc_data: get_str(data, "/enc_data").unwrap_or_default(),
        is_special: get_i64(data, "/is_special").unwrap_or(0) as i32,
    })
}

async fn fetch_h5_play(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    env: &EnvConfig,
    rid: i64,
    rate: i32,
) -> Result<(Vec<StreamVariant>, i32), LivestreamError> {
    let did = env.douyu_did();
    let enc = fetch_encryption(http, cfg, &did).await?;
    let ts = (env.now_s)();
    let auth = enc.auth(&rid.to_string(), ts);

    let mut form = std::collections::BTreeMap::<&str, String>::new();
    form.insert("enc_data", enc.enc_data.clone());
    form.insert("tt", ts.to_string());
    form.insert("did", did);
    form.insert("auth", auth);
    form.insert("cdn", "".to_string());
    form.insert("rate", rate.to_string());
    // Prefer H.264 for maximum decoder compatibility (aligned with dart_simple_live).
    form.insert("hevc", "0".to_string());
    form.insert("fa", "0".to_string());
    form.insert("ive", "0".to_string());

    let base = cfg.endpoints.douyu_base.trim_end_matches('/');
    let url = format!("{base}/lapi/live/getH5PlayV1/{rid}");
    let json = post_form_json(http, &url, &form).await?;
    let data = json
        .get("data")
        .ok_or_else(|| LivestreamError::Parse("douyu: missing data".to_string()))?;

    let current_rate = get_i64(data, "/rate").unwrap_or(0) as i32;
    let rtmp_url = get_str(data, "/rtmp_url").unwrap_or_default();
    let rtmp_live = get_str(data, "/rtmp_live").unwrap_or_default();
    let multirates = data
        .get("multirates")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LivestreamError::Parse("douyu: missing multirates".to_string()))?;

    // Fetch CDN list if p2pMeta exists.
    let mut cdn_hosts: Vec<String> = Vec::new();
    let p2p_meta = data.get("p2pMeta");
    if let Some(meta) = p2p_meta {
        if let Some(domain) = meta.get("xp2p_domain").and_then(|v| v.as_str()) {
            if let Some(cdn_url) = build_cdn_url(cfg, domain, &rtmp_live) {
                let cdn_json = get_json(http, &cdn_url).await?;
                for k in ["sug", "bak"] {
                    if let Some(arr) = cdn_json.get(k).and_then(|v| v.as_array()) {
                        for it in arr {
                            if let Some(s) = it.as_str() {
                                cdn_hosts.push(s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let (flv_url, p2p_urls) =
        build_play_urls(cfg, env, &rtmp_url, &rtmp_live, p2p_meta, &cdn_hosts);
    // Prefer direct FLV first; keep P2P/xs URLs as backups (they can be flaky in some players).
    let mut urls: Vec<String> = Vec::new();
    urls.push(flv_url.clone());
    urls.extend(p2p_urls);

    let mut variants: Vec<StreamVariant> = Vec::new();
    for mr in multirates {
        let label = mr
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mr_rate = mr.get("rate").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
        let bit = mr.get("bit").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
        if label.is_empty() || mr_rate < 0 || bit < 0 {
            continue;
        }
        let mut v = StreamVariant {
            id: make_variant_id(mr_rate, &label),
            label,
            quality: bit,
            rate: Some(mr_rate),
            url: None,
            backup_urls: vec![],
        };
        if mr_rate == current_rate && !urls.is_empty() {
            v.url = Some(urls[0].clone());
            v.backup_urls = urls[1..].to_vec();
        }
        variants.push(v);
    }

    variants.sort_by(|a, b| b.quality.cmp(&a.quality));
    Ok((variants, current_rate))
}

pub async fn decode_manifest(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    room_id: &str,
    raw_input: &str,
    _opt: ResolveOptions,
) -> Result<LiveManifest, LivestreamError> {
    let base = cfg.endpoints.douyu_base.trim_end_matches('/');
    let url = format!("{base}/{}", room_id.trim_matches('/'));
    let html = get_text(http, &url).await?;
    let (rid, is_living) = parse_room_id_from_html(&html)?;

    let betard_url = format!("{base}/betard/{rid}");
    let betard = get_json(http, &betard_url).await?;
    let mut info = parse_betard_info(&betard)?;
    // Prefer the live_status from roomInfo JSON if present.
    info.is_living = is_living;

    let (variants, _) = fetch_h5_play(http, cfg, &cfg.env, rid, 0).await?;

    Ok(LiveManifest {
        site: Site::Douyu,
        room_id: rid.to_string(),
        raw_input: raw_input.to_string(),
        info,
        playback: PlaybackHints {
            referer: Some("https://www.douyu.com/".to_string()),
            user_agent: None,
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
    let rid: i64 = room_id
        .trim()
        .parse()
        .map_err(|_| LivestreamError::InvalidInput("invalid room_id".to_string()))?;

    let vid = variant_id.trim();
    let (_, rest) = vid
        .split_once(':')
        .ok_or_else(|| LivestreamError::InvalidInput("invalid variant_id".to_string()))?;
    let (rate_str, _label) = rest
        .split_once(':')
        .ok_or_else(|| LivestreamError::InvalidInput("invalid variant_id".to_string()))?;
    let rate: i32 = rate_str
        .parse()
        .map_err(|_| LivestreamError::InvalidInput("invalid rate".to_string()))?;

    let (vars, current_rate) = fetch_h5_play(http, cfg, &cfg.env, rid, rate).await?;
    let mut v = vars
        .into_iter()
        .find(|v| v.rate == Some(rate))
        .ok_or_else(|| LivestreamError::Parse("variant not found".to_string()))?;

    // If server didn't honor the rate, best-effort fall back to the resolved one.
    if v.url.is_none() && current_rate != rate {
        // no-op: keep empty url, caller can decide.
    }

    v.id = make_variant_id(rate, &v.label);
    Ok(v)
}
