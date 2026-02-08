#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Duration;

use chaos_core::danmaku::client::DanmakuClient;
use chaos_core::danmaku::model::{ConnectOptions, DanmakuSession};
use chaos_core::subtitle;
use chaos_core::subtitle::models::ThunderSubtitleItem;
use tauri::{AppHandle, Emitter, Manager, State};

mod danmaku_ui;

const HOMEPAGE: &str = "https://github.com/ZeroDevi1/Chaos-Seed";

struct ActiveDanmaku {
    session: DanmakuSession,
    reader_task: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
struct DanmakuState {
    active: Mutex<Option<ActiveDanmaku>>,
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
    let Some(host) = u.host_str() else { return true };
    let h = host.to_lowercase();
    if h == "localhost" {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
        };
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_local_or_private_host;

    fn parse(url: &str) -> url::Url {
        url::Url::parse(url).expect("valid url")
    }

    #[test]
    fn blocks_localhost_and_private_ipv4() {
        assert!(is_local_or_private_host(&parse("http://localhost:8080/a.png")));
        assert!(is_local_or_private_host(&parse("http://127.0.0.1/a.png")));
        assert!(is_local_or_private_host(&parse("http://192.168.1.10/a.png")));
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
        assert!(!is_local_or_private_host(&parse("https://example.com/a.png")));
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
            // Map core events to UI-friendly messages.
            for msg in danmaku_ui::map_event_to_ui(ev) {
                let _ = emit_to_known_windows(&app2, "danmaku_msg", msg);
            }
        }
        let _ = emit_to_known_windows(&app2, "danmaku_status", "已断开");
    });

    *state.active.lock().expect("danmaku mutex") = Some(ActiveDanmaku {
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

fn ensure_window(app: &AppHandle, label: &str) -> Option<tauri::WebviewWindow> {
    let w = app.get_webview_window(label)?;
    let _ = w.show();
    let _ = w.set_focus();
    Some(w)
}

#[tauri::command]
fn open_chat_window(app: AppHandle) -> Result<(), String> {
    if ensure_window(&app, "chat").is_some() {
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "chat",
        tauri::WebviewUrl::App("index.html?view=chat".into()),
    )
        .title("弹幕 - Chat")
        .inner_size(420.0, 640.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_overlay_window(app: AppHandle, opaque: Option<bool>) -> Result<(), String> {
    if ensure_window(&app, "overlay").is_some() {
        return Ok(());
    }
    let opaque = opaque.unwrap_or(false);
    let url = if opaque {
        "index.html?view=overlay&overlay=opaque"
    } else {
        "index.html?view=overlay"
    };

    tauri::WebviewWindowBuilder::new(&app, "overlay", tauri::WebviewUrl::App(url.into()))
        .title("弹幕 - Overlay")
        .inner_size(960.0, 320.0)
        .resizable(true)
        .decorations(false)
        .transparent(!opaque)
        .always_on_top(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(DanmakuState::default())
        .invoke_handler(tauri::generate_handler![
            subtitle_search,
            subtitle_download,
            get_app_info,
            open_url,
            danmaku_fetch_image,
            danmaku_connect,
            danmaku_disconnect,
            open_chat_window,
            open_overlay_window
        ])
        .run(tauri::generate_context!())
        .expect("tauri run");
}
