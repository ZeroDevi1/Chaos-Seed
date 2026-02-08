use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use regex::Regex;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use crate::danmaku::model::{
    ConnectInfo, ConnectOptions, DanmakuComment, DanmakuError, DanmakuEvent, DanmakuEventTx,
    DanmakuMethod, ResolvedTarget, Site,
};

use super::huya_jce as jce;

const SERVER_URL: &str = "wss://cdnws.api.huya.com";
const HEARTBEAT_MS: u64 = 30_000;
const HEARTBEAT_BYTES: &[u8] = &[0x00, 0x14, 0x1d, 0x00, 0x0c, 0x2c, 0x36, 0x00, 0x4c];

pub async fn resolve(
    http: &reqwest::Client,
    room_id: &str,
) -> Result<ResolvedTarget, DanmakuError> {
    let room_id = room_id.trim();
    if room_id.is_empty() {
        return Err(DanmakuError::InvalidInput("empty room id".to_string()));
    }

    let info = fetch_huya_room_info(http, room_id).await?;
    Ok(ResolvedTarget {
        site: Site::Huya,
        room_id: room_id.to_string(),
        connect: ConnectInfo::Huya {
            room_id: room_id.to_string(),
            yyuid: info.yyuid,
            uid: info.uid,
        },
    })
}

pub async fn run(
    target: ResolvedTarget,
    opt: ConnectOptions,
    tx: DanmakuEventTx,
    cancel: CancellationToken,
) -> Result<(), DanmakuError> {
    let (room_id, yyuid, uid) = match &target.connect {
        ConnectInfo::Huya {
            room_id,
            yyuid,
            uid,
        } => (room_id.clone(), *yyuid, *uid),
        _ => {
            return Err(DanmakuError::InvalidInput(
                "huya connector expects ConnectInfo::Huya".to_string(),
            ));
        }
    };

    let (ws, _resp) = tokio_tungstenite::connect_async(SERVER_URL).await?;
    let (sink, mut stream) = ws.split();
    let sink = Arc::new(Mutex::new(sink));

    // Join room.
    {
        let join_cmd = encode_join_cmd(yyuid, uid);
        sink.lock()
            .await
            .send(Message::Binary(join_cmd.into()))
            .await?;
        let _ = tx.send(DanmakuEvent::new(
            Site::Huya,
            room_id.clone(),
            DanmakuMethod::LiveDMServer,
            "",
            None,
        ));
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
                        if sink
                            .lock()
                            .await
                            .send(Message::Binary(HEARTBEAT_BYTES.to_vec().into()))
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

    let any_msg_seen = Arc::new(AtomicBool::new(false));

    // Read loop.
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            msg = stream.next() => {
                match msg {
                    None => break,
                    Some(Err(e)) => {
                        let _ = tx.send(DanmakuEvent::new(
                            Site::Huya,
                            room_id.clone(),
                            DanmakuMethod::LiveDMServer,
                            "error",
                            None,
                        ));
                        return Err(e.into());
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        if !any_msg_seen.swap(true, Ordering::Relaxed) {
                            // Some environments drop the initial ok event, so we resend once we see traffic.
                            let _ = tx.send(DanmakuEvent::new(
                                Site::Huya,
                                room_id.clone(),
                                DanmakuMethod::LiveDMServer,
                                "",
                                None,
                            ));
                        }
                        if let Err(e) = handle_binary(&room_id, &opt, &tx, &bin).await {
                            let _ = tx.send(DanmakuEvent::new(
                                Site::Huya,
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

fn encode_ws_cmd(cmd_type: i32, data: &[u8]) -> Vec<u8> {
    let mut e = jce::Encoder::new();
    e.write_i32(0, cmd_type);
    e.write_bytes(1, data);
    e.into_bytes()
}

fn encode_join_cmd(yyuid: i64, uid: i64) -> Vec<u8> {
    let mut inner = jce::Encoder::new();
    inner.write_i64(0, yyuid);
    inner.write_bool(1, true);
    inner.write_string(2, "");
    inner.write_string(3, "");
    inner.write_i64(4, uid);
    inner.write_i64(5, uid);
    inner.write_i32(6, 0);
    inner.write_i32(7, 0);
    let inner = inner.into_bytes();
    encode_ws_cmd(1, &inner)
}

async fn handle_binary(
    room_id: &str,
    opt: &ConnectOptions,
    tx: &DanmakuEventTx,
    bin: &[u8],
) -> Result<(), DanmakuError> {
    let msg_type = jce::get_i32(bin, 0)?.unwrap_or(-1);
    let data = jce::get_bytes(bin, 1)?.unwrap_or_default();

    if msg_type == 7 {
        handle_push(room_id, opt, tx, &data).await?;
    }
    Ok(())
}

async fn handle_push(
    room_id: &str,
    opt: &ConnectOptions,
    tx: &DanmakuEventTx,
    data: &[u8],
) -> Result<(), DanmakuError> {
    let uri = jce::get_i64(data, 1)?.unwrap_or(0);
    let msg = jce::get_bytes(data, 2)?.unwrap_or_default();

    if uri == 1400 {
        // MessageNotice: content(tag=3), user_info(tag=0 struct) with nick(tag=2), icon(tag=4)
        let content = jce::get_string(&msg, 3)?.unwrap_or_default();
        if content.is_empty() {
            return Ok(());
        }
        if opt.blocklist.iter().any(|b| content.contains(b)) {
            return Ok(());
        }

        let user_info = jce::get_struct_bytes(&msg, 0)?;
        let nick = user_info
            .as_ref()
            .and_then(|u| jce::get_string(u, 2).ok().flatten())
            .unwrap_or_default();

        // For now we only emit comments; user/avatar are UI concerns for the next phase.
        let dm = DanmakuComment::text(content);
        let mut ev = DanmakuEvent::new(
            Site::Huya,
            room_id.to_string(),
            DanmakuMethod::SendDM,
            "",
            Some(vec![dm]),
        );
        ev.user = nick.clone();
        let _ = tx.send(ev);

        // Keep compiler happy if we decide to extend comment model later.
        let _ = nick;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct HuyaRoomInfo {
    yyuid: i64,
    uid: i64,
}

async fn fetch_huya_room_info(
    http: &reqwest::Client,
    room_id: &str,
) -> Result<HuyaRoomInfo, DanmakuError> {
    let url = format!("https://m.huya.com/{room_id}");
    let text = http
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (iPhone; CPU iPhone OS 13_2_3 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.0.3 Mobile/15E148 Safari/604.1",
        )
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let json_str = extract_huya_global_init_json(&text).ok_or_else(|| {
        DanmakuError::Parse("failed to extract Huya HNF_GLOBAL_INIT json".to_string())
    })?;
    let v: serde_json::Value = serde_json::from_str(&json_str)?;

    let yyuid = v
        .pointer("/roomInfo/tLiveInfo/lYyid")
        .and_then(value_to_i64)
        .ok_or_else(|| DanmakuError::Parse("missing roomInfo.tLiveInfo.lYyid".to_string()))?;
    let uid = v
        .pointer("/roomInfo/tLiveInfo/lUid")
        .and_then(value_to_i64)
        .ok_or_else(|| DanmakuError::Parse("missing roomInfo.tLiveInfo.lUid".to_string()))?;

    Ok(HuyaRoomInfo { yyuid, uid })
}

fn value_to_i64(v: &serde_json::Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    v.as_str().and_then(|s| s.parse::<i64>().ok())
}

fn extract_huya_global_init_json(html: &str) -> Option<String> {
    // First try a regex anchored to </script>.
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?s)window\.HNF_GLOBAL_INIT\s*=\s*\{(.*)\}\s*;?\s*</script>"#)
            .expect("huya regex")
    });
    if let Some(caps) = re.captures(html) {
        let inner = caps.get(1)?.as_str();
        return Some(format!("{{{inner}}}"));
    }

    // Fallback: locate the assignment and do brace-matching (string-aware).
    let needle = "window.HNF_GLOBAL_INIT";
    let start = html.find(needle)?;
    let after = &html[start..];
    let brace_pos = after.find('{')?;
    let abs_brace = start + brace_pos;
    let bytes = html.as_bytes();

    let mut i = abs_brace;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    // inclusive range
                    return Some(html[abs_brace..=i].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
