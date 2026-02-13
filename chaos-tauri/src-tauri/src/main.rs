#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use bytes::Bytes;
use chaos_core::danmaku::client::DanmakuClient;
use chaos_core::danmaku::model::{ConnectOptions, DanmakuSession, Site};
use chaos_core::livestream::client::LivestreamClient;
use chaos_core::livestream::model::ResolveOptions;
use chaos_core::lyrics;
use chaos_core::now_playing;
use chaos_core::subtitle;
use chaos_core::subtitle::models::ThunderSubtitleItem;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex as AsyncMutex, Notify};

use lyrics_app::{LyricsAppSettings, LyricsAppSettingsPatch, LyricsAppState};

mod danmaku_ui;
mod livestream_ui;
mod lyrics_app;

const HOMEPAGE: &str = "https://github.com/ZeroDevi1/Chaos-Seed";

struct ActiveDanmaku {
    input: String,
    site: String,
    room_id: String,
    session: DanmakuSession,
    reader_task: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
struct DanmakuState {
    active: Mutex<Option<ActiveDanmaku>>,
    msg_subscribers: Mutex<HashSet<String>>,
    // Tracks auxiliary renderer windows that are currently open (Chat/Overlay).
    // We use this instead of querying `get_webview_window()` on every message to avoid
    // platform-specific window handle quirks and to keep the suppression logic deterministic.
    aux_windows: Mutex<HashSet<String>>,
}

#[derive(Default)]
struct LyricsWindowState {
    current: Mutex<Option<lyrics::model::LyricsSearchResult>>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct StreamReadReply {
    eof: bool,
    #[serde(rename = "dataB64")]
    data_b64: String,
}

struct StreamSession {
    url: String,
    notify: Notify,
    state: AsyncMutex<StreamSessionState>,
    reader_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
}

struct StreamSessionState {
    queue: VecDeque<Vec<u8>>,
    head_off: usize,
    buffered: usize,
    done: bool,
    error: Option<String>,
}

impl StreamSessionState {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            head_off: 0,
            buffered: 0,
            done: false,
            error: None,
        }
    }
}

#[derive(Default)]
struct StreamProxyState {
    sessions: AsyncMutex<HashMap<String, Arc<StreamSession>>>,
}

static STREAM_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, serde::Serialize)]
struct WindowStatePayload {
    label: String,
    open: bool,
}

async fn stop_active(active: ActiveDanmaku) {
    // Stop the connector tasks first so the event channel closes naturally.
    active.session.stop().await;
    active.reader_task.abort();
    let _ = active.reader_task.await;
}

fn emit_to_known_windows<S: serde::Serialize + Clone>(
    app: &AppHandle,
    event: &str,
    payload: S,
) -> Result<(), tauri::Error> {
    // Broadcast once. On some setups, `emit_to` can behave unexpectedly with multi-webview apps,
    // causing duplicate deliveries. We only have a few windows and want consistent behavior.
    let _ = app.emit(event, payload);
    Ok(())
}

fn parse_site_str(site: &str) -> Result<Site, String> {
    let s = site.trim().to_ascii_lowercase();
    match s.as_str() {
        "bili_live" | "bili" | "bilibili" | "bl" => Ok(Site::BiliLive),
        "douyu" | "dy" => Ok(Site::Douyu),
        "huya" | "hy" => Ok(Site::Huya),
        _ => Err(format!("unsupported site: {site}")),
    }
}

#[tauri::command]
async fn stream_open(
    state: State<'_, StreamProxyState>,
    url: String,
    referer: Option<String>,
    user_agent: Option<String>,
) -> Result<String, String> {
    let u = url::Url::parse(&url).map_err(|e| e.to_string())?;
    match u.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported url scheme: {other}")),
    }
    if is_local_or_private_host(&u) {
        return Err("blocked host".to_string());
    }

    let id = STREAM_ID.fetch_add(1, Ordering::Relaxed);
    let handle = format!("s{id}");

    let session = Arc::new(StreamSession {
        url: url.clone(),
        notify: Notify::new(),
        state: AsyncMutex::new(StreamSessionState::new()),
        reader_task: AsyncMutex::new(None),
    });

    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(handle.clone(), session.clone());
    }

    // Spawn a reader task that continuously fetches the remote stream and buffers it.
    // This avoids webview CORS/origin restrictions in packaged apps.
    let url2 = url.clone();
    let referer2 = referer.clone();
    let ua2 = user_agent.clone();
    let sess = session.clone();
    let task = tokio::spawn(async move {
        const MAX_BUFFER_BYTES: usize = 8 * 1024 * 1024;
        const BACKPRESSURE_SLEEP_MS: u64 = 15;

        let client = reqwest::Client::builder()
            .tcp_keepalive(Some(Duration::from_secs(15)))
            .build();
        let client = match client {
            Ok(c) => c,
            Err(e) => {
                let mut st = sess.state.lock().await;
                st.error = Some(format!("reqwest client error: {e}"));
                st.done = true;
                sess.notify.notify_waiters();
                return;
            }
        };

        let mut req = client.get(url2);
        if let Some(ua) = ua2.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            req = req.header(reqwest::header::USER_AGENT, ua);
        }
        if let Some(r) = referer2
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            req = req.header(reqwest::header::REFERER, r);
            // Some CDNs check Origin; align it with referer best-effort.
            if let Ok(ru) = url::Url::parse(r) {
                let origin = ru.origin().ascii_serialization();
                if !origin.trim().is_empty() && origin != "null" {
                    req = req.header(reqwest::header::HeaderName::from_static("origin"), origin);
                }
            }
        }

        let resp = match req.send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => r,
            Err(e) => {
                let mut st = sess.state.lock().await;
                st.error = Some(format!("http error: {e}"));
                st.done = true;
                sess.notify.notify_waiters();
                return;
            }
        };

        let mut stream = resp.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk: Bytes = match item {
                Ok(b) => b,
                Err(e) => {
                    let mut st = sess.state.lock().await;
                    st.error = Some(format!("read error: {e}"));
                    st.done = true;
                    sess.notify.notify_waiters();
                    return;
                }
            };

            // Backpressure: keep a bounded buffer.
            loop {
                let st = sess.state.lock().await;
                let should_wait = st.buffered >= MAX_BUFFER_BYTES;
                if !should_wait {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(BACKPRESSURE_SLEEP_MS)).await;
            }

            let mut st = sess.state.lock().await;
            let v = chunk.to_vec();
            st.buffered = st.buffered.saturating_add(v.len());
            st.queue.push_back(v);
            sess.notify.notify_waiters();
        }

        let mut st = sess.state.lock().await;
        st.done = true;
        sess.notify.notify_waiters();
    });

    *session.reader_task.lock().await = Some(task);

    Ok(handle)
}

#[tauri::command]
async fn stream_read(
    state: State<'_, StreamProxyState>,
    handle: String,
    max_len: Option<usize>,
) -> Result<StreamReadReply, String> {
    let max_len = max_len.unwrap_or(64 * 1024).clamp(1, 1024 * 1024);
    let sess = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(handle.trim())
            .cloned()
            .ok_or_else(|| "stream not found".to_string())?
    };

    loop {
        // Fast path: have buffered data or terminal state.
        {
            let mut st = sess.state.lock().await;
            if let Some(e) = st.error.clone() {
                return Err(e);
            }
            if st.queue.is_empty() {
                if st.done {
                    return Ok(StreamReadReply {
                        eof: true,
                        data_b64: String::new(),
                    });
                }
            } else {
                let mut out: Vec<u8> = Vec::with_capacity(max_len);
                while out.len() < max_len {
                    let Some(front) = st.queue.pop_front() else {
                        break;
                    };
                    let start = st.head_off;
                    let avail = front.len().saturating_sub(start);
                    if avail == 0 {
                        st.head_off = 0;
                        continue;
                    }
                    let take = std::cmp::min(avail, max_len - out.len());
                    out.extend_from_slice(&front[start..start + take]);
                    let next_off = start + take;
                    if next_off < front.len() {
                        // Put the remaining back as the new head.
                        st.head_off = next_off;
                        st.queue.push_front(front);
                    } else {
                        st.head_off = 0;
                    }
                }
                st.buffered = st.buffered.saturating_sub(out.len());
                let data_b64 = base64::engine::general_purpose::STANDARD.encode(out);
                return Ok(StreamReadReply {
                    eof: false,
                    data_b64,
                });
            }
        }

        // Wait for more data or close.
        sess.notify.notified().await;
    }
}

#[tauri::command]
async fn stream_close(state: State<'_, StreamProxyState>, handle: String) -> Result<(), String> {
    let sess = {
        let mut sessions = state.sessions.lock().await;
        sessions.remove(handle.trim())
    };

    if let Some(sess) = sess {
        // Abort reader task so it stops pulling remote data.
        if let Some(task) = sess.reader_task.lock().await.take() {
            task.abort();
            let _ = task.await;
        }
        let mut st = sess.state.lock().await;
        st.done = true;
        sess.notify.notify_waiters();
    }
    Ok(())
}

#[tauri::command]
async fn subtitle_search(
    query: String,
    min_score: Option<f64>,
    lang: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ThunderSubtitleItem>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.unwrap_or(50).clamp(1, 200);
    subtitle::core::search_items(
        &query,
        limit,
        min_score,
        lang.as_deref(),
        Duration::from_secs(20),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn livestream_decode_manifest(
    input: String,
) -> Result<livestream_ui::LivestreamUiManifest, String> {
    let client = LivestreamClient::new().map_err(|e| e.to_string())?;
    let man = client
        .decode_manifest(&input, ResolveOptions::default())
        .await
        .map_err(|e| e.to_string())?;
    Ok(livestream_ui::map_manifest(man))
}

#[tauri::command]
async fn livestream_resolve_variant(
    site: String,
    room_id: String,
    variant_id: String,
) -> Result<livestream_ui::LivestreamUiVariant, String> {
    let site = parse_site_str(&site)?;
    let client = LivestreamClient::new().map_err(|e| e.to_string())?;
    let v = client
        .resolve_variant(site, &room_id, &variant_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(livestream_ui::map_variant(v))
}

#[tauri::command]
async fn subtitle_download(
    item: ThunderSubtitleItem,
    out_dir: String,
    overwrite: Option<bool>,
) -> Result<String, String> {
    let out_dir = PathBuf::from(out_dir);
    let path = subtitle::core::download_item(
        &item,
        &out_dir,
        Duration::from_secs(60),
        2,
        overwrite.unwrap_or(false),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
struct AppInfo {
    version: String,
    homepage: String,
}

#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        homepage: HOMEPAGE.to_string(),
    }
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let u = url::Url::parse(&url).map_err(|e| e.to_string())?;
    match u.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported url scheme: {other}")),
    }
    open::that(url).map_err(|e| e.to_string())
}

#[tauri::command]
async fn now_playing_snapshot(
    include_thumbnail: Option<bool>,
    max_thumbnail_bytes: Option<u32>,
    max_sessions: Option<u32>,
) -> Result<now_playing::NowPlayingSnapshot, String> {
    let include_thumbnail = include_thumbnail.unwrap_or(true);
    let opt = now_playing::NowPlayingOptions {
        include_thumbnail,
        max_thumbnail_bytes: max_thumbnail_bytes.unwrap_or(if include_thumbnail {
            2_500_000
        } else {
            262_144
        }) as usize,
        max_sessions: max_sessions.unwrap_or(32) as usize,
    };
    tokio::task::spawn_blocking(move || now_playing::snapshot(opt))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn lyrics_search(
    title: String,
    album: Option<String>,
    artist: Option<String>,
    duration_ms: Option<u64>,
    limit: Option<usize>,
    strict_match: Option<bool>,
    services_csv: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<Vec<lyrics::model::LyricsSearchResult>, String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Ok(Vec::new());
    }

    let artist = artist
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let album = album
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let term = match artist {
        Some(artist) => lyrics::model::LyricsSearchTerm::Info {
            title,
            artist,
            album,
        },
        None => lyrics::model::LyricsSearchTerm::Keyword { keyword: title },
    };

    let mut req = lyrics::model::LyricsSearchRequest::new(term);
    req.duration_ms = duration_ms.filter(|v| *v > 0);
    req.limit = limit.unwrap_or(10).clamp(1, 50);

    let mut opt = lyrics::model::LyricsSearchOptions::default();
    opt.timeout_ms = timeout_ms.unwrap_or(8000).max(1);
    opt.strict_match = strict_match.unwrap_or(false);

    // Default to a stable, fast subset for UI responsiveness.
    let csv = services_csv
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "qq,netease,lrclib".to_string());
    let mut services = Vec::new();
    for part in csv.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        services.push(lyrics::model::LyricsService::from_str(p).map_err(|e| e.to_string())?);
    }
    if !services.is_empty() {
        opt.services = services;
    }

    lyrics::core::search(&req, opt)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
struct DanmakuConnectReply {
    site: String,
    room_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DanmakuImageReply {
    mime: String,
    bytes: Vec<u8>,
}

fn image_referer(site: Option<String>, room_id: Option<String>, url: &url::Url) -> Option<String> {
    let host = url.host_str().unwrap_or_default().to_lowercase();
    let site = site.unwrap_or_default().to_lowercase();
    let room_id = room_id.unwrap_or_default();

    // Common anti-hotlink behavior: bilibili/hdslb emoji/images require a live.bilibili.com referer.
    if site.contains("bili") || host.contains("bilibili.com") || host.contains("hdslb.com") {
        if room_id.trim().is_empty() {
            return Some("https://live.bilibili.com/".to_string());
        }
        return Some(format!("https://live.bilibili.com/{}/", room_id.trim()));
    }

    None
}

fn is_local_or_private_host(u: &url::Url) -> bool {
    use url::Host;

    let Some(host) = u.host() else {
        return true;
    };

    match host {
        Host::Domain(d) => {
            let h = d.to_lowercase();
            h == "localhost"
        }
        Host::Ipv4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        Host::Ipv6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::is_local_or_private_host;

    fn parse(url: &str) -> url::Url {
        url::Url::parse(url).expect("valid url")
    }

    #[test]
    fn blocks_localhost_and_private_ipv4() {
        assert!(is_local_or_private_host(&parse(
            "http://localhost:8080/a.png"
        )));
        assert!(is_local_or_private_host(&parse("http://127.0.0.1/a.png")));
        assert!(is_local_or_private_host(&parse(
            "http://192.168.1.10/a.png"
        )));
        assert!(is_local_or_private_host(&parse("http://10.0.0.5/a.png")));
    }

    #[test]
    fn blocks_link_local_ipv4() {
        assert!(is_local_or_private_host(&parse("http://169.254.1.2/a.png")));
    }

    #[test]
    fn blocks_private_ipv6_ranges() {
        assert!(is_local_or_private_host(&parse("http://[::1]/a.png")));
        assert!(is_local_or_private_host(&parse("http://[fe80::1]/a.png")));
        assert!(is_local_or_private_host(&parse("http://[fd00::1]/a.png")));
    }

    #[test]
    fn allows_public_hosts() {
        assert!(!is_local_or_private_host(&parse(
            "https://example.com/a.png"
        )));
        assert!(!is_local_or_private_host(&parse("http://8.8.8.8/a.png")));
    }
}

#[tauri::command]
async fn danmaku_fetch_image(
    url: String,
    site: Option<String>,
    room_id: Option<String>,
) -> Result<DanmakuImageReply, String> {
    let u = url::Url::parse(&url).map_err(|e| e.to_string())?;
    match u.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported url scheme: {other}")),
    }
    if is_local_or_private_host(&u) {
        return Err("blocked host".to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent("chaos-seed/0.1 (tauri)")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.get(u.clone());
    if let Some(r) = image_referer(site, room_id, &u) {
        req = req.header(reqwest::header::REFERER, r);
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("http {} when fetching image", resp.status()));
    }

    if let Some(len) = resp.content_length() {
        // Prevent pathological payloads.
        if len > 2_500_000 {
            return Err(format!("image too large: {len} bytes"));
        }
    }

    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(';').next())
        .unwrap_or("image/png")
        .to_string();

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
    if bytes.len() > 2_500_000 {
        return Err(format!("image too large: {} bytes", bytes.len()));
    }

    Ok(DanmakuImageReply { mime, bytes })
}

#[tauri::command]
async fn danmaku_connect(
    app: AppHandle,
    state: State<'_, DanmakuState>,
    input: String,
) -> Result<DanmakuConnectReply, String> {
    // Stop any previous connection.
    let prev = { state.active.lock().expect("danmaku mutex").take() };
    if let Some(active) = prev {
        stop_active(active).await;
    }

    let input = input.trim().to_string();
    if input.is_empty() {
        let _ = emit_to_known_windows(&app, "danmaku_status", "请输入直播间地址。");
        return Err("empty input".to_string());
    }

    let _ = emit_to_known_windows(&app, "danmaku_status", "连接中...");

    let client = DanmakuClient::new().map_err(|e| e.to_string())?;
    let target = client.resolve(&input).await.map_err(|e| e.to_string())?;

    let site = target.site.as_str().to_string();
    let room_id = target.room_id.clone();

    let (session, mut rx) = client
        .connect_resolved(target, ConnectOptions::default())
        .await
        .map_err(|e| e.to_string())?;

    let app2 = app.clone();
    let reader_task = tokio::spawn(async move {
        let _ = emit_to_known_windows(&app2, "danmaku_status", "已连接");
        while let Some(ev) = rx.recv().await {
            // Only push high-frequency danmaku messages to subscribed windows.
            // Additionally: when any auxiliary renderer window is open (Chat/Overlay),
            // we suppress `main` even if it's accidentally subscribed. This makes the
            // behavior robust against missed frontend events and prevents the "main still refreshes" issue.
            let subs: Vec<String> = {
                let st = app2.state::<DanmakuState>();
                st.msg_subscribers
                    .lock()
                    .expect("danmaku subscribers mutex")
                    .iter()
                    .cloned()
                    .collect()
            };
            let suppress_main = {
                let st = app2.state::<DanmakuState>();
                let aux = st.aux_windows.lock().expect("danmaku aux windows mutex");
                !aux.is_empty()
            };

            for msg in danmaku_ui::map_event_to_ui(ev) {
                for label in &subs {
                    if suppress_main && label == "main" {
                        continue;
                    }
                    let _ = app2.emit_to(label.as_str(), "danmaku_msg", msg.clone());
                }
            }
        }
        let _ = emit_to_known_windows(&app2, "danmaku_status", "已断开");
    });

    *state.active.lock().expect("danmaku mutex") = Some(ActiveDanmaku {
        input: input.clone(),
        site: site.clone(),
        room_id: room_id.clone(),
        session,
        reader_task,
    });
    Ok(DanmakuConnectReply { site, room_id })
}

#[tauri::command]
async fn danmaku_disconnect(app: AppHandle, state: State<'_, DanmakuState>) -> Result<(), String> {
    let prev = { state.active.lock().expect("danmaku mutex").take() };
    if let Some(active) = prev {
        let _ = emit_to_known_windows(&app, "danmaku_status", "正在断开...");
        stop_active(active).await;
    }
    let _ = emit_to_known_windows(&app, "danmaku_status", "已断开");
    Ok(())
}

fn update_msg_subscription(
    subs: &mut HashSet<String>,
    label: &str,
    enabled: bool,
    suppress_main: bool,
) {
    if enabled {
        if suppress_main && label == "main" {
            // When Chat/Overlay is open, never allow the main window to be a high-frequency subscriber.
            return;
        }
        subs.insert(label.to_string());
    } else {
        subs.remove(label);
    }
}

#[tauri::command]
fn danmaku_set_msg_subscription(
    window: tauri::Window,
    state: State<'_, DanmakuState>,
    enabled: bool,
) -> Result<(), String> {
    let label = window.label().to_string();
    let suppress_main = {
        let aux = state.aux_windows.lock().expect("danmaku aux windows mutex");
        !aux.is_empty()
    };
    let mut subs = state
        .msg_subscribers
        .lock()
        .expect("danmaku subscribers mutex");
    update_msg_subscription(&mut subs, &label, enabled, suppress_main);
    Ok(())
}

fn ensure_window(app: &AppHandle, label: &str) -> Option<tauri::WebviewWindow> {
    let w = app.get_webview_window(label)?;
    let _ = w.show();
    let _ = w.set_focus();
    Some(w)
}

fn child_url_from_main(
    app: &AppHandle,
    view: &str,
    overlay_opaque: Option<bool>,
) -> tauri::WebviewUrl {
    // Prefer reusing the main window's origin in dev mode (http(s) dev server),
    // since it is the most reliable way to ensure child windows load the exact same frontend.
    if let Some(main) = app.get_webview_window("main") {
        if let Ok(mut u) = main.url() {
            let scheme = u.scheme().to_ascii_lowercase();
            if scheme == "http" || scheme == "https" {
                u.set_fragment(None);
                u.set_username("").ok();
                u.set_password(None).ok();
                u.set_path("/");
                u.set_query(None);
                let mut pairs: Vec<(String, String)> = Vec::new();
                pairs.push(("view".to_string(), view.to_string()));
                if view == "overlay" {
                    if overlay_opaque.unwrap_or(false) {
                        pairs.push(("overlay".to_string(), "opaque".to_string()));
                    }
                }
                let query = url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
                    .finish();
                u.set_query(Some(&query));
                return tauri::WebviewUrl::External(u);
            }
        }
    }

    // Fallback for non-http schemes (tauri:// in production): use the app URL + init script boot params.
    tauri::WebviewUrl::App("index.html".into())
}

// NOTE: Child windows must load the same frontend origin as the main window.
//
// In dev mode, the main window is typically `http(s)://...` (Vite dev server). We reuse that URL so
// Chat/Overlay are guaranteed to load the same assets (and we avoid `tauri://` proxy quirks).
//
// In production, the main window uses the app protocol (`tauri://...`). We fall back to
// `WebviewUrl::App("index.html")` and use init-script boot params (since `App(PathBuf)` cannot carry query).

fn debug_enabled() -> bool {
    match std::env::var("CHAOS_SEED_DEBUG") {
        Ok(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}

fn child_init_script(boot: serde_json::Value) -> String {
    let boot_json = boot.to_string();
    if !debug_enabled() {
        return format!("window.__CHAOS_SEED_BOOT = {boot_json};");
    }

    // A tiny debug strip that renders even if the app bundle fails to mount.
    // Enabled only when CHAOS_SEED_DEBUG=1.
    let title = boot
        .get("view")
        .and_then(|v| v.as_str())
        .map(|v| format!("Chaos Seed [{v}]"))
        .unwrap_or_else(|| "Chaos Seed [boot]".to_string());
    let title_json = serde_json::Value::String(title).to_string();

    format!(
        r#"
window.__CHAOS_SEED_BOOT = {boot_json};
(function() {{ try {{ document.title = {title_json}; }} catch (e) {{}} }})();
(function() {{
  function safe(v) {{ try {{ return JSON.stringify(v); }} catch (e) {{ return String(v); }} }}
  function text() {{
    var b = window.__CHAOS_SEED_BOOT;
    var hasTauri = typeof window.__TAURI__ !== 'undefined';
    var out = '[ChaosSeed BOOT] ' + (b && b.label ? b.label : '?') + ' view=' + (b && b.view ? b.view : '?');
    out += ' opaque=' + (b && b.overlayOpaque ? '1' : '0');
    out += '\\nurl=' + String(window.location.href);
    out += '\\n__TAURI__=' + (hasTauri ? 'yes' : 'no');
    out += '\\nboot=' + safe(b);
    if (window.__CHAOS_SEED_LAST_ERR) out += '\\nERR=' + window.__CHAOS_SEED_LAST_ERR;
    return out;
  }}
  function mount() {{
    try {{
      var d = document.getElementById('__chaos_seed_boot_bar');
      if (!d) {{
        d = document.createElement('div');
        d.id = '__chaos_seed_boot_bar';
        d.style.cssText = 'position:fixed;left:0;right:0;top:0;z-index:2147483647;' +
          'background:rgba(0,0,0,0.85);color:#a7f3d0;font:12px/1.35 ui-monospace,Consolas,monospace;' +
          'padding:6px 8px;white-space:pre-wrap;pointer-events:none;';
        document.documentElement.appendChild(d);
      }}
      d.textContent = text();
    }} catch (e) {{}}
  }}
  window.addEventListener('error', function(ev) {{
    try {{ window.__CHAOS_SEED_LAST_ERR = String(ev && (ev.message || ev.error || ev)); mount(); }} catch (e) {{}}
  }});
  window.addEventListener('unhandledrejection', function(ev) {{
    try {{ window.__CHAOS_SEED_LAST_ERR = String(ev && (ev.reason || ev)); mount(); }} catch (e) {{}}
  }});
  window.addEventListener('DOMContentLoaded', mount, {{ once: true }});
  if (document.readyState === 'interactive' || document.readyState === 'complete') mount();
  setInterval(mount, 1000);
}})();
"#
    )
}

fn init_lyrics_tray(app: &AppHandle) -> Result<(), String> {
    #[cfg(not(desktop))]
    {
        let _ = app;
        return Ok(());
    }

    #[cfg(desktop)]
    {
        use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItem};
        use tauri::tray::TrayIconBuilder;

        let enabled = app
            .state::<LyricsAppState>()
            .detection_enabled
            .load(Ordering::Relaxed);

        let item_open = MenuItem::with_id(app, "tray_open_main", "打开主界面", true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let item_detect =
            CheckMenuItemBuilder::with_id("tray_toggle_detection", "歌词检测（开/关）")
                .checked(enabled)
                .build(app)
                .map_err(|e| e.to_string())?;

        let item_dock =
            MenuItem::with_id(app, "tray_open_dock", "打开窄屏歌词窗", true, None::<&str>)
                .map_err(|e| e.to_string())?;

        let item_float =
            MenuItem::with_id(app, "tray_open_float", "打开顶栏歌词", true, None::<&str>)
                .map_err(|e| e.to_string())?;

        let item_quit = MenuItem::with_id(app, "tray_quit", "退出", true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let menu = MenuBuilder::new(app)
            .item(&item_open)
            .item(&item_detect)
            .separator()
            .item(&item_dock)
            .item(&item_float)
            .separator()
            .item(&item_quit)
            .build()
            .map_err(|e| e.to_string())?;

        let icon = app.default_window_icon().cloned();

        let mut builder = TrayIconBuilder::with_id("chaos_seed_tray")
            .menu(&menu)
            .tooltip("chaos-seed")
            .on_menu_event(move |app, ev| {
                let id = ev.id();
                if id == "tray_open_main" {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                    return;
                }
                if id == "tray_open_dock" {
                    let app2 = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = open_lyrics_dock_window(app2).await;
                    });
                    return;
                }
                if id == "tray_open_float" {
                    let app2 = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = open_lyrics_float_window(app2).await;
                    });
                    return;
                }
                if id == "tray_toggle_detection" {
                    let st = app.state::<LyricsAppState>();
                    let mut s = st.settings.lock().expect("lyrics settings mutex");
                    let next = !s.lyrics_detection_enabled;
                    s.lyrics_detection_enabled = next;
                    st.detection_enabled.store(next, Ordering::Relaxed);
                    let _ = lyrics_app::save_settings(app, &s);
                    drop(s);
                    let _ = item_detect.set_checked(next);
                    lyrics_app::emit_detection_state(app, next);
                    return;
                }
                if id == "tray_quit" {
                    app.exit(0);
                }
            });

        if let Some(icon) = icon {
            builder = builder.icon(icon);
        }

        let _ = builder.build(app);
        Ok(())
    }
}

#[tauri::command]
async fn open_chat_window(app: AppHandle) -> Result<(), String> {
    if ensure_window(&app, "chat").is_some() {
        // Keep backend state consistent even if the window already exists.
        let st = app.state::<DanmakuState>();
        {
            let mut aux = st.aux_windows.lock().expect("danmaku aux windows mutex");
            aux.insert("chat".to_string());
        }
        {
            let mut subs = st
                .msg_subscribers
                .lock()
                .expect("danmaku subscribers mutex");
            subs.remove("main");
        }
        return Ok(());
    }
    let boot =
        serde_json::json!({ "view": "chat", "label": "chat", "build": env!("CARGO_PKG_VERSION") });
    let init_script = child_init_script(boot);
    let url = child_url_from_main(&app, "chat", None);
    if debug_enabled() {
        println!("[tauri] open_chat_window url={url}");
    }
    let mut builder = tauri::WebviewWindowBuilder::new(&app, "chat", url)
        .title("弹幕 - Chat")
        .inner_size(420.0, 640.0)
        .resizable(true)
        // Transparent multi-window WebView2 can be flaky on some machines; keep Chat opaque for stability.
        .transparent(false)
        .initialization_script(init_script);
    if debug_enabled() {
        let eval_script = child_init_script(serde_json::json!({
            "view": "chat",
            "label": "chat",
            "build": env!("CARGO_PKG_VERSION")
        }));
        builder = builder.on_page_load(move |window, payload| {
            use tauri::webview::PageLoadEvent;
            match payload.event() {
                PageLoadEvent::Started => {
                    println!("[tauri] chat PageLoad Started url={}", payload.url());
                }
                PageLoadEvent::Finished => {
                    println!("[tauri] chat PageLoad Finished url={}", payload.url());
                    // Fallback: inject the boot bar again after load in case init scripts didn't run.
                    let _ = window.eval(eval_script.clone());
                }
            }
        });
    }
    let w = builder.build().map_err(|e| e.to_string())?;
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit(
        "chaos_window_state",
        WindowStatePayload {
            label: "chat".to_string(),
            open: true,
        },
    );
    // Authoritative suppression: when Chat is open, ensure main stops receiving high-frequency messages.
    {
        let st = app.state::<DanmakuState>();
        let mut aux = st.aux_windows.lock().expect("danmaku aux windows mutex");
        aux.insert("chat".to_string());
    }
    {
        let st = app.state::<DanmakuState>();
        let mut subs = st
            .msg_subscribers
            .lock()
            .expect("danmaku subscribers mutex");
        subs.remove("main");
    }
    {
        let app2 = app.clone();
        let label = "chat".to_string();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let st = app2.state::<DanmakuState>();
                {
                    let mut aux = st.aux_windows.lock().expect("danmaku aux windows mutex");
                    aux.remove(&label);
                }
                let mut subs = st
                    .msg_subscribers
                    .lock()
                    .expect("danmaku subscribers mutex");
                subs.remove(&label);
                let _ = app2.emit(
                    "chaos_window_state",
                    WindowStatePayload {
                        label: label.clone(),
                        open: false,
                    },
                );
            }
        });
    }
    // `cfg!(debug_assertions)` is a runtime check and still compiles the block in release,
    // but `open_devtools` is not always available depending on tauri/wry feature flags.
    // Gate it at compile time so release builds never reference the method.
    #[cfg(debug_assertions)]
    {
        if std::env::var("CHAOS_SEED_CHILD_DEVTOOLS").is_ok() {
            w.open_devtools();
        }
    }

    Ok(())
}

#[tauri::command]
async fn open_overlay_window(app: AppHandle, opaque: Option<bool>) -> Result<(), String> {
    if let Some(_w) = ensure_window(&app, "overlay") {
        // Keep backend state consistent even if the window already exists.
        let st = app.state::<DanmakuState>();
        {
            let mut aux = st.aux_windows.lock().expect("danmaku aux windows mutex");
            aux.insert("overlay".to_string());
        }
        {
            let mut subs = st
                .msg_subscribers
                .lock()
                .expect("danmaku subscribers mutex");
            subs.remove("main");
        }
        return Ok(());
    }
    let opaque = opaque.unwrap_or(false);
    let boot = serde_json::json!({
        "view": "overlay",
        "label": "overlay",
        "overlayOpaque": opaque,
        "build": env!("CARGO_PKG_VERSION")
    });
    let init_script = child_init_script(boot);
    let url = child_url_from_main(&app, "overlay", Some(opaque));
    if debug_enabled() {
        println!("[tauri] open_overlay_window opaque={opaque} url={url}");
    }
    let mut builder = tauri::WebviewWindowBuilder::new(&app, "overlay", url)
        .title("弹幕 - Overlay")
        .inner_size(960.0, 320.0)
        .resizable(true)
        // Keep native titlebar so the window can be moved/resized/closed without relying on JS.
        .decorations(true)
        // Default to opaque on Windows for stability; transparent overlay is still available via settings.
        .transparent(!opaque)
        .always_on_top(true)
        .initialization_script(init_script);
    if debug_enabled() {
        let eval_script = child_init_script(serde_json::json!({
            "view": "overlay",
            "label": "overlay",
            "overlayOpaque": opaque,
            "build": env!("CARGO_PKG_VERSION")
        }));
        builder = builder.on_page_load(move |window, payload| {
            use tauri::webview::PageLoadEvent;
            match payload.event() {
                PageLoadEvent::Started => {
                    println!("[tauri] overlay PageLoad Started url={}", payload.url());
                }
                PageLoadEvent::Finished => {
                    println!("[tauri] overlay PageLoad Finished url={}", payload.url());
                    let _ = window.eval(eval_script.clone());
                }
            }
        });
    }

    let w = builder.build().map_err(|e| e.to_string())?;
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit(
        "chaos_window_state",
        WindowStatePayload {
            label: "overlay".to_string(),
            open: true,
        },
    );
    // Authoritative suppression: when Overlay is open, ensure main stops receiving high-frequency messages.
    {
        let st = app.state::<DanmakuState>();
        let mut aux = st.aux_windows.lock().expect("danmaku aux windows mutex");
        aux.insert("overlay".to_string());
    }
    {
        let st = app.state::<DanmakuState>();
        let mut subs = st
            .msg_subscribers
            .lock()
            .expect("danmaku subscribers mutex");
        subs.remove("main");
    }
    {
        let app2 = app.clone();
        let label = "overlay".to_string();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let st = app2.state::<DanmakuState>();
                {
                    let mut aux = st.aux_windows.lock().expect("danmaku aux windows mutex");
                    aux.remove(&label);
                }
                let mut subs = st
                    .msg_subscribers
                    .lock()
                    .expect("danmaku subscribers mutex");
                subs.remove(&label);
                let _ = app2.emit(
                    "chaos_window_state",
                    WindowStatePayload {
                        label: label.clone(),
                        open: false,
                    },
                );
            }
        });
    }
    #[cfg(debug_assertions)]
    {
        if std::env::var("CHAOS_SEED_CHILD_DEVTOOLS").is_ok() {
            w.open_devtools();
        }
    }
    Ok(())
}

#[tauri::command]
fn lyrics_get_current(
    state: State<'_, LyricsWindowState>,
) -> Option<lyrics::model::LyricsSearchResult> {
    state
        .current
        .lock()
        .expect("lyrics window state mutex")
        .clone()
}

#[tauri::command]
fn lyrics_settings_get(
    app: AppHandle,
    state: State<'_, LyricsAppState>,
) -> Result<LyricsAppSettings, String> {
    let loaded = lyrics_app::load_settings(&app).unwrap_or_default();
    {
        let mut s = state.settings.lock().expect("lyrics settings mutex");
        *s = loaded.clone();
    }
    state
        .detection_enabled
        .store(loaded.lyrics_detection_enabled, Ordering::Relaxed);
    Ok(loaded)
}

#[tauri::command]
fn lyrics_settings_set(
    app: AppHandle,
    state: State<'_, LyricsAppState>,
    partial: LyricsAppSettingsPatch,
) -> Result<LyricsAppSettings, String> {
    let next = state.apply_settings_patch(partial);
    lyrics_app::save_settings(&app, &next)?;
    lyrics_app::emit_detection_state(&app, next.lyrics_detection_enabled);
    Ok(next)
}

#[tauri::command]
fn lyrics_detection_set_enabled(
    app: AppHandle,
    state: State<'_, LyricsAppState>,
    enabled: bool,
) -> Result<(), String> {
    let next = state.apply_settings_patch(LyricsAppSettingsPatch {
        lyrics_detection_enabled: Some(enabled),
        auto_hide_on_pause: None,
        auto_hide_delay_ms: None,
        matching_threshold: None,
        timeout_ms: None,
        limit: None,
        providers_order: None,
        default_display: None,
        effects: None,
    });
    lyrics_app::save_settings(&app, &next)?;
    lyrics_app::emit_detection_state(&app, enabled);
    Ok(())
}

#[tauri::command]
fn lyrics_set_current(
    app: AppHandle,
    state: State<'_, LyricsWindowState>,
    payload: lyrics::model::LyricsSearchResult,
) -> Result<(), String> {
    *state.current.lock().expect("lyrics window state mutex") = Some(payload.clone());
    app.emit("lyrics_current_changed", payload)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_lyrics_chat_window(app: AppHandle) -> Result<(), String> {
    if ensure_window(&app, "lyrics_chat").is_some() {
        return Ok(());
    }

    let boot = serde_json::json!({
        "view": "lyrics_chat",
        "label": "lyrics_chat",
        "build": env!("CARGO_PKG_VERSION")
    });
    let init_script = child_init_script(boot);
    let url = child_url_from_main(&app, "lyrics_chat", None);
    if debug_enabled() {
        println!("[tauri] open_lyrics_chat_window url={url}");
    }

    let w = tauri::WebviewWindowBuilder::new(&app, "lyrics_chat", url)
        .title("歌词 - Chat")
        .inner_size(520.0, 720.0)
        .resizable(true)
        .transparent(false)
        .initialization_script(init_script)
        .build()
        .map_err(|e| e.to_string())?;

    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit(
        "chaos_window_state",
        WindowStatePayload {
            label: "lyrics_chat".to_string(),
            open: true,
        },
    );
    {
        let app2 = app.clone();
        let label = "lyrics_chat".to_string();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let _ = app2.emit(
                    "chaos_window_state",
                    WindowStatePayload {
                        label: label.clone(),
                        open: false,
                    },
                );
            }
        });
    }

    #[cfg(debug_assertions)]
    {
        if std::env::var("CHAOS_SEED_CHILD_DEVTOOLS").is_ok() {
            w.open_devtools();
        }
    }

    Ok(())
}

#[tauri::command]
async fn open_lyrics_overlay_window(app: AppHandle) -> Result<(), String> {
    if ensure_window(&app, "lyrics_overlay").is_some() {
        return Ok(());
    }

    let boot = serde_json::json!({
        "view": "lyrics_overlay",
        "label": "lyrics_overlay",
        "build": env!("CARGO_PKG_VERSION")
    });
    let init_script = child_init_script(boot);
    let url = child_url_from_main(&app, "lyrics_overlay", None);
    if debug_enabled() {
        println!("[tauri] open_lyrics_overlay_window url={url}");
    }

    let w = tauri::WebviewWindowBuilder::new(&app, "lyrics_overlay", url)
        .title("歌词 - Overlay")
        .inner_size(960.0, 200.0)
        .resizable(true)
        .decorations(true)
        .transparent(true)
        .always_on_top(true)
        .initialization_script(init_script)
        .build()
        .map_err(|e| e.to_string())?;

    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit(
        "chaos_window_state",
        WindowStatePayload {
            label: "lyrics_overlay".to_string(),
            open: true,
        },
    );
    {
        let app2 = app.clone();
        let label = "lyrics_overlay".to_string();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let _ = app2.emit(
                    "chaos_window_state",
                    WindowStatePayload {
                        label: label.clone(),
                        open: false,
                    },
                );
            }
        });
    }

    #[cfg(debug_assertions)]
    {
        if std::env::var("CHAOS_SEED_CHILD_DEVTOOLS").is_ok() {
            w.open_devtools();
        }
    }

    Ok(())
}

#[tauri::command]
async fn open_lyrics_dock_window(app: AppHandle) -> Result<(), String> {
    if ensure_window(&app, "lyrics_dock").is_some() {
        let st = app.state::<LyricsAppState>();
        st.dock_open.store(true, Ordering::Relaxed);
        let _ = app.emit(
            "chaos_window_state",
            WindowStatePayload {
                label: "lyrics_dock".to_string(),
                open: true,
            },
        );
        return Ok(());
    }

    let boot = serde_json::json!({
        "view": "lyrics_dock",
        "label": "lyrics_dock",
        "build": env!("CARGO_PKG_VERSION")
    });
    let init_script = child_init_script(boot);
    let url = child_url_from_main(&app, "lyrics_dock", None);

    let w = tauri::WebviewWindowBuilder::new(&app, "lyrics_dock", url)
        .title("歌词 - Dock")
        .inner_size(420.0, 720.0)
        .resizable(true)
        .decorations(false)
        .transparent(false)
        .always_on_top(true)
        .initialization_script(init_script)
        .build()
        .map_err(|e| e.to_string())?;

    // Place it near the top-center of the main window's monitor (avoid "lost window" on multi-monitor setups).
    let mon = app
        .get_webview_window("main")
        .and_then(|mw| mw.current_monitor().ok().flatten())
        .or_else(|| w.current_monitor().ok().flatten());
    if let Some(m) = mon {
        let size = m.size();
        let pos = m.position();
        let x = pos.x + ((size.width as i32 - 420) / 2);
        let y = pos.y + 60;
        let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
    }

    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit(
        "chaos_window_state",
        WindowStatePayload {
            label: "lyrics_dock".to_string(),
            open: true,
        },
    );
    {
        let st = app.state::<LyricsAppState>();
        st.dock_open.store(true, Ordering::Relaxed);
    }
    {
        let app2 = app.clone();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let st = app2.state::<LyricsAppState>();
                st.dock_open.store(false, Ordering::Relaxed);
                let _ = app2.emit(
                    "chaos_window_state",
                    WindowStatePayload {
                        label: "lyrics_dock".to_string(),
                        open: false,
                    },
                );
            }
        });
    }
    Ok(())
}

#[tauri::command]
async fn open_lyrics_float_window(app: AppHandle) -> Result<(), String> {
    if ensure_window(&app, "lyrics_float").is_some() {
        let st = app.state::<LyricsAppState>();
        st.float_open.store(true, Ordering::Relaxed);
        let _ = app.emit(
            "chaos_window_state",
            WindowStatePayload {
                label: "lyrics_float".to_string(),
                open: true,
            },
        );
        return Ok(());
    }

    let boot = serde_json::json!({
        "view": "lyrics_float",
        "label": "lyrics_float",
        "build": env!("CARGO_PKG_VERSION")
    });
    let init_script = child_init_script(boot);
    let url = child_url_from_main(&app, "lyrics_float", None);

    let w = tauri::WebviewWindowBuilder::new(&app, "lyrics_float", url)
        .title("歌词 - TopBar")
        .inner_size(1200.0, 72.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .initialization_script(init_script)
        .build()
        .map_err(|e| e.to_string())?;

    // Snap to the top edge and span the current monitor width.
    let mon = app
        .get_webview_window("main")
        .and_then(|mw| mw.current_monitor().ok().flatten())
        .or_else(|| w.current_monitor().ok().flatten());
    if let Some(m) = mon {
        let size = m.size();
        let pos = m.position();
        let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: pos.x,
            y: pos.y,
        }));
        let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: size.width,
            height: 72,
        }));
    }

    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit(
        "chaos_window_state",
        WindowStatePayload {
            label: "lyrics_float".to_string(),
            open: true,
        },
    );
    {
        let st = app.state::<LyricsAppState>();
        st.float_open.store(true, Ordering::Relaxed);
    }
    {
        let app2 = app.clone();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let st = app2.state::<LyricsAppState>();
                st.float_open.store(false, Ordering::Relaxed);
                let _ = app2.emit(
                    "chaos_window_state",
                    WindowStatePayload {
                        label: "lyrics_float".to_string(),
                        open: false,
                    },
                );
            }
        });
    }
    Ok(())
}

#[tauri::command]
async fn open_player_window(
    app: AppHandle,
    req: livestream_ui::PlayerBootRequest,
    from_rect: Option<livestream_ui::WindowRect>,
) -> Result<(), String> {
    if let Some(_w) = ensure_window(&app, "player") {
        // Best-effort: ask the player window to load a new source.
        let _ = emit_to_known_windows(&app, "player_load", req);
        return Ok(());
    }

    let has_hero = from_rect.is_some();
    // Used by the frontend as the minimum time to keep the hero/poster overlay visible.
    // We still start loading immediately to overlap buffering/decoder init with the window animation.
    let hero_delay_ms: u32 = if from_rect.is_some() { 220 } else { 0 };
    let boot = serde_json::json!({
        "view": "player",
        "label": "player",
        "player": req,
        "heroDelayMs": hero_delay_ms,
        "build": env!("CARGO_PKG_VERSION")
    });
    let init_script = child_init_script(boot);
    let url = child_url_from_main(&app, "player", None);
    if debug_enabled() {
        println!("[tauri] open_player_window url={url}");
    }

    let mut builder = tauri::WebviewWindowBuilder::new(&app, "player", url)
        .title("播放器 - Live")
        .inner_size(960.0, 540.0)
        .resizable(true)
        .transparent(false)
        .initialization_script(init_script);

    if has_hero {
        // Avoid a visible "first frame" at default position/size before we apply the hero rect.
        builder = builder.visible(false);
    }

    if debug_enabled() {
        let eval_script = child_init_script(serde_json::json!({
            "view": "player",
            "label": "player",
            "build": env!("CARGO_PKG_VERSION")
        }));
        builder = builder.on_page_load(move |window, payload| {
            use tauri::webview::PageLoadEvent;
            match payload.event() {
                PageLoadEvent::Started => {
                    println!("[tauri] player PageLoad Started url={}", payload.url());
                }
                PageLoadEvent::Finished => {
                    println!("[tauri] player PageLoad Finished url={}", payload.url());
                    let _ = window.eval(eval_script.clone());
                }
            }
        });
    }

    let w = builder.build().map_err(|e| e.to_string())?;

    let from_rect = from_rect.map(|mut r| {
        if r.width < 80 {
            r.width = 80;
        }
        if r.height < 60 {
            r.height = 60;
        }
        r
    });

    if let Some(r) = from_rect.as_ref() {
        let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: r.x,
            y: r.y,
        }));
        let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: r.width,
            height: r.height,
        }));
    }

    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit(
        "chaos_window_state",
        WindowStatePayload {
            label: "player".to_string(),
            open: true,
        },
    );
    {
        let app2 = app.clone();
        let label = "player".to_string();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let _ = app2.emit(
                    "chaos_window_state",
                    WindowStatePayload {
                        label: label.clone(),
                        open: false,
                    },
                );
            }
        });
    }

    if let Some(fr) = from_rect {
        let app2 = app.clone();
        let w2 = w.clone();
        tauri::async_runtime::spawn(async move {
            let mon = app2
                .get_webview_window("main")
                .and_then(|mw| mw.current_monitor().ok().flatten())
                .or_else(|| w2.current_monitor().ok().flatten());

            let (mon_pos, mon_size) = mon.map(|m| (*m.position(), *m.size())).unwrap_or((
                tauri::PhysicalPosition { x: 0, y: 0 },
                tauri::PhysicalSize {
                    width: 1920,
                    height: 1080,
                },
            ));

            let sf = w2.scale_factor().unwrap_or(1.0);
            let mut tw = (960.0 * sf).round() as u32;
            let mut th = (540.0 * sf).round() as u32;

            let max_w = mon_size.width.saturating_sub(40);
            let max_h = mon_size.height.saturating_sub(40);
            if tw > max_w || th > max_h {
                let aspect = 540.0 / 960.0;
                tw = tw.min(max_w);
                th = (tw as f64 * aspect).round() as u32;
                if th > max_h {
                    th = th.min(max_h);
                    tw = (th as f64 / aspect).round() as u32;
                }
            }

            let cx = fr.x.saturating_add((fr.width / 2) as i32);
            let cy = fr.y.saturating_add((fr.height / 2) as i32);

            let mut tx = cx.saturating_sub((tw / 2) as i32);
            let mut ty = cy.saturating_sub((th / 2) as i32);

            let min_x = mon_pos.x;
            let min_y = mon_pos.y;
            let max_x = mon_pos
                .x
                .saturating_add(mon_size.width as i32)
                .saturating_sub(tw as i32);
            let max_y = mon_pos
                .y
                .saturating_add(mon_size.height as i32)
                .saturating_sub(th as i32);

            tx = tx.clamp(min_x, max_x);
            ty = ty.clamp(min_y, max_y);

            let to = livestream_ui::WindowRect {
                x: tx,
                y: ty,
                width: tw,
                height: th,
            };

            // Give the window manager a frame to settle after show() to reduce occasional jitter.
            tokio::time::sleep(Duration::from_millis(16)).await;
            animate_window_rect(w2, fr, to, 220).await;
        });
    }

    #[cfg(debug_assertions)]
    {
        if std::env::var("CHAOS_SEED_CHILD_DEVTOOLS").is_ok() {
            w.open_devtools();
        }
    }

    Ok(())
}

async fn animate_window_rect(
    w: tauri::WebviewWindow,
    from: livestream_ui::WindowRect,
    to: livestream_ui::WindowRect,
    duration_ms: u64,
) {
    let frames = (duration_ms / 16).max(8);
    for i in 0..=frames {
        let t = (i as f64) / (frames as f64);
        let e = 1.0 - (1.0 - t).powi(3); // easeOutCubic

        let lerp_i32 = |a: i32, b: i32| -> i32 { (a as f64 + (b - a) as f64 * e).round() as i32 };
        let lerp_u32 =
            |a: u32, b: u32| -> u32 { (a as f64 + (b - a) as f64 * e).round().max(1.0) as u32 };

        let x = lerp_i32(from.x, to.x);
        let y = lerp_i32(from.y, to.y);
        let width = lerp_u32(from.width, to.width);
        let height = lerp_u32(from.height, to.height);

        let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }));

        tokio::time::sleep(Duration::from_millis(16)).await;
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct RgbColor {
    r: u8,
    g: u8,
    b: u8,
}

#[tauri::command]
fn system_accent_rgb() -> Result<RgbColor, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::UI::ViewManagement::{UIColorType, UISettings};

        let ui = UISettings::new().map_err(|e| format!("{e}"))?;
        let c = ui
            .GetColorValue(UIColorType::Accent)
            .map_err(|e| format!("{e}"))?;

        Ok(RgbColor {
            r: c.R,
            g: c.G,
            b: c.B,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("unsupported".to_string())
    }
}

#[tauri::command]
fn set_backdrop(app: AppHandle, mode: String) -> Result<(), String> {
    // Silence unused-variable warnings on non-Windows builds (the implementation is Windows-only).
    let _ = (&app, &mode);
    // Best-effort: on non-Windows, or if a window is missing/not transparent, just no-op.
    #[cfg(target_os = "windows")]
    {
        use tauri::window::{Effect, EffectsBuilder};

        let m = mode.trim().to_ascii_lowercase();
        for label in ["main", "chat"] {
            let Some(w) = app.get_webview_window(label) else {
                continue;
            };
            if m == "mica" {
                let _ = w.set_effects(EffectsBuilder::new().effect(Effect::Mica).build());
            } else {
                let _ = w.set_effects(None);
            }
        }
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(not(target_os = "windows"))]
            {
                let _ = app;
            }
            // Enable Mica by default on Windows for a more native Win11 look.
            // The frontend can later call `set_backdrop` to switch to `none` without restarting.
            #[cfg(target_os = "windows")]
            {
                use tauri::window::{Effect, EffectsBuilder};
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_effects(EffectsBuilder::new().effect(Effect::Mica).build());
                }
            }

            // Lyrics app settings + background watcher (Now Playing -> events -> auto search).
            {
                let handle = app.handle().clone();
                let st = handle.state::<LyricsAppState>();
                if let Ok(settings) = lyrics_app::load_settings(&handle) {
                    st.detection_enabled
                        .store(settings.lyrics_detection_enabled, Ordering::Relaxed);
                    *st.settings.lock().expect("lyrics settings mutex") = settings.clone();
                    lyrics_app::emit_detection_state(&handle, settings.lyrics_detection_enabled);
                }

                // The watch loop is cheap when disabled; it self-throttles.
                tauri::async_runtime::spawn(lyrics_app::start_watch_loop(handle.clone()));

                // Tray is best-effort. If it fails, the app should still run.
                let _ = init_lyrics_tray(&handle);
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .manage(DanmakuState::default())
        .manage(StreamProxyState::default())
        .manage(LyricsWindowState::default())
        .manage(LyricsAppState::default())
        .invoke_handler(tauri::generate_handler![
            stream_open,
            stream_read,
            stream_close,
            subtitle_search,
            subtitle_download,
            now_playing_snapshot,
            lyrics_search,
            get_app_info,
            open_url,
            livestream_decode_manifest,
            livestream_resolve_variant,
            danmaku_fetch_image,
            danmaku_connect,
            danmaku_disconnect,
            danmaku_set_msg_subscription,
            open_chat_window,
            open_overlay_window,
            lyrics_settings_get,
            lyrics_settings_set,
            lyrics_detection_set_enabled,
            lyrics_get_current,
            lyrics_set_current,
            open_lyrics_chat_window,
            open_lyrics_overlay_window,
            open_lyrics_dock_window,
            open_lyrics_float_window,
            open_player_window,
            system_accent_rgb,
            set_backdrop
        ])
        .run(tauri::generate_context!())
        .expect("tauri run");
}

#[cfg(test)]
mod danmaku_subscription_tests {
    use super::*;

    #[test]
    fn update_msg_subscription_suppresses_main_when_aux_open() {
        let mut subs: HashSet<String> = HashSet::new();

        update_msg_subscription(&mut subs, "main", true, true);
        assert!(!subs.contains("main"));

        update_msg_subscription(&mut subs, "chat", true, true);
        assert!(subs.contains("chat"));

        update_msg_subscription(&mut subs, "chat", false, true);
        assert!(!subs.contains("chat"));
    }

    #[test]
    fn update_msg_subscription_allows_main_when_no_aux_windows() {
        let mut subs: HashSet<String> = HashSet::new();
        update_msg_subscription(&mut subs, "main", true, false);
        assert!(subs.contains("main"));
    }
}
