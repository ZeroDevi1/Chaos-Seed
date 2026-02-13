use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, ORIGIN, USER_AGENT};
use tokio_util::sync::CancellationToken;

use crate::danmaku::model::{
    ConnectInfo, ConnectOptions, DanmakuComment, DanmakuError, DanmakuEvent, DanmakuEventTx,
    DanmakuMethod, ResolvedTarget, Site,
};

const SERVER_URL: &str = "wss://danmuproxy.douyu.com:8506/";
const HEARTBEAT_MS: u64 = 30_000;

pub async fn resolve(
    http: &reqwest::Client,
    room_id: &str,
) -> Result<ResolvedTarget, DanmakuError> {
    let rid = fetch_room_id(http, room_id).await?;
    Ok(ResolvedTarget {
        site: Site::Douyu,
        room_id: room_id.trim().to_string(),
        connect: ConnectInfo::Douyu { room_id: rid },
    })
}

pub async fn run(
    target: ResolvedTarget,
    opt: ConnectOptions,
    tx: DanmakuEventTx,
    cancel: CancellationToken,
    _http: reqwest::Client,
) -> Result<(), DanmakuError> {
    let room_id = target.room_id.clone();
    let rid = match target.connect {
        ConnectInfo::Douyu { room_id } => room_id,
        _ => {
            return Err(DanmakuError::InvalidInput(
                "douyu connector expects ConnectInfo::Douyu".to_string(),
            ));
        }
    };

    let mut req = SERVER_URL.into_client_request()?;
    // Douyu's danmu gateway can be picky about browser-like headers.
    req.headers_mut()
        .insert(ORIGIN, HeaderValue::from_static("https://www.douyu.com"));
    req.headers_mut().insert(
        USER_AGENT,
        HeaderValue::from_static("chaos-seed/0.1 (douyu-danmaku)"),
    );

    // Use tokio-tungstenite's native-tls backend (vendored OpenSSL) for maximum compatibility
    // with Douyu's TLS stack.
    let (ws, _resp) = tokio_tungstenite::connect_async(req).await?;
    let (mut sink, mut stream) = ws.split();

    // Join room.
    let login = encode_packet(&format!("type@=loginreq/roomid@={rid}/"));
    let join = encode_packet(&format!("type@=joingroup/rid@={rid}/gid@=-9999/"));
    sink.send(Message::Binary(login.into())).await?;
    sink.send(Message::Binary(join.into())).await?;

    // Heartbeat loop.
    {
        let mut sink = sink;
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(HEARTBEAT_MS));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        let hb = encode_packet("type@=mrkl/");
                        if sink.send(Message::Binary(hb.into())).await.is_err() {
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
                            Site::Douyu,
                            room_id.clone(),
                            DanmakuMethod::LiveDMServer,
                            "error",
                            None,
                        ));
                        return Err(e.into());
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        for text in decode_packets(&bin) {
                            handle_text(&room_id, &opt, &tx, &text);
                        }
                    }
                    Some(Ok(Message::Text(txt))) => {
                        handle_text(&room_id, &opt, &tx, &txt);
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    Ok(())
}

fn encode_packet(msg: &str) -> Vec<u8> {
    // Douyu framing (matches dart_simple_live):
    // full_len = len2(4) + type(2) + enc(1) + reserved(1) + body(N) + nul(1) = N + 9
    // Packet bytes = len(4) + full_len.
    let body = msg.as_bytes();
    let full_len = (body.len() + 9) as u32;

    let mut out = Vec::with_capacity((full_len + 4) as usize);
    out.extend_from_slice(&full_len.to_le_bytes());
    out.extend_from_slice(&full_len.to_le_bytes());
    out.extend_from_slice(&(689u16).to_le_bytes()); // clientSendToServer
    out.push(0); // encrypted
    out.push(0); // reserved
    out.extend_from_slice(body);
    out.push(0); // NUL terminator
    out
}

fn decode_packets(buf: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + 12 <= buf.len() {
        let full_len =
            u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]) as usize;
        // total bytes = 4 (len field) + full_len
        let total = full_len.saturating_add(4);
        if full_len < 9 || off + total > buf.len() {
            break;
        }
        // body_len excludes the trailing NUL: body_len = full_len - 9
        let body_len = full_len.saturating_sub(9);
        let start = off + 12;
        let end = (start + body_len).min(buf.len());
        let text = String::from_utf8_lossy(&buf[start..end]).to_string();
        out.push(text);
        off += total;
    }
    out
}

fn handle_text(room_id: &str, opt: &ConnectOptions, tx: &DanmakuEventTx, text: &str) {
    if text.is_empty() {
        return;
    }

    // Fast-path checks used by IINA+.
    if text.starts_with("type@=chatmsg") {
        if !text.contains("dms@=") {
            return;
        }
        let map = parse_kv(text);
        let Some(txt) = map.get("txt") else {
            return;
        };
        if opt.blocklist.iter().any(|b| txt.contains(b)) {
            return;
        }
        let dm = DanmakuComment::text(txt.clone());
        let mut ev = DanmakuEvent::new(
            Site::Douyu,
            room_id.to_string(),
            DanmakuMethod::SendDM,
            "",
            Some(vec![dm]),
        );
        ev.user = map.get("nn").cloned().unwrap_or_default();
        let _ = tx.send(ev);
        return;
    }

    if text.starts_with("type@=loginres") {
        let _ = tx.send(DanmakuEvent::new(
            Site::Douyu,
            room_id.to_string(),
            DanmakuMethod::LiveDMServer,
            "",
            None,
        ));
        return;
    }

    if text.starts_with("type@=error") {
        let _ = tx.send(DanmakuEvent::new(
            Site::Douyu,
            room_id.to_string(),
            DanmakuMethod::LiveDMServer,
            "error",
            None,
        ));
    }
}

fn parse_kv(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for part in text.split('/') {
        if let Some((k, v)) = part.split_once("@=") {
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn douyu_packet_len_matches_total_bytes() {
        let msg = "type@=loginreq/roomid@=1/";
        let pkt = encode_packet(msg);
        assert!(pkt.len() >= 16);
        let full_len = u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]) as usize;
        assert_eq!(pkt.len(), full_len + 4);
        // The last byte must be NUL.
        assert_eq!(pkt[pkt.len() - 1], 0);
    }

    #[test]
    fn douyu_packet_roundtrip_decode() {
        let msg1 = "type@=loginreq/roomid@=1/";
        let msg2 = "type@=joingroup/rid@=1/gid@=-9999/";
        let mut buf = Vec::new();
        buf.extend_from_slice(&encode_packet(msg1));
        buf.extend_from_slice(&encode_packet(msg2));
        let out = decode_packets(&buf);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], msg1);
        assert_eq!(out[1], msg2);
    }
}

async fn fetch_room_id(http: &reqwest::Client, room_id: &str) -> Result<String, DanmakuError> {
    let rid_raw = room_id.trim();
    if rid_raw.is_empty() {
        return Err(DanmakuError::InvalidInput("empty room id".to_string()));
    }

    let url = format!("https://www.douyu.com/{rid_raw}");
    let text = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let json_text = extract_room_info_json(&text)
        .ok_or_else(|| DanmakuError::Parse("failed to extract douyu roomInfo json".to_string()))?;

    let json_text = json_text.replace("\\\"", "\"").replace("\\\"", "\"");
    let v: serde_json::Value = serde_json::from_str(&json_text)?;

    v.pointer("/room/room_id")
        .and_then(|x| {
            x.as_i64()
                .map(|n| n.to_string())
                .or_else(|| x.as_str().map(|s| s.to_string()))
        })
        .ok_or_else(|| DanmakuError::Parse("missing room.room_id".to_string()))
}

fn extract_room_info_json(input: &str) -> Option<String> {
    // Ported from IINA+: locate \"roomInfo\" then brace-match the subsequent JSON object.
    let idx = input.find("\\\"roomInfo\\\"")?;
    let suffix = &input[idx..];
    let open_rel = suffix.find('{')?;
    let open = idx + open_rel;

    let bytes = input.as_bytes();
    let mut i = open;
    let mut depth: i32 = 0;
    let mut end = None;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let end = end?;
    Some(input[open..=end].to_string())
}
