#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;
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
}

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
    let Some(host) = u.host_str() else {
        return true;
    };
    let h = host.to_lowercase();
    if h == "localhost" {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
            }
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
            // This keeps background / hidden pages from doing unnecessary DOM work.
            let subs: Vec<String> = {
                let st = app2.state::<DanmakuState>();
                st.msg_subscribers
                    .lock()
                    .expect("danmaku subscribers mutex")
                    .iter()
                    .cloned()
                    .collect()
            };

            for msg in danmaku_ui::map_event_to_ui(ev) {
                for label in &subs {
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

#[tauri::command]
fn danmaku_set_msg_subscription(
    window: tauri::Window,
    state: State<'_, DanmakuState>,
    enabled: bool,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut subs = state
        .msg_subscribers
        .lock()
        .expect("danmaku subscribers mutex");
    if enabled {
        subs.insert(label);
    } else {
        subs.remove(&label);
    }
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

#[tauri::command]
async fn open_chat_window(app: AppHandle) -> Result<(), String> {
    if ensure_window(&app, "chat").is_some() {
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
    {
        let app2 = app.clone();
        let label = "chat".to_string();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let st = app2.state::<DanmakuState>();
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
    {
        let app2 = app.clone();
        let label = "overlay".to_string();
        w.on_window_event(move |ev| {
            if matches!(ev, tauri::WindowEvent::Destroyed) {
                let st = app2.state::<DanmakuState>();
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
fn set_backdrop(app: AppHandle, mode: String) -> Result<(), String> {
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
            // Enable Mica by default on Windows for a more native Win11 look.
            // The frontend can later call `set_backdrop` to switch to `none` without restarting.
            #[cfg(target_os = "windows")]
            {
                use tauri::window::{Effect, EffectsBuilder};
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_effects(EffectsBuilder::new().effect(Effect::Mica).build());
                }
            }
            Ok(())
        })
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
            danmaku_set_msg_subscription,
            open_chat_window,
            open_overlay_window,
            set_backdrop
        ])
        .run(tauri::generate_context!())
        .expect("tauri run");
}
