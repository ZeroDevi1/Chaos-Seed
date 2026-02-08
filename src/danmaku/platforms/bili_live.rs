use std::collections::HashMap;
use std::io::Read;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use base64::Engine;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use futures_util::{SinkExt, StreamExt};
use prost::Message as _;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use crate::danmaku::model::{
    ConnectInfo, ConnectOptions, DanmakuComment, DanmakuError, DanmakuEvent, DanmakuEventTx,
    DanmakuMethod, EmoticonMeta, ResolvedTarget, Site,
};
use crate::danmaku::proto::bili_live_dm_v2::{BizScene, Dm as DmV2, DmType};

const SERVER_URL: &str = "wss://broadcastlv.chat.bilibili.com/sub";
const HEARTBEAT_MS: u64 = 30_000;

pub async fn resolve(
    http: &reqwest::Client,
    room_id: &str,
) -> Result<ResolvedTarget, DanmakuError> {
    let short: u64 = room_id
        .trim()
        .parse()
        .map_err(|_| DanmakuError::InvalidInput(format!("invalid bilibili room id: {room_id}")))?;

    let rid = fetch_room_rid(http, short).await?;
    let (uid, img_key, sub_key) = fetch_nav_wbi(http).await?;
    let token = fetch_token(http, rid, &img_key, &sub_key).await?;
    let emoticons = fetch_emoticons(http, rid).await.unwrap_or_default();

    Ok(ResolvedTarget {
        site: Site::BiliLive,
        room_id: room_id.trim().to_string(),
        connect: ConnectInfo::BiliLive {
            rid,
            token,
            uid,
            emoticons,
        },
    })
}

pub async fn run(
    target: ResolvedTarget,
    _opt: ConnectOptions,
    tx: DanmakuEventTx,
    cancel: CancellationToken,
) -> Result<(), DanmakuError> {
    let room_id = target.room_id.clone();
    let (rid, token, uid, emoticons) = match target.connect {
        ConnectInfo::BiliLive {
            rid,
            token,
            uid,
            emoticons,
        } => (rid, token, uid, emoticons),
        _ => {
            return Err(DanmakuError::InvalidInput(
                "bili_live connector expects ConnectInfo::BiliLive".to_string(),
            ));
        }
    };

    let emoticons = Arc::new(emoticons);

    let (ws, _resp) = tokio_tungstenite::connect_async(SERVER_URL).await?;
    let (sink, mut stream) = ws.split();
    let sink = Arc::new(Mutex::new(sink));

    let live_ok_sent = Arc::new(AtomicBool::new(false));

    // Join room (auth).
    {
        let buvid = mk_buvid();
        let json = serde_json::json!({
            "uid": uid,
            "roomid": rid,
            "protover": 2,
            "buvid": buvid,
            "platform": "web",
            "type": 2,
            "key": token,
        });
        let pkt = encode_packet(json.to_string().as_bytes(), 7, 1);
        sink.lock().await.send(Message::Binary(pkt.into())).await?;
    }

    // Heartbeat loop.
    {
        let sink = sink.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(HEARTBEAT_MS));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        let pkt = encode_packet(&[], 2, 1);
                        if sink
                            .lock()
                            .await
                            .send(Message::Binary(pkt.into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
    }

    // Read loop.
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            msg = stream.next() => {
                match msg {
                    None => break,
                    Some(Err(e)) => {
                        let _ = tx.send(DanmakuEvent::new(
                            Site::BiliLive,
                            room_id.clone(),
                            DanmakuMethod::LiveDMServer,
                            "error",
                            None,
                        ));
                        return Err(e.into());
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        if let Err(e) = handle_frame(&room_id, &tx, &bin, 0, emoticons.clone(), live_ok_sent.clone()) {
                            let _ = tx.send(DanmakuEvent::new(
                                Site::BiliLive,
                                room_id.clone(),
                                DanmakuMethod::LiveDMServer,
                                "error",
                                None,
                            ));
                            return Err(e);
                        }
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    Ok(())
}

fn mk_buvid() -> String {
    // IINA+ uses UUID + random digits + "infoc". We avoid adding a uuid dep here.
    let a = fastrand::u128(..);
    let b = fastrand::u64(..);
    let d = fastrand::u32(10_000..90_000);
    format!("{a:032x}{b:016x}{d}infoc")
}

fn ensure_https(url: &str) -> String {
    let s = url.trim();
    if s.starts_with("https://") {
        return s.to_string();
    }
    if s.starts_with("http://") {
        return format!("https://{}", &s["http://".len()..]);
    }
    if s.starts_with("//") {
        return format!("https:{s}");
    }
    s.to_string()
}

fn scaled_width(width: i64) -> Option<u32> {
    if width <= 0 {
        return None;
    }
    let w = (width as u64).min(200) as u32;
    Some(w / 2)
}

fn encode_packet(body: &[u8], operation: u32, protover: u16) -> Vec<u8> {
    let packet_len = (16 + body.len()) as u32;
    let mut out = Vec::with_capacity(packet_len as usize);
    out.extend_from_slice(&packet_len.to_be_bytes());
    out.extend_from_slice(&(16u16).to_be_bytes());
    out.extend_from_slice(&protover.to_be_bytes());
    out.extend_from_slice(&operation.to_be_bytes());
    out.extend_from_slice(&(1u32).to_be_bytes()); // seq
    out.extend_from_slice(body);
    out
}

#[derive(Debug, Clone)]
struct Packet {
    protover: u16,
    operation: u32,
    body: Vec<u8>,
}

fn parse_packets(mut data: &[u8]) -> Result<Vec<Packet>, DanmakuError> {
    let mut out = Vec::new();
    while data.len() >= 16 {
        let packet_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if packet_len < 16 || packet_len > data.len() {
            break;
        }
        let header_len = u16::from_be_bytes([data[4], data[5]]) as usize;
        let protover = u16::from_be_bytes([data[6], data[7]]);
        let operation = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        if header_len > packet_len {
            return Err(DanmakuError::Parse(
                "invalid bilibili header_len".to_string(),
            ));
        }

        let body = data[header_len..packet_len].to_vec();
        out.push(Packet {
            protover,
            operation,
            body,
        });
        data = &data[packet_len..];
    }
    Ok(out)
}

fn inflate_any(data: &[u8]) -> Result<Vec<u8>, DanmakuError> {
    // Primary: zlib-wrapped deflate.
    let mut out = Vec::new();
    let mut z = ZlibDecoder::new(data);
    if z.read_to_end(&mut out).is_ok() {
        return Ok(out);
    }

    // Fallback 1: raw deflate (strip 2 bytes best-effort).
    if data.len() > 2 {
        out.clear();
        let mut d = DeflateDecoder::new(&data[2..]);
        if d.read_to_end(&mut out).is_ok() {
            return Ok(out);
        }
    }

    // Fallback 2: gzip (IINA+ uses gunzip in its legacy path)
    out.clear();
    let mut g = GzDecoder::new(data);
    g.read_to_end(&mut out)
        .map_err(|e| DanmakuError::Codec(format!("inflate failed: {e}")))?;
    Ok(out)
}

fn handle_frame(
    room_id: &str,
    tx: &DanmakuEventTx,
    data: &[u8],
    depth: u32,
    emoticons: Arc<HashMap<String, EmoticonMeta>>,
    live_ok_sent: Arc<AtomicBool>,
) -> Result<(), DanmakuError> {
    if depth > 4 {
        return Err(DanmakuError::Parse(
            "bilibili packet nesting too deep".to_string(),
        ));
    }

    let packets = parse_packets(data)?;
    for p in packets {
        match p.operation {
            8 => {
                // Auth response.
                if !live_ok_sent.swap(true, Ordering::Relaxed) {
                    let _ = tx.send(DanmakuEvent::new(
                        Site::BiliLive,
                        room_id.to_string(),
                        DanmakuMethod::LiveDMServer,
                        "",
                        None,
                    ));
                }
            }
            5 => match p.protover {
                0 => {
                    let text = String::from_utf8_lossy(&p.body).to_string();
                    for part in text.split('\0').filter(|s| s.trim_start().starts_with('{')) {
                        let v: serde_json::Value = match serde_json::from_str(part) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        handle_json(room_id, tx, &v, emoticons.clone());
                    }
                }
                2 => {
                    let inflated = inflate_any(&p.body)?;
                    handle_frame(
                        room_id,
                        tx,
                        &inflated,
                        depth + 1,
                        emoticons.clone(),
                        live_ok_sent.clone(),
                    )?;
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}

fn handle_json(
    room_id: &str,
    tx: &DanmakuEventTx,
    v: &serde_json::Value,
    emoticons: Arc<HashMap<String, EmoticonMeta>>,
) {
    let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
    // Prefer legacy "DANMU_MSG" parsing because it contains the username in `info`.
    if cmd.starts_with("DANMU_MSG") {
        let user = v
            .pointer("/info/2/1")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();

        let Some(info0) = v.pointer("/info/0").and_then(|x| x.as_array()) else {
            return;
        };

        // info[0][13]: {"emoticon_unique": "...", "url": "...", "width":..., "height":...}
        if let Some(obj) = info0.get(13).and_then(|x| x.as_object()) {
            let unique = obj.get("emoticon_unique").and_then(|x| x.as_str());
            let url = obj.get("url").and_then(|x| x.as_str());
            if let (Some(_unique), Some(url)) = (unique, url) {
                let width = obj.get("width").and_then(|x| x.as_i64()).unwrap_or(180);
                let c = DanmakuComment {
                    text: "".to_string(),
                    image_url: Some(ensure_https(url)),
                    image_width: scaled_width(width),
                };
                let mut ev = DanmakuEvent::new(
                    Site::BiliLive,
                    room_id.to_string(),
                    DanmakuMethod::SendDM,
                    "",
                    Some(vec![c]),
                );
                ev.user = user;
                let _ = tx.send(ev);
                return;
            }
        }

        // info[0][15].extra: JSON string; may contain emots / emoticon_unique
        if let Some(extra_str) = info0
            .get(15)
            .and_then(|x| x.as_object())
            .and_then(|o| o.get("extra"))
            .and_then(|x| x.as_str())
        {
            if let Ok(extra_v) = serde_json::from_str::<serde_json::Value>(extra_str) {
                let content = extra_v
                    .get("content")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();

                if let Some(emots) = extra_v.get("emots").and_then(|x| x.as_object()) {
                    if let Some((_k, v0)) = emots.iter().next() {
                        if let Some(url) = v0.get("url").and_then(|x| x.as_str()) {
                            let width = v0.get("width").and_then(|x| x.as_i64()).unwrap_or(180);
                            let c = DanmakuComment {
                                text: "".to_string(),
                                image_url: Some(ensure_https(url)),
                                image_width: scaled_width(width),
                            };
                            let mut ev = DanmakuEvent::new(
                                Site::BiliLive,
                                room_id.to_string(),
                                DanmakuMethod::SendDM,
                                "",
                                Some(vec![c]),
                            );
                            ev.user = user.clone();
                            let _ = tx.send(ev);
                            return;
                        }
                    }
                }

                if let Some(unique) = extra_v.get("emoticon_unique").and_then(|x| x.as_str()) {
                    if let Some(meta) = emoticons.get(unique) {
                        let c = DanmakuComment {
                            text: "".to_string(),
                            image_url: Some(meta.url.clone()),
                            image_width: scaled_width(meta.width as i64),
                        };
                        let mut ev = DanmakuEvent::new(
                            Site::BiliLive,
                            room_id.to_string(),
                            DanmakuMethod::SendDM,
                            "",
                            Some(vec![c]),
                        );
                        ev.user = user.clone();
                        let _ = tx.send(ev);
                        return;
                    }
                }

                if !content.is_empty() {
                    let c = DanmakuComment::text(content);
                    let mut ev = DanmakuEvent::new(
                        Site::BiliLive,
                        room_id.to_string(),
                        DanmakuMethod::SendDM,
                        "",
                        Some(vec![c]),
                    );
                    ev.user = user.clone();
                    let _ = tx.send(ev);
                    return;
                }
            }
        }

        if let Some(msg) = v.pointer("/info/1").and_then(|x| x.as_str()) {
            if !msg.is_empty() {
                let c = DanmakuComment::text(msg);
                let mut ev = DanmakuEvent::new(
                    Site::BiliLive,
                    room_id.to_string(),
                    DanmakuMethod::SendDM,
                    "",
                    Some(vec![c]),
                );
                ev.user = user;
                let _ = tx.send(ev);
            }
        }
        return;
    }

    // Fallback: dm_v2 parsing (best-effort, may not include username).
    if let Some(dm_v2) = v
        .get("dm_v2")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
    {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dm_v2) {
            if let Ok(dm) = DmV2::decode(bytes.as_slice()) {
                // Skip survive scene.
                if BizScene::try_from(dm.biz_scene).ok() == Some(BizScene::Survive) {
                    return;
                }

                if DmType::try_from(dm.dm_type).ok() == Some(DmType::Emoticon) {
                    if let Some(e) = dm.emoticons.iter().find_map(|x| x.value.as_ref()) {
                        let url = ensure_https(&e.url);
                        let c = DanmakuComment {
                            text: "".to_string(),
                            image_url: Some(url),
                            image_width: scaled_width(e.width),
                        };
                        let _ = tx.send(DanmakuEvent::new(
                            Site::BiliLive,
                            room_id.to_string(),
                            DanmakuMethod::SendDM,
                            "",
                            Some(vec![c]),
                        ));
                        return;
                    }
                }

                if !dm.text.is_empty() {
                    let c = DanmakuComment::text(dm.text);
                    let _ = tx.send(DanmakuEvent::new(
                        Site::BiliLive,
                        room_id.to_string(),
                        DanmakuMethod::SendDM,
                        "",
                        Some(vec![c]),
                    ));
                }
            }
        }
    }
}

async fn fetch_room_rid(http: &reqwest::Client, room_id: u64) -> Result<u64, DanmakuError> {
    let url = format!("https://api.live.bilibili.com/room/v1/Room/get_info?room_id={room_id}");
    let v: serde_json::Value = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    v.pointer("/data/room_id")
        .and_then(|x| {
            x.as_u64()
                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
        })
        .ok_or_else(|| DanmakuError::Parse("missing data.room_id".to_string()))
}

async fn fetch_nav_wbi(http: &reqwest::Client) -> Result<(u64, String, String), DanmakuError> {
    let v: serde_json::Value = http
        .get("https://api.bilibili.com/x/web-interface/nav")
        .header(reqwest::header::REFERER, "https://www.bilibili.com/")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let uid = v
        .pointer("/data/mid")
        .and_then(|x| {
            x.as_u64()
                .or_else(|| x.as_i64().and_then(|n| u64::try_from(n).ok()))
        })
        .unwrap_or(0);

    let img_url = v
        .pointer("/data/wbi_img/img_url")
        .and_then(|x| x.as_str())
        .ok_or_else(|| DanmakuError::Parse("missing data.wbi_img.img_url".to_string()))?;
    let sub_url = v
        .pointer("/data/wbi_img/sub_url")
        .and_then(|x| x.as_str())
        .ok_or_else(|| DanmakuError::Parse("missing data.wbi_img.sub_url".to_string()))?;

    let img_key = extract_wbi_key(img_url)
        .ok_or_else(|| DanmakuError::Parse("failed to extract wbi img_key".to_string()))?;
    let sub_key = extract_wbi_key(sub_url)
        .ok_or_else(|| DanmakuError::Parse("failed to extract wbi sub_key".to_string()))?;

    Ok((uid, img_key, sub_key))
}

fn extract_wbi_key(url: &str) -> Option<String> {
    // Extract last path component without extension.
    if let Ok(u) = url::Url::parse(url) {
        if let Some(seg) = u.path_segments().and_then(|s| s.last()) {
            return Some(seg.split('.').next().unwrap_or(seg).to_string());
        }
    }
    let last = url.split('/').last()?;
    Some(last.split('.').next().unwrap_or(last).to_string())
}

async fn fetch_token(
    http: &reqwest::Client,
    rid: u64,
    img_key: &str,
    sub_key: &str,
) -> Result<String, DanmakuError> {
    let base_param = format!("id={rid}&type=0&web_location=444.8");
    let signed = wbi_sign(&base_param, img_key, sub_key);
    let url =
        format!("https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?{signed}");

    let v: serde_json::Value = http
        .get(url)
        .header(reqwest::header::REFERER, "https://www.bilibili.com/")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    v.pointer("/data/token")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DanmakuError::Parse("missing data.token".to_string()))
}

fn wbi_sign(param: &str, img_key: &str, sub_key: &str) -> String {
    // Ported from IINA+ BilbiliShare.swift:biliWbiSign
    const MIXIN_KEY_ENC_TAB: [usize; 64] = [
        46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19,
        29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4,
        22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
    ];

    fn get_mixin_key(orig: &str) -> String {
        let chars: Vec<char> = orig.chars().collect();
        MIXIN_KEY_ENC_TAB
            .iter()
            .filter_map(|&i| chars.get(i).copied())
            .take(32)
            .collect()
    }

    fn md5_hex(s: &str) -> String {
        format!("{:x}", md5::compute(s.as_bytes()))
    }

    // Parse "a=b&c=d" into map.
    let mut params: HashMap<String, String> = HashMap::new();
    for pair in param.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            params.insert(k.to_string(), v.to_string());
        }
    }

    let mixin_key = get_mixin_key(&format!("{img_key}{sub_key}"));
    let wts = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()) as i64;
    params.insert("wts".to_string(), wts.to_string());

    let mut items = params.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| a.0.cmp(&b.0));

    let query = items
        .iter()
        .map(|(k, v)| {
            let filtered: String = v.chars().filter(|c| !"!'()*".contains(*c)).collect();
            format!("{k}={filtered}")
        })
        .collect::<Vec<_>>()
        .join("&");

    let w_rid = md5_hex(&(query.clone() + &mixin_key));
    format!("{query}&w_rid={w_rid}")
}

async fn fetch_emoticons(
    http: &reqwest::Client,
    rid: u64,
) -> Result<HashMap<String, EmoticonMeta>, DanmakuError> {
    let url = format!(
        "https://api.live.bilibili.com/xlive/web-ucenter/v2/emoticon/GetEmoticons?platform=pc&room_id={rid}"
    );
    let v: serde_json::Value = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut map = HashMap::<String, EmoticonMeta>::new();
    let Some(pkgs) = v.pointer("/data/data").and_then(|x| x.as_array()) else {
        return Ok(map);
    };

    for pkg in pkgs {
        let pkg_name = pkg.get("pkg_name").and_then(|x| x.as_str()).unwrap_or("");
        let Some(emots) = pkg.get("emoticons").and_then(|x| x.as_array()) else {
            continue;
        };

        for e in emots {
            let unique = e
                .get("emoticon_unique")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if unique.is_empty() {
                continue;
            }
            let url = e.get("url").and_then(|x| x.as_str()).unwrap_or("");
            if url.is_empty() {
                continue;
            }
            let width = e.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let height = e.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as u32;

            let (width, height) = if pkg_name == "emoji" {
                (75, 75)
            } else {
                (width, height)
            };

            map.insert(
                unique.clone(),
                EmoticonMeta {
                    unique,
                    url: ensure_https(url),
                    width,
                    height,
                },
            );
        }
    }

    Ok(map)
}
