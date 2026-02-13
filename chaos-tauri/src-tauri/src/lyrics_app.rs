use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex as AsyncMutex;

use chaos_core::{lyrics, now_playing};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsDefaultDisplay {
    MainOnly,
    Dock,
    Float,
    DockAndFloat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsBackgroundEffect {
    None,
    Fluid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsLayoutEffect {
    None,
    Fan3d,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsParticleEffect {
    None,
    Snow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsEffectsSettings {
    pub background_effect: LyricsBackgroundEffect,
    pub layout_effect: LyricsLayoutEffect,
    pub particle_effect: LyricsParticleEffect,
}

impl Default for LyricsEffectsSettings {
    fn default() -> Self {
        Self {
            background_effect: LyricsBackgroundEffect::Fluid,
            layout_effect: LyricsLayoutEffect::Fan3d,
            particle_effect: LyricsParticleEffect::Snow,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsAppSettings {
    pub lyrics_detection_enabled: bool,
    pub auto_hide_on_pause: bool,
    pub auto_hide_delay_ms: u64,
    pub matching_threshold: u8,
    pub timeout_ms: u64,
    pub limit: usize,
    pub providers_order: Vec<lyrics::model::LyricsService>,
    pub default_display: LyricsDefaultDisplay,
    pub effects: LyricsEffectsSettings,
}

impl Default for LyricsAppSettings {
    fn default() -> Self {
        Self {
            lyrics_detection_enabled: false,
            auto_hide_on_pause: true,
            auto_hide_delay_ms: 800,
            matching_threshold: 40,
            timeout_ms: 8000,
            limit: 10,
            providers_order: vec![
                lyrics::model::LyricsService::QQMusic,
                lyrics::model::LyricsService::Netease,
                lyrics::model::LyricsService::LrcLib,
            ],
            default_display: LyricsDefaultDisplay::MainOnly,
            effects: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LyricsAppSettingsPatch {
    pub lyrics_detection_enabled: Option<bool>,
    pub auto_hide_on_pause: Option<bool>,
    pub auto_hide_delay_ms: Option<u64>,
    pub matching_threshold: Option<u8>,
    pub timeout_ms: Option<u64>,
    pub limit: Option<usize>,
    pub providers_order: Option<Vec<lyrics::model::LyricsService>>,
    pub default_display: Option<LyricsDefaultDisplay>,
    pub effects: Option<LyricsEffectsSettings>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NowPlayingStatePayload {
    pub supported: bool,
    pub app_id: Option<String>,
    pub playback_status: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_title: Option<String>,
    pub position_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub retrieved_at_unix_ms: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song_id: Option<String>,
}

impl Default for NowPlayingStatePayload {
    fn default() -> Self {
        Self {
            supported: false,
            app_id: None,
            playback_status: None,
            title: None,
            artist: None,
            album_title: None,
            position_ms: None,
            duration_ms: None,
            retrieved_at_unix_ms: 0,
            genres: Vec::new(),
            song_id: None,
        }
    }
}

fn make_now_playing_payload(snap: &now_playing::NowPlayingSnapshot) -> NowPlayingStatePayload {
    let np = snap.now_playing.as_ref();
    NowPlayingStatePayload {
        supported: snap.supported,
        app_id: np.map(|x| x.app_id.clone()),
        playback_status: np.map(|x| x.playback_status.clone()),
        title: np.and_then(|x| x.title.clone()),
        artist: np.and_then(|x| x.artist.clone()),
        album_title: np.and_then(|x| x.album_title.clone()),
        position_ms: np.and_then(|x| x.position_ms),
        duration_ms: np.and_then(|x| x.duration_ms),
        retrieved_at_unix_ms: snap.retrieved_at_unix_ms,
        genres: np.map(|x| x.genres.clone()).unwrap_or_default(),
        song_id: np.and_then(|x| x.song_id.clone()),
    }
}

fn payload_key(p: &NowPlayingStatePayload) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        p.app_id.as_deref().unwrap_or(""),
        p.title.as_deref().unwrap_or(""),
        p.artist.as_deref().unwrap_or(""),
        p.album_title.as_deref().unwrap_or(""),
        p.duration_ms.unwrap_or(0)
    )
}

#[derive(Default)]
pub struct LyricsAppState {
    pub settings: Mutex<LyricsAppSettings>,
    pub detection_enabled: AtomicBool,

    pub dock_open: AtomicBool,
    pub float_open: AtomicBool,

    // Track whether a window was auto-hidden by the "pause" behavior.
    pub auto_hidden: Mutex<HashSet<String>>,

    pub last_now_payload: Mutex<NowPlayingStatePayload>,
    pub last_song_key: Mutex<Option<String>>,

    pub search_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
}

impl LyricsAppState {
    pub fn apply_settings_patch(&self, patch: LyricsAppSettingsPatch) -> LyricsAppSettings {
        let mut s = self.settings.lock().expect("lyrics settings mutex");
        if let Some(v) = patch.lyrics_detection_enabled {
            s.lyrics_detection_enabled = v;
            self.detection_enabled.store(v, Ordering::Relaxed);
        }
        if let Some(v) = patch.auto_hide_on_pause {
            s.auto_hide_on_pause = v;
        }
        if let Some(v) = patch.auto_hide_delay_ms {
            s.auto_hide_delay_ms = v;
        }
        if let Some(v) = patch.matching_threshold {
            s.matching_threshold = v;
        }
        if let Some(v) = patch.timeout_ms {
            s.timeout_ms = v.max(1);
        }
        if let Some(v) = patch.limit {
            s.limit = v.clamp(1, 50);
        }
        if let Some(v) = patch.providers_order {
            if !v.is_empty() {
                s.providers_order = v;
            }
        }
        if let Some(v) = patch.default_display {
            s.default_display = v;
        }
        if let Some(v) = patch.effects {
            s.effects = v;
        }
        s.clone()
    }
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("lyrics_settings.json"))
}

pub fn load_settings(app: &AppHandle) -> Result<LyricsAppSettings, String> {
    let path = settings_path(app)?;
    let bytes = match std::fs::read(&path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LyricsAppSettings::default());
        }
        Err(e) => return Err(e.to_string()),
    };
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub fn save_settings(app: &AppHandle, settings: &LyricsAppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    // Windows `rename` behavior can vary if the destination exists; remove first (best-effort).
    let _ = std::fs::remove_file(&path);
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn emit_detection_state(app: &AppHandle, enabled: bool) {
    let _ = app.emit(
        "lyrics_detection_state_changed",
        serde_json::json!({ "enabled": enabled }),
    );
}

pub fn emit_now_playing_state(app: &AppHandle, payload: &NowPlayingStatePayload) {
    let _ = app.emit("now_playing_state_changed", payload);
}

pub async fn start_watch_loop(app: AppHandle) {
    let st = app.state::<LyricsAppState>();

    // Refresh immediately on start, then adaptively.
    loop {
        let enabled = st.detection_enabled.load(Ordering::Relaxed);
        let dock = st.dock_open.load(Ordering::Relaxed);
        let float = st.float_open.load(Ordering::Relaxed);
        let want_timeline = dock || float;

        if !enabled && !want_timeline {
            tokio::time::sleep(Duration::from_millis(800)).await;
            continue;
        }

        let snap = tokio::task::spawn_blocking(|| {
            now_playing::snapshot(now_playing::NowPlayingOptions {
                include_thumbnail: false,
                max_thumbnail_bytes: 1,
                max_sessions: 32,
            })
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_else(|| now_playing::NowPlayingSnapshot {
            supported: false,
            now_playing: None,
            sessions: Vec::new(),
            picked_app_id: None,
            retrieved_at_unix_ms: 0,
        });

        let payload = make_now_playing_payload(&snap);
        let is_playing = payload
            .playback_status
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("Playing"))
            .unwrap_or(false);

        let mut should_emit = false;
        {
            let mut prev = st
                .last_now_payload
                .lock()
                .expect("lyrics last_now_payload mutex");
            let key_changed = payload_key(&payload) != payload_key(&prev);
            let status_changed = payload.playback_status != prev.playback_status;
            let timeline_changed = payload.duration_ms != prev.duration_ms
                || (want_timeline && payload.position_ms != prev.position_ms);

            if key_changed || status_changed || timeline_changed {
                should_emit = true;
                *prev = payload.clone();
            }
        }

        if should_emit {
            emit_now_playing_state(&app, &payload);
            handle_auto_hide(&app, &payload).await;
            handle_auto_search(&app, &payload).await;
        } else if want_timeline && is_playing {
            // Even if the key is stable, refresh occasionally so the UI can re-sync drift.
            emit_now_playing_state(&app, &payload);
        }

        let sleep_ms = if want_timeline && is_playing {
            2000
        } else if enabled && is_playing {
            4000
        } else if enabled {
            8000
        } else {
            3000
        };
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }
}

async fn handle_auto_hide(app: &AppHandle, payload: &NowPlayingStatePayload) {
    let st = app.state::<LyricsAppState>();
    let s = st.settings.lock().expect("lyrics settings mutex").clone();
    if !s.auto_hide_on_pause {
        return;
    }

    let status = payload
        .playback_status
        .as_deref()
        .unwrap_or("Unknown")
        .to_string();

    let paused = !status.eq_ignore_ascii_case("Playing");
    let delay = Duration::from_millis(s.auto_hide_delay_ms.max(50));

    let dock_label = "lyrics_dock".to_string();
    let float_label = "lyrics_float".to_string();

    if paused {
        tokio::time::sleep(delay).await;
        let cur = st
            .last_now_payload
            .lock()
            .expect("lyrics last_now_payload mutex")
            .clone();
        let still_paused = cur
            .playback_status
            .as_deref()
            .map(|x| !x.eq_ignore_ascii_case("Playing"))
            .unwrap_or(true);
        if !still_paused {
            return;
        }

        for label in [dock_label, float_label] {
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.hide();
                let mut ah = st.auto_hidden.lock().expect("lyrics auto_hidden mutex");
                ah.insert(label);
            }
        }
    } else {
        let mut ah = st.auto_hidden.lock().expect("lyrics auto_hidden mutex");
        for label in [dock_label, float_label] {
            if ah.remove(&label) {
                if let Some(w) = app.get_webview_window(&label) {
                    let _ = w.show();
                }
            }
        }
    }
}

async fn handle_auto_search(app: &AppHandle, payload: &NowPlayingStatePayload) {
    let st = app.state::<LyricsAppState>();
    if !st.detection_enabled.load(Ordering::Relaxed) {
        return;
    }
    if !payload.supported {
        return;
    }
    let title = payload.title.as_deref().unwrap_or("").trim().to_string();
    if title.is_empty() {
        return;
    }

    let song_key = payload_key(payload);
    {
        let mut prev = st.last_song_key.lock().expect("lyrics last_song_key mutex");
        if prev.as_deref() == Some(&song_key) {
            return;
        }
        *prev = Some(song_key.clone());
    }

    // Cancel previous search task if it is still running.
    {
        let mut g = st.search_task.lock().await;
        if let Some(task) = g.take() {
            task.abort();
        }
    }

    let app2 = app.clone();
    let payload2 = payload.clone();
    let task = tokio::spawn(async move {
        run_sequential_search(&app2, &payload2).await;
    });
    *st.search_task.lock().await = Some(task);
}

async fn run_sequential_search(app: &AppHandle, payload: &NowPlayingStatePayload) {
    let st = app.state::<LyricsAppState>();
    let s = st.settings.lock().expect("lyrics settings mutex").clone();

    let artist = payload.artist.as_deref().unwrap_or("").trim().to_string();
    let album = payload
        .album_title
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    let term = if !artist.is_empty() {
        lyrics::model::LyricsSearchTerm::Info {
            title: payload.title.clone().unwrap_or_default(),
            artist,
            album: (!album.is_empty()).then_some(album),
        }
    } else {
        lyrics::model::LyricsSearchTerm::Keyword {
            keyword: payload.title.clone().unwrap_or_default(),
        }
    };

    let mut req = lyrics::model::LyricsSearchRequest::new(term);
    req.duration_ms = payload.duration_ms.filter(|v| *v > 0);
    req.limit = s.limit.clamp(1, 50);

    let timeout_ms = s.timeout_ms.max(1);
    let threshold = s.matching_threshold;

    for service in s.providers_order.iter().copied() {
        let mut opt = lyrics::model::LyricsSearchOptions::default();
        opt.timeout_ms = timeout_ms;
        opt.strict_match = false;
        opt.services = vec![service];

        let results = match lyrics::core::search(&req, opt).await {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };

        let best = results.into_iter().max_by_key(|r| r.match_percentage);
        let Some(best) = best else {
            continue;
        };
        if best.match_percentage >= threshold {
            let _ = app.emit("lyrics_current_changed", best.clone());
            // Persist as the global "current lyrics" so all windows can read.
            let state = app.state::<crate::LyricsWindowState>();
            *state.current.lock().expect("lyrics window state mutex") = Some(best);
            return;
        }
    }

    // Not found: clear current.
    let state = app.state::<crate::LyricsWindowState>();
    *state.current.lock().expect("lyrics window state mutex") = None;
    let _ = app.emit("lyrics_current_changed", serde_json::Value::Null);
}
