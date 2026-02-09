use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::ptr;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use libc::{c_char, c_void};

use chaos_core::{danmaku, livestream, now_playing, subtitle};

const API_VERSION: u32 = 2;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(message: impl Into<String>, context: Option<String>) {
    #[derive(serde::Serialize)]
    struct ErrJson {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    }
    let err = ErrJson {
        message: message.into(),
        context,
    };
    let s = serde_json::to_string(&err).unwrap_or_else(|_| "{\"message\":\"error\"}".to_string());
    let c = CString::new(s).unwrap_or_else(|_| CString::new("{\"message\":\"error\"}").unwrap());
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(c));
}

fn take_last_error() -> Option<CString> {
    LAST_ERROR.with(|e| e.borrow_mut().take())
}

fn ok_json(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => {
            set_last_error("invalid utf-8/embedded NUL", None);
            ptr::null_mut()
        }
    }
}

fn require_cstr<'a>(p: *const c_char, name: &'static str) -> Result<&'a str, ()> {
    if p.is_null() {
        set_last_error(format!("{name} is null"), None);
        return Err(());
    }
    let s = unsafe { CStr::from_ptr(p) };
    match s.to_str() {
        Ok(v) => Ok(v),
        Err(_) => {
            set_last_error(format!("{name} is not valid utf-8"), None);
            Err(())
        }
    }
}

fn optional_cstr<'a>(p: *const c_char, name: &'static str) -> Result<Option<&'a str>, ()> {
    if p.is_null() {
        return Ok(None);
    }
    Ok(Some(require_cstr(p, name)?))
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn livestream_http() -> &'static reqwest::Client {
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("chaos-seed/0.1")
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client")
    })
}

fn livestream_cfg() -> &'static livestream::LivestreamConfig {
    static CFG: OnceLock<livestream::LivestreamConfig> = OnceLock::new();
    CFG.get_or_init(livestream::LivestreamConfig::default)
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_ffi_api_version() -> u32 {
    API_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_ffi_version_json() -> *mut c_char {
    #[derive(serde::Serialize)]
    struct Ver<'a> {
        version: &'a str,
        git: &'a str,
        api: u32,
    }
    let git = option_env!("CHAOS_GIT_HASH").unwrap_or("unknown");
    let v = Ver {
        version: env!("CARGO_PKG_VERSION"),
        git,
        api: API_VERSION,
    };
    match serde_json::to_string(&v) {
        Ok(s) => ok_json(s),
        Err(e) => {
            set_last_error("failed to serialize version", Some(e.to_string()));
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_ffi_last_error_json() -> *mut c_char {
    match take_last_error() {
        Some(c) => c.into_raw(),
        None => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_ffi_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_now_playing_snapshot_json(
    include_thumbnail: u8,
    max_thumbnail_bytes: u32,
    max_sessions: u32,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let opt = now_playing::NowPlayingOptions {
            include_thumbnail: include_thumbnail != 0,
            max_thumbnail_bytes: (max_thumbnail_bytes as usize).max(1),
            max_sessions: (max_sessions as usize).max(1),
        };

        let snap = now_playing::snapshot(opt).map_err(|e| {
            set_last_error("now playing snapshot failed", Some(e.to_string()));
        })?;

        serde_json::to_string(&snap).map_err(|e| {
            set_last_error(
                "failed to serialize now playing snapshot",
                Some(e.to_string()),
            );
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_now_playing_snapshot_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_subtitle_search_json(
    query_utf8: *const c_char,
    limit: u32,
    min_score_or_neg1: f64,
    lang_utf8_or_null: *const c_char,
    timeout_ms: u32,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let query = require_cstr(query_utf8, "query_utf8")?.trim().to_string();
        if query.is_empty() {
            set_last_error("query_utf8 is empty", None);
            return Err(());
        }

        let lang = optional_cstr(lang_utf8_or_null, "lang_utf8_or_null")?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let min_score = if min_score_or_neg1.is_sign_negative() {
            None
        } else {
            Some(min_score_or_neg1)
        };

        let timeout = Duration::from_millis(timeout_ms.max(1) as u64);
        let items = runtime()
            .block_on(subtitle::core::search_items(
                &query,
                limit.max(1) as usize,
                min_score,
                lang.as_deref(),
                timeout,
            ))
            .map_err(|e| {
                set_last_error("subtitle search failed", Some(e.to_string()));
            })?;

        serde_json::to_string(&items).map_err(|e| {
            set_last_error("failed to serialize items", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_subtitle_search_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_subtitle_download_item_json(
    item_json_utf8: *const c_char,
    out_dir_utf8: *const c_char,
    timeout_ms: u32,
    retries: u32,
    overwrite: u8,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let item_json = require_cstr(item_json_utf8, "item_json_utf8")?;
        let out_dir = require_cstr(out_dir_utf8, "out_dir_utf8")?;

        let item: subtitle::models::ThunderSubtitleItem =
            serde_json::from_str(item_json).map_err(|e| {
                set_last_error("invalid item_json_utf8", Some(e.to_string()));
            })?;

        let out_dir: PathBuf = out_dir.into();
        let timeout = Duration::from_millis(timeout_ms.max(1) as u64);
        let path = runtime()
            .block_on(subtitle::core::download_item(
                &item,
                &out_dir,
                timeout,
                retries,
                overwrite != 0,
            ))
            .map_err(|e| {
                set_last_error("subtitle download failed", Some(e.to_string()));
            })?;

        #[derive(serde::Serialize)]
        struct Reply {
            path: String,
            bytes: u64,
        }
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let reply = Reply {
            path: path.to_string_lossy().to_string(),
            bytes,
        };
        serde_json::to_string(&reply).map_err(|e| {
            set_last_error("failed to serialize download reply", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_subtitle_download_item_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_livestream_decode_manifest_json(
    input_utf8: *const c_char,
    drop_inaccessible_high_qualities: u8,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let input = require_cstr(input_utf8, "input_utf8")?.trim().to_string();
        if input.is_empty() {
            set_last_error("input_utf8 is empty", None);
            return Err(());
        }

        let (site, room_id) =
            chaos_core::danmaku::sites::parse_target_hint(&input).map_err(|e| {
                set_last_error("invalid input_utf8", Some(e.to_string()));
            })?;

        let opt = livestream::ResolveOptions {
            drop_inaccessible_high_qualities: drop_inaccessible_high_qualities != 0,
        };

        let man = runtime()
            .block_on(livestream::platforms::decode_manifest(
                livestream_http(),
                livestream_cfg(),
                site,
                &room_id,
                &input,
                opt,
            ))
            .map_err(|e| {
                set_last_error("livestream decode failed", Some(e.to_string()));
            })?;

        serde_json::to_string(&man).map_err(|e| {
            set_last_error("failed to serialize manifest", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_livestream_decode_manifest_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_livestream_resolve_variant_json(
    input_utf8: *const c_char,
    variant_id_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let input = require_cstr(input_utf8, "input_utf8")?.trim().to_string();
        if input.is_empty() {
            set_last_error("input_utf8 is empty", None);
            return Err(());
        }
        let variant_id = require_cstr(variant_id_utf8, "variant_id_utf8")?
            .trim()
            .to_string();
        if variant_id.is_empty() {
            set_last_error("variant_id_utf8 is empty", None);
            return Err(());
        }

        let (site, room_id) =
            chaos_core::danmaku::sites::parse_target_hint(&input).map_err(|e| {
                set_last_error("invalid input_utf8", Some(e.to_string()));
            })?;

        let v = runtime()
            .block_on(livestream::platforms::resolve_variant(
                livestream_http(),
                livestream_cfg(),
                site,
                &room_id,
                &variant_id,
            ))
            .map_err(|e| {
                set_last_error("livestream resolve_variant failed", Some(e.to_string()));
            })?;

        serde_json::to_string(&v).map_err(|e| {
            set_last_error("failed to serialize variant", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_livestream_resolve_variant_json", None);
            ptr::null_mut()
        }
    }
}

pub type ChaosDanmakuCallback =
    Option<extern "C" fn(event_json_utf8: *const c_char, user_data: *mut c_void)>;

struct DanmakuHandle {
    disposed: Arc<AtomicBool>,
    queue: Arc<Mutex<VecDeque<danmaku::model::DanmakuEvent>>>,
    // Store user_data as usize so this struct can be shared across threads safely.
    callback: Arc<Mutex<(ChaosDanmakuCallback, usize)>>,
    session: Mutex<Option<danmaku::model::DanmakuSession>>,
    forwarder: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl DanmakuHandle {
    fn new(
        session: danmaku::model::DanmakuSession,
        mut rx: danmaku::model::DanmakuEventRx,
    ) -> Self {
        let queue: Arc<Mutex<VecDeque<danmaku::model::DanmakuEvent>>> =
            Arc::new(Mutex::new(VecDeque::new()));
        let callback: Arc<Mutex<(ChaosDanmakuCallback, usize)>> = Arc::new(Mutex::new((None, 0)));
        let disposed: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

        let queue2 = Arc::clone(&queue);
        let cb2 = Arc::clone(&callback);
        let disposed_thread = Arc::clone(&disposed);

        // Dispatch loop runs on a normal thread, not inside tokio runtime.
        let t = std::thread::spawn(move || {
            const MAX_QUEUE: usize = 2000;
            loop {
                if disposed_thread.load(Ordering::Relaxed) {
                    break;
                }
                let ev = runtime().block_on(rx.recv());
                let Some(ev) = ev else { break };

                {
                    let mut q = queue2.lock().unwrap();
                    q.push_back(ev.clone());
                    while q.len() > MAX_QUEUE {
                        q.pop_front();
                    }
                }

                let (cb, ud) = {
                    let g = cb2.lock().unwrap();
                    (g.0, g.1)
                };
                if let Some(cb) = cb {
                    if let Ok(s) = serde_json::to_string(&ev) {
                        if let Ok(cs) = CString::new(s) {
                            let _ = std::panic::catch_unwind(|| {
                                cb(cs.as_ptr(), ud as *mut c_void);
                            });
                        }
                    }
                }
            }
        });

        Self {
            disposed,
            queue,
            callback,
            session: Mutex::new(Some(session)),
            forwarder: Mutex::new(Some(t)),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_danmaku_connect(input_utf8: *const c_char) -> *mut c_void {
    let res = std::panic::catch_unwind(|| -> Result<*mut c_void, ()> {
        let input = require_cstr(input_utf8, "input_utf8")?.to_string();
        if input.trim().is_empty() {
            set_last_error("input_utf8 is empty", None);
            return Err(());
        }

        let client = danmaku::client::DanmakuClient::new().map_err(|e| {
            set_last_error("failed to create danmaku client", Some(e.to_string()));
        })?;

        let (session, rx) = runtime()
            .block_on(client.connect(&input, danmaku::model::ConnectOptions::default()))
            .map_err(|e| {
                set_last_error("danmaku connect failed", Some(e.to_string()));
            })?;

        let h = Box::new(DanmakuHandle::new(session, rx));
        Ok(Box::into_raw(h) as *mut c_void)
    });

    match res {
        Ok(Ok(p)) => p,
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_danmaku_connect", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_danmaku_set_callback(
    handle: *mut c_void,
    cb: ChaosDanmakuCallback,
    user_data: *mut c_void,
) -> i32 {
    let res = std::panic::catch_unwind(|| -> Result<i32, ()> {
        if handle.is_null() {
            set_last_error("handle is null", None);
            return Err(());
        }
        let h = unsafe { &*(handle as *mut DanmakuHandle) };
        let mut g = h.callback.lock().unwrap();
        *g = (cb, user_data as usize);
        Ok(0)
    });
    match res {
        Ok(Ok(v)) => v,
        Ok(Err(())) => -1,
        Err(_) => {
            set_last_error("panic in chaos_danmaku_set_callback", None);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_danmaku_poll_json(handle: *mut c_void, max_events: u32) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        if handle.is_null() {
            set_last_error("handle is null", None);
            return Err(());
        }
        let h = unsafe { &*(handle as *mut DanmakuHandle) };
        let n = if max_events == 0 {
            50
        } else {
            max_events as usize
        };
        let mut out = Vec::new();
        {
            let mut q = h.queue.lock().unwrap();
            for _ in 0..n {
                let Some(ev) = q.pop_front() else { break };
                out.push(ev);
            }
        }
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize events", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_danmaku_poll_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_danmaku_disconnect(handle: *mut c_void) -> i32 {
    let res = std::panic::catch_unwind(|| -> Result<i32, ()> {
        if handle.is_null() {
            set_last_error("handle is null", None);
            return Err(());
        }
        let h = unsafe { Box::from_raw(handle as *mut DanmakuHandle) };
        h.disposed.store(true, Ordering::Relaxed);

        // Stop the core session first so the receiver closes.
        if let Some(sess) = h.session.lock().unwrap().take() {
            runtime().block_on(sess.stop());
        }

        // Now join the forwarder thread to guarantee no more callbacks after return.
        if let Some(t) = h.forwarder.lock().unwrap().take() {
            let _ = t.join();
        }

        Ok(0)
    });
    match res {
        Ok(Ok(v)) => v,
        Ok(Err(())) => -1,
        Err(_) => {
            set_last_error("panic in chaos_danmaku_disconnect", None);
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn version_json_is_valid() {
        let p = chaos_ffi_version_json();
        assert!(!p.is_null());
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
        chaos_ffi_string_free(p);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["api"].as_u64().unwrap(), API_VERSION as u64);
    }

    #[test]
    fn last_error_is_null_when_empty() {
        let p = chaos_ffi_last_error_json();
        assert!(p.is_null());
    }

    #[test]
    fn subtitle_search_rejects_null_query() {
        let p = chaos_subtitle_search_json(ptr::null(), 10, -1.0, ptr::null(), 1000);
        assert!(p.is_null());
        let err = chaos_ffi_last_error_json();
        assert!(!err.is_null());
        chaos_ffi_string_free(err);
    }

    #[test]
    fn subtitle_download_rejects_bad_json() {
        let item = c("{not json}");
        let out = c("/tmp");
        let p = chaos_subtitle_download_item_json(item.as_ptr(), out.as_ptr(), 1000, 0, 0);
        assert!(p.is_null());
        let err = chaos_ffi_last_error_json();
        assert!(!err.is_null());
        chaos_ffi_string_free(err);
    }

    #[test]
    fn danmaku_poll_rejects_null_handle() {
        let p = chaos_danmaku_poll_json(ptr::null_mut(), 10);
        assert!(p.is_null());
        let err = chaos_ffi_last_error_json();
        assert!(!err.is_null());
        chaos_ffi_string_free(err);
    }

    #[test]
    fn livestream_decode_rejects_null_input() {
        let p = chaos_livestream_decode_manifest_json(ptr::null(), 1);
        assert!(p.is_null());
        let err = chaos_ffi_last_error_json();
        assert!(!err.is_null());
        chaos_ffi_string_free(err);
    }

    #[test]
    fn livestream_resolve_rejects_null_args() {
        let p = chaos_livestream_resolve_variant_json(ptr::null(), ptr::null());
        assert!(p.is_null());
        let err = chaos_ffi_last_error_json();
        assert!(!err.is_null());
        chaos_ffi_string_free(err);
    }

    #[test]
    fn now_playing_snapshot_returns_json_payload() {
        let p = chaos_now_playing_snapshot_json(0, 64, 8);
        assert!(!p.is_null());
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
        chaos_ffi_string_free(p);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("supported").is_some());
        assert!(v.get("sessions").is_some());
    }
}
