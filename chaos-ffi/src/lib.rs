use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::ptr;
use std::str::FromStr;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use libc::{c_char, c_void};

use chaos_core::live_directory;
use chaos_core::{bili_video, danmaku, livestream, lyrics, music, now_playing, subtitle};
use chaos_proto::{
    // music (FFI JSON shape follows chaos-proto)
    KugouUserInfo,
    LiveDirCategory,
    LiveDirRoomCard,
    LiveDirRoomListResult,
    LiveDirSubCategory,
    MusicAlbum,
    MusicAlbumTracksParams,
    MusicArtist,
    MusicArtistAlbumsParams,
    MusicAuthState,
    MusicDownloadJobResult,
    MusicDownloadStartParams,
    MusicDownloadStartResult,
    MusicDownloadStatus,
    MusicDownloadTarget,
    MusicDownloadTotals,
    MusicJobState,
    MusicLoginQr,
    MusicLoginQrPollResult,
    MusicLoginQrState,
    MusicLoginType,
    MusicProviderConfig,
    MusicSearchParams,
    MusicService,
    MusicTrack,
    MusicTrackPlayUrlParams,
    MusicTrackPlayUrlResult,
    OkReply,
    QqMusicCookie,
    // bili
    BiliApiType,
    BiliAuthBundle,
    BiliTvAuth,
    BiliWebAuth,
    BiliAuthState,
    BiliCheckLoginParams,
    BiliCheckLoginResult,
    BiliDownloadJobStatus,
    BiliDownloadStartParams,
    BiliDownloadStartResult,
    BiliDownloadStatus,
    BiliDownloadTotals,
    BiliJobPhase,
    BiliJobState,
    BiliLoginQr,
    BiliLoginQrCreateV2Params,
    BiliLoginQrPollResult,
    BiliLoginQrPollResultV2,
    BiliLoginQrState,
    BiliLoginType,
    BiliPage,
    BiliParseParams,
    BiliParseResult,
    BiliParsedVideo,
    BiliRefreshCookieParams,
    BiliRefreshCookieResult,
    BiliTask,
    BiliTaskAddParams,
    BiliTaskAddResult,
    BiliTaskCancelParams,
    BiliTaskDetail,
    BiliTaskGetParams,
    BiliTasksGetParams,
    BiliTasksGetResult,
    BiliTasksRemoveFinishedParams,
    // tts
    TtsAudioResult,
    TtsJobState,
    TtsPromptStrategy,
    TtsSftStartParams,
    TtsSftStartResult,
    TtsSftStatus,
};

const API_VERSION: u32 = 9;

fn ensure_rustls_provider() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        // In some dependency graphs multiple rustls CryptoProviders can be enabled; picking one
        // avoids runtime panics. Prefer rustls' default (aws-lc-rs).
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

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

// -----------------------------
// Music (FFI JSON)
// -----------------------------

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_config_set_json(config_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(config_json_utf8, "config_json_utf8")?;
        let cfg: MusicProviderConfig = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid config_json_utf8", Some(e.to_string()));
        })?;
        let cfg = map_music_provider_config_to_core(cfg);

        let st = music_state();
        let mut locked = st.lock().map_err(|_| {
            set_last_error("music state poisoned", None);
        })?;
        locked.cfg = cfg.clone();
        locked.client.set_config(cfg);

        serde_json::to_string(&OkReply { ok: true }).map_err(|e| {
            set_last_error("failed to serialize ok reply", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_config_set_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_search_tracks_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: MusicSearchParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let keyword = params.keyword.trim().to_string();
        if keyword.is_empty() {
            return serde_json::to_string::<Vec<MusicTrack>>(&vec![]).map_err(|e| {
                set_last_error("failed to serialize tracks", Some(e.to_string()));
            });
        }

        let client = {
            let st = music_state();
            st.lock()
                .map_err(|_| {
                    set_last_error("music state poisoned", None);
                })?
                .client
                .clone()
        };

        let out = runtime()
            .block_on(client.search_tracks(
                map_music_service_to_core(params.service),
                &keyword,
                params.page.max(1),
                params.page_size.clamp(1, 50).max(1),
            ))
            .map_err(|e| {
                set_last_error("music search tracks failed", Some(e.to_string()));
            })?;

        let mapped: Vec<MusicTrack> = out.into_iter().map(map_music_track_to_proto).collect();
        serde_json::to_string(&mapped).map_err(|e| {
            set_last_error("failed to serialize tracks", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_search_tracks_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_search_albums_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: MusicSearchParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let keyword = params.keyword.trim().to_string();
        if keyword.is_empty() {
            return serde_json::to_string::<Vec<MusicAlbum>>(&vec![]).map_err(|e| {
                set_last_error("failed to serialize albums", Some(e.to_string()));
            });
        }

        let client = {
            let st = music_state();
            st.lock()
                .map_err(|_| {
                    set_last_error("music state poisoned", None);
                })?
                .client
                .clone()
        };

        let out = runtime()
            .block_on(client.search_albums(
                map_music_service_to_core(params.service),
                &keyword,
                params.page.max(1),
                params.page_size.clamp(1, 50).max(1),
            ))
            .map_err(|e| {
                set_last_error("music search albums failed", Some(e.to_string()));
            })?;

        let mapped: Vec<MusicAlbum> = out.into_iter().map(map_music_album_to_proto).collect();
        serde_json::to_string(&mapped).map_err(|e| {
            set_last_error("failed to serialize albums", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_search_albums_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_search_artists_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: MusicSearchParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let keyword = params.keyword.trim().to_string();
        if keyword.is_empty() {
            return serde_json::to_string::<Vec<MusicArtist>>(&vec![]).map_err(|e| {
                set_last_error("failed to serialize artists", Some(e.to_string()));
            });
        }

        let client = {
            let st = music_state();
            st.lock()
                .map_err(|_| {
                    set_last_error("music state poisoned", None);
                })?
                .client
                .clone()
        };

        let out = runtime()
            .block_on(client.search_artists(
                map_music_service_to_core(params.service),
                &keyword,
                params.page.max(1),
                params.page_size.clamp(1, 50).max(1),
            ))
            .map_err(|e| {
                set_last_error("music search artists failed", Some(e.to_string()));
            })?;

        let mapped: Vec<MusicArtist> = out.into_iter().map(map_music_artist_to_proto).collect();
        serde_json::to_string(&mapped).map_err(|e| {
            set_last_error("failed to serialize artists", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_search_artists_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_album_tracks_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: MusicAlbumTracksParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let client = {
            let st = music_state();
            st.lock()
                .map_err(|_| {
                    set_last_error("music state poisoned", None);
                })?
                .client
                .clone()
        };

        let out = runtime()
            .block_on(client.album_tracks(
                map_music_service_to_core(params.service),
                params.album_id.trim(),
            ))
            .map_err(|e| {
                set_last_error("music albumTracks failed", Some(e.to_string()));
            })?;
        let mapped: Vec<MusicTrack> = out.into_iter().map(map_music_track_to_proto).collect();
        serde_json::to_string(&mapped).map_err(|e| {
            set_last_error("failed to serialize tracks", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_album_tracks_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_artist_albums_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: MusicArtistAlbumsParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let client = {
            let st = music_state();
            st.lock()
                .map_err(|_| {
                    set_last_error("music state poisoned", None);
                })?
                .client
                .clone()
        };

        let out = runtime()
            .block_on(client.artist_albums(
                map_music_service_to_core(params.service),
                params.artist_id.trim(),
            ))
            .map_err(|e| {
                set_last_error("music artistAlbums failed", Some(e.to_string()));
            })?;
        let mapped: Vec<MusicAlbum> = out.into_iter().map(map_music_album_to_proto).collect();
        serde_json::to_string(&mapped).map_err(|e| {
            set_last_error("failed to serialize albums", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_artist_albums_json", None);
            ptr::null_mut()
        }
    }
}

fn parse_login_type(s: &str) -> Result<MusicLoginType, ()> {
    let p = s.trim().to_ascii_lowercase();
    match p.as_str() {
        "qq" => Ok(MusicLoginType::Qq),
        "wechat" | "wx" => Ok(MusicLoginType::Wechat),
        _ => Err(()),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_track_play_url_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: MusicTrackPlayUrlParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let track_id = params.track_id.trim().to_string();
        if track_id.is_empty() {
            set_last_error("trackId is empty", None);
            return Err(());
        }

        let quality_id = params
            .quality_id
            .unwrap_or_else(|| "mp3_128".to_string())
            .trim()
            .to_string();
        if quality_id.is_empty() {
            set_last_error("qualityId is empty", None);
            return Err(());
        }

        let auth = map_music_auth_to_core(params.auth);
        let svc = map_music_service_to_core(params.service);

        let client = {
            let st = music_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.client.clone()
        };

        let (url, ext) = runtime()
            .block_on(async move {
                client
                    .track_download_url(svc, &track_id, &quality_id, &auth)
                    .await
                    .map_err(|e| e.to_string())
            })
            .map_err(|e| {
                set_last_error("music track play url failed", Some(e));
            })?;

        let out = MusicTrackPlayUrlResult { url, ext };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize play url", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_track_play_url_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_qq_login_qr_create_json(
    login_type_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let lt = require_cstr(login_type_utf8, "login_type_utf8")?;
        let login_type = parse_login_type(lt).map_err(|_| {
            set_last_error("invalid login_type_utf8 (expected: qq|wechat)", None);
        })?;

        let http = music::providers::qq_login::new_login_client().map_err(|e| {
            set_last_error("failed to init qq login client", Some(e.to_string()));
        })?;

        let core_lt = match login_type {
            MusicLoginType::Qq => music::model::MusicLoginType::Qq,
            MusicLoginType::Wechat => music::model::MusicLoginType::Wechat,
        };
        let (identifier, mime, bytes) = runtime()
            .block_on(music::providers::qq_login::create_login_qr(&http, core_lt))
            .map_err(|e| {
                set_last_error("qq login qr create failed", Some(e.to_string()));
            })?;

        let session_id = gen_session_id("qqlogin");
        let created_at_unix_ms = now_unix_ms();
        let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);

        {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.qq_sessions.insert(
                session_id.clone(),
                QqLoginSession {
                    created_at_ms: created_at_unix_ms,
                    login_type,
                    identifier: identifier.clone(),
                    http,
                },
            );
        }

        let qr = MusicLoginQr {
            session_id,
            login_type,
            mime,
            base64,
            identifier,
            created_at_unix_ms,
        };
        serde_json::to_string(&qr).map_err(|e| {
            set_last_error("failed to serialize login qr", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_qq_login_qr_create_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_qq_login_qr_poll_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?
            .trim()
            .to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let (login_type, identifier, http, created_at_ms) = {
            let st = music_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            let Some(s) = locked.qq_sessions.get(&sid) else {
                set_last_error("session not found", None);
                return Err(());
            };
            (
                s.login_type,
                s.identifier.clone(),
                s.http.clone(),
                s.created_at_ms,
            )
        };

        if now_unix_ms().saturating_sub(created_at_ms) > 5 * 60 * 1000 {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.qq_sessions.remove(&sid);
            let out = MusicLoginQrPollResult {
                session_id: sid,
                state: MusicLoginQrState::Timeout,
                message: Some("login session timeout".to_string()),
                cookie: None,
                kugou_user: None,
            };
            return serde_json::to_string(&out).map_err(|e| {
                set_last_error("failed to serialize poll result", Some(e.to_string()));
            });
        }

        let core_lt = match login_type {
            MusicLoginType::Qq => music::model::MusicLoginType::Qq,
            MusicLoginType::Wechat => music::model::MusicLoginType::Wechat,
        };
        let (state, msg, sig_or_code, uin) = runtime()
            .block_on(music::providers::qq_login::poll_login_qr(
                &http,
                core_lt,
                &identifier,
            ))
            .map_err(|e| {
                set_last_error("qq login qr poll failed", Some(e.to_string()));
            })?;

        let state_proto = match state {
            music::model::MusicLoginQrState::Scan => MusicLoginQrState::Scan,
            music::model::MusicLoginQrState::Confirm => MusicLoginQrState::Confirm,
            music::model::MusicLoginQrState::Done => MusicLoginQrState::Done,
            music::model::MusicLoginQrState::Timeout => MusicLoginQrState::Timeout,
            music::model::MusicLoginQrState::Refuse => MusicLoginQrState::Refuse,
            music::model::MusicLoginQrState::Other => MusicLoginQrState::Other,
        };

        if state_proto != MusicLoginQrState::Done {
            let out = MusicLoginQrPollResult {
                session_id: sid,
                state: state_proto,
                message: msg,
                cookie: None,
                kugou_user: None,
            };
            return serde_json::to_string(&out).map_err(|e| {
                set_last_error("failed to serialize poll result", Some(e.to_string()));
            });
        }

        let cookie = match login_type {
            MusicLoginType::Qq => {
                let sigx = sig_or_code.ok_or_else(|| {
                    set_last_error("missing ptsigx", None);
                })?;
                let uin = uin.ok_or_else(|| {
                    set_last_error("missing uin", None);
                })?;
                let code = runtime()
                    .block_on(music::providers::qq_login::authorize_qq_and_get_code(
                        &http, &sigx, &uin,
                    ))
                    .map_err(|e| {
                        set_last_error("qq oauth authorize failed", Some(e.to_string()));
                    })?;
                let c = runtime()
                    .block_on(music::providers::qq_login::exchange_code_for_cookie(
                        &http,
                        &code,
                        music::model::MusicLoginType::Qq,
                    ))
                    .map_err(|e| {
                        set_last_error("qq exchange cookie failed", Some(e.to_string()));
                    })?;
                map_music_qq_cookie_to_proto(c)
            }
            MusicLoginType::Wechat => {
                let wx_code = sig_or_code.ok_or_else(|| {
                    set_last_error("missing wx_code", None);
                })?;
                let c = runtime()
                    .block_on(music::providers::qq_login::exchange_code_for_cookie(
                        &http,
                        &wx_code,
                        music::model::MusicLoginType::Wechat,
                    ))
                    .map_err(|e| {
                        set_last_error("wechat exchange cookie failed", Some(e.to_string()));
                    })?;
                map_music_qq_cookie_to_proto(c)
            }
        };

        {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.qq_sessions.remove(&sid);
        }

        let out = MusicLoginQrPollResult {
            session_id: sid,
            state: MusicLoginQrState::Done,
            message: None,
            cookie: Some(cookie),
            kugou_user: None,
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize poll result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_qq_login_qr_poll_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_qq_refresh_cookie_json(
    cookie_json_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(cookie_json_utf8, "cookie_json_utf8")?;
        let cookie: QqMusicCookie = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid cookie_json_utf8", Some(e.to_string()));
        })?;
        let core_cookie = map_music_qq_cookie_to_core(cookie);

        let http = music::providers::qq_login::new_login_client().map_err(|e| {
            set_last_error("failed to init qq login client", Some(e.to_string()));
        })?;
        let out = runtime()
            .block_on(music::providers::qq_login::refresh_cookie(
                &http,
                &core_cookie,
            ))
            .map_err(|e| {
                set_last_error("qq refresh cookie failed", Some(e.to_string()));
            })?;
        let out = map_music_qq_cookie_to_proto(out);

        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize cookie", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_qq_refresh_cookie_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_kugou_login_qr_create_json(
    login_type_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let lt = require_cstr(login_type_utf8, "login_type_utf8")?;
        let login_type = parse_login_type(lt).map_err(|_| {
            set_last_error("invalid login_type_utf8 (expected: qq|wechat)", None);
        })?;

        let (client, cfg) = {
            let st = music_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            (locked.client.clone(), locked.cfg.clone())
        };

        let (identifier, mime, base64) = match login_type {
            MusicLoginType::Qq => {
                let qr = runtime()
                    .block_on(music::providers::kugou::kugou_qr_create(
                        &client.http,
                        &cfg,
                        client.timeout,
                    ))
                    .map_err(|e| {
                        set_last_error("kugou qr create failed", Some(e.to_string()));
                    })?;
                (qr.key, "image/png".to_string(), qr.image_base64)
            }
            MusicLoginType::Wechat => {
                let (uuid, data_uri) = runtime()
                    .block_on(music::providers::kugou::kugou_wx_qr_create(
                        &client.http,
                        &cfg,
                        client.timeout,
                    ))
                    .map_err(|e| {
                        set_last_error("kugou wechat qr create failed", Some(e.to_string()));
                    })?;
                if let Some((meta, b64)) = data_uri.split_once(',') {
                    let mime = meta
                        .strip_prefix("data:")
                        .and_then(|s| s.split(';').next())
                        .unwrap_or("image/jpeg");
                    (uuid, mime.to_string(), b64.to_string())
                } else {
                    (uuid, "image/jpeg".to_string(), data_uri)
                }
            }
        };

        let session_id = gen_session_id("kugoulogin");
        let created_at_unix_ms = now_unix_ms();
        {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.kugou_sessions.insert(
                session_id.clone(),
                KugouLoginSession {
                    created_at_ms: created_at_unix_ms,
                    login_type,
                    identifier: identifier.clone(),
                },
            );
        }

        let qr = MusicLoginQr {
            session_id,
            login_type,
            mime,
            base64,
            identifier,
            created_at_unix_ms,
        };
        serde_json::to_string(&qr).map_err(|e| {
            set_last_error("failed to serialize login qr", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_kugou_login_qr_create_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_kugou_login_qr_poll_json(
    session_id_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?
            .trim()
            .to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let (login_type, identifier, created_at_ms) = {
            let st = music_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            let Some(s) = locked.kugou_sessions.get(&sid) else {
                set_last_error("session not found", None);
                return Err(());
            };
            (s.login_type, s.identifier.clone(), s.created_at_ms)
        };

        if now_unix_ms().saturating_sub(created_at_ms) > 5 * 60 * 1000 {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.kugou_sessions.remove(&sid);
            let out = MusicLoginQrPollResult {
                session_id: sid,
                state: MusicLoginQrState::Timeout,
                message: Some("login session timeout".to_string()),
                cookie: None,
                kugou_user: None,
            };
            return serde_json::to_string(&out).map_err(|e| {
                set_last_error("failed to serialize poll result", Some(e.to_string()));
            });
        }

        let (client, cfg) = {
            let st = music_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            (locked.client.clone(), locked.cfg.clone())
        };

        let user = match login_type {
            MusicLoginType::Qq => runtime()
                .block_on(music::providers::kugou::kugou_qr_poll(
                    &client.http,
                    &cfg,
                    &identifier,
                    client.timeout,
                ))
                .map_err(|e| {
                    set_last_error("kugou qr poll failed", Some(e.to_string()));
                })?,
            MusicLoginType::Wechat => runtime()
                .block_on(music::providers::kugou::kugou_wx_qr_poll(
                    &client.http,
                    &cfg,
                    &identifier,
                    client.timeout,
                ))
                .map_err(|e| {
                    set_last_error("kugou wechat qr poll failed", Some(e.to_string()));
                })?,
        };

        if let Some(u) = user {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.kugou_sessions.remove(&sid);

            let out = MusicLoginQrPollResult {
                session_id: sid,
                state: MusicLoginQrState::Done,
                message: None,
                cookie: None,
                kugou_user: Some(KugouUserInfo {
                    token: u.token,
                    userid: u.userid,
                }),
            };
            return serde_json::to_string(&out).map_err(|e| {
                set_last_error("failed to serialize poll result", Some(e.to_string()));
            });
        }

        let out = MusicLoginQrPollResult {
            session_id: sid,
            state: MusicLoginQrState::Scan,
            message: None,
            cookie: None,
            kugou_user: None,
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize poll result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_kugou_login_qr_poll_json", None);
            ptr::null_mut()
        }
    }
}

async fn try_download_lyrics_for_track(
    http: &reqwest::Client,
    track: &MusicTrack,
    audio_path: &std::path::Path,
    overwrite: bool,
) -> Result<(), String> {
    // Best-effort: lyrics download should not fail the audio download job.
    let title = (track.title.as_str()).trim();
    if title.is_empty() {
        return Ok(());
    }

    let artist = track
        .artists
        .iter()
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" / ");

    let term = if artist.trim().is_empty() {
        lyrics::model::LyricsSearchTerm::Keyword {
            keyword: title.to_string(),
        }
    } else {
        lyrics::model::LyricsSearchTerm::Info {
            title: title.to_string(),
            artist,
            album: track.album.clone().filter(|s| !s.trim().is_empty()),
        }
    };

    let mut req = lyrics::model::LyricsSearchRequest::new(term);
    req.duration_ms = track.duration_ms;
    req.limit = 1;

    let opt = lyrics::model::LyricsSearchOptions {
        timeout_ms: 8000,
        strict_match: false,
        services: vec![
            lyrics::model::LyricsService::QQMusic,
            lyrics::model::LyricsService::Netease,
            lyrics::model::LyricsService::LrcLib,
        ],
    };

    let mut items = lyrics::core::search_with_http(http, &req, opt)
        .await
        .map_err(|e| e.to_string())?;
    let Some(best) = items.pop() else {
        return Ok(());
    };
    if best.lyrics_original.trim().is_empty() {
        return Ok(());
    }

    let lrc_path = audio_path.with_extension("lrc");
    if !overwrite && lrc_path.exists() {
        return Ok(());
    }

    let mut content = best.lyrics_original;
    if let Some(t) = best.lyrics_translation {
        if !t.trim().is_empty() {
            content.push_str("\n\n");
            content.push_str(&t);
        }
    }
    tokio::fs::write(&lrc_path, content)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_download_start_json(
    start_params_json_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(start_params_json_utf8, "start_params_json_utf8")?;
        let params: MusicDownloadStartParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid start_params_json_utf8", Some(e.to_string()));
        })?;

        let MusicDownloadStartParams {
            config,
            auth,
            target,
            options,
        } = params;

        let out_dir = options.out_dir.trim().to_string();
        if out_dir.is_empty() {
            set_last_error("options.outDir is empty", None);
            return Err(());
        }
        let quality_id = options.quality_id.trim().to_string();
        if quality_id.is_empty() {
            set_last_error("options.qualityId is empty", None);
            return Err(());
        }

        let cfg_core = map_music_provider_config_to_core(config);
        let client = {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.cfg = cfg_core.clone();
            locked.client.set_config(cfg_core.clone());
            locked.client.clone()
        };

        let session_id = gen_session_id("musicdl");
        let sid = session_id.clone();

        runtime()
            .block_on(async move {
                let opts = options;
                let out_dir = PathBuf::from(out_dir);

                let mut auth = map_music_auth_to_core(auth);

                let target_service = match &target {
                    MusicDownloadTarget::Track { track } => track.service,
                    MusicDownloadTarget::Album { service, .. } => *service,
                    MusicDownloadTarget::ArtistAll { service, .. } => *service,
                };
                if matches!(target_service, MusicService::Netease) && auth.netease_cookie.is_none()
                {
                    if let Ok(c) = music::providers::netease::fetch_anonymous_cookie(
                        &client.http,
                        &cfg_core,
                        client.timeout,
                    )
                    .await
                    {
                        auth.netease_cookie = Some(c);
                    }
                }

                let mut items: Vec<(MusicTrack, Option<u32>)> = Vec::new();
                match target {
                    MusicDownloadTarget::Track { track } => items.push((track, None)),
                    MusicDownloadTarget::Album { service, album_id } => {
                        let tracks = client
                            .album_tracks(map_music_service_to_core(service), &album_id)
                            .await
                            .map_err(|e| e.to_string())?;
                        for (idx, t) in tracks.into_iter().enumerate() {
                            items.push((map_music_track_to_proto(t), Some((idx as u32) + 1)));
                        }
                    }
                    MusicDownloadTarget::ArtistAll { service, artist_id } => {
                        let albums = client
                            .artist_albums(map_music_service_to_core(service), &artist_id)
                            .await
                            .map_err(|e| e.to_string())?;
                        let mut seen = std::collections::HashSet::<String>::new();
                        for alb in albums {
                            let album_title = alb.title.clone();
                            let tracks = client
                                .album_tracks(map_music_service_to_core(service), &alb.id)
                                .await
                                .unwrap_or_default();
                            for (idx, mut t) in tracks.into_iter().enumerate() {
                                if !seen.insert(t.id.clone()) {
                                    continue;
                                }
                                if t.album.is_none() {
                                    t.album = Some(album_title.clone());
                                }
                                items.push((map_music_track_to_proto(t), Some((idx as u32) + 1)));
                            }
                        }
                    }
                }

                let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
                let status = MusicDownloadStatus {
                    done: total == 0,
                    totals: MusicDownloadTotals {
                        total,
                        done: 0,
                        failed: 0,
                        skipped: 0,
                        canceled: 0,
                    },
                    jobs: items
                        .iter()
                        .enumerate()
                        .map(|(i, (t, _))| MusicDownloadJobResult {
                            index: i as u32,
                            track_id: Some(t.id.clone()),
                            state: MusicJobState::Pending,
                            path: None,
                            bytes: None,
                            error: None,
                        })
                        .collect(),
                };

                let status = Arc::new(tokio::sync::Mutex::new(status));
                let cancel = Arc::new(AtomicBool::new(false));

                let concurrency = opts.concurrency.max(1).min(16) as usize;
                let retries = opts.retries.min(10);
                let overwrite = opts.overwrite;
                let path_template = opts
                    .path_template
                    .as_deref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                let st = Arc::clone(&status);
                let cancel_flag = Arc::clone(&cancel);
                let req_quality = quality_id;

                let handle = tokio::spawn(async move {
                    if items.is_empty() {
                        let mut s = st.lock().await;
                        s.done = true;
                        return;
                    }

                    let (tx, rx) = tokio::sync::mpsc::channel::<(u32, MusicTrack, Option<u32>)>(
                        items.len().max(1),
                    );
                    for (idx, (t, no)) in items.into_iter().enumerate() {
                        let _ = tx.send((idx as u32, t, no)).await;
                    }
                    drop(tx);
                    let rx = Arc::new(tokio::sync::Mutex::new(rx));

                    let mut joinset = tokio::task::JoinSet::new();
                    for _ in 0..concurrency {
                        let rx = Arc::clone(&rx);
                        let st = Arc::clone(&st);
                        let cancel = Arc::clone(&cancel_flag);
                        let client = client.clone();
                        let auth = auth.clone();
                        let out_dir = out_dir.clone();
                        let req_quality = req_quality.clone();
                        let path_template = path_template.clone();
                        joinset.spawn(async move {
                            loop {
                                if cancel.load(Ordering::Relaxed) {
                                    return;
                                }
                                let next = {
                                    let mut locked = rx.lock().await;
                                    locked.recv().await
                                };
                                let Some((index, track, track_no)) = next else {
                                    return;
                                };

                                {
                                    let mut s = st.lock().await;
                                    if let Some(job) = s.jobs.get_mut(index as usize) {
                                        job.state = MusicJobState::Running;
                                    }
                                }

                                let core_svc = map_music_service_to_core(track.service);
                                let chosen_quality = choose_quality_id(&track, &req_quality)
                                    .unwrap_or_else(|| req_quality.clone());

                                let res: Result<(PathBuf, Option<u64>, Option<String>), String> =
                                    async {
                                        let (url, ext) = client
                                            .track_download_url(
                                                core_svc,
                                                &track.id,
                                                &chosen_quality,
                                                &auth,
                                            )
                                            .await
                                            .map_err(|e| e.to_string())?;

                                        let path = if let Some(tpl) = path_template.as_deref() {
                                            music::util::build_track_path_by_template(
                                                &out_dir,
                                                tpl,
                                                &track.artists,
                                                track.album.as_deref(),
                                                track_no,
                                                &track.title,
                                                &ext,
                                            )
                                        } else {
                                            music::util::build_track_path(
                                                &out_dir,
                                                &track.artists,
                                                track.album.as_deref(),
                                                track_no,
                                                &track.title,
                                                &ext,
                                            )
                                        };

                                        if path.exists() && !overwrite {
                                            return Ok((
                                                path,
                                                None,
                                                Some("skipped: target exists".to_string()),
                                            ));
                                        }
                                        let bytes = music::download::download_url_to_file(
                                            &client.http,
                                            &url,
                                            &path,
                                            client.timeout,
                                            retries,
                                            overwrite,
                                        )
                                        .await
                                        .map_err(|e| e.to_string())?;
                                        Ok((path, Some(bytes), None))
                                    }
                                    .await;

                                match res {
                                    Ok((path, bytes, skipped_msg)) => {
                                        if skipped_msg.is_none() && bytes.is_some() {
                                            let _ = try_download_lyrics_for_track(
                                                &client.http,
                                                &track,
                                                &path,
                                                overwrite,
                                            )
                                            .await;
                                        }
                                        let mut s = st.lock().await;
                                        let mut inc_skipped: u32 = 0;
                                        let mut inc_done: u32 = 0;
                                        if let Some(job) = s.jobs.get_mut(index as usize) {
                                            job.path = Some(path.to_string_lossy().to_string());
                                            job.bytes = bytes;
                                            job.error = skipped_msg;
                                            if job.error.is_some() {
                                                job.state = MusicJobState::Skipped;
                                                inc_skipped = 1;
                                            } else {
                                                job.state = MusicJobState::Done;
                                                inc_done = 1;
                                            }
                                        }
                                        s.totals.skipped =
                                            s.totals.skipped.saturating_add(inc_skipped);
                                        s.totals.done = s.totals.done.saturating_add(inc_done);
                                    }
                                    Err(e) => {
                                        let mut s = st.lock().await;
                                        let mut inc_failed: u32 = 0;
                                        if let Some(job) = s.jobs.get_mut(index as usize) {
                                            job.state = MusicJobState::Failed;
                                            job.error = Some(e);
                                            inc_failed = 1;
                                        }
                                        s.totals.failed =
                                            s.totals.failed.saturating_add(inc_failed);
                                    }
                                }
                            }
                        });
                    }

                    while joinset.join_next().await.is_some() {}
                    let mut s = st.lock().await;
                    s.done = true;
                });

                {
                    let st = music_state();
                    let mut locked = st.lock().map_err(|_| "music state poisoned".to_string())?;
                    locked.downloads.insert(
                        sid.clone(),
                        MusicDownloadSession {
                            status,
                            cancel,
                            handle,
                        },
                    );
                }

                Ok::<(), String>(())
            })
            .map_err(|e| {
                set_last_error("music download start failed", Some(e));
            })?;

        let out = MusicDownloadStartResult { session_id };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error(
                "failed to serialize download start result",
                Some(e.to_string()),
            );
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_download_start_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_download_status_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?
            .trim()
            .to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let status = {
            let st = music_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            let Some(sess) = locked.downloads.get(&sid) else {
                set_last_error("download session not found", None);
                return Err(());
            };
            Arc::clone(&sess.status)
        };

        let out = runtime().block_on(async move { status.lock().await.clone() });
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize download status", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_download_status_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_download_cancel_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?
            .trim()
            .to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let (status, cancel) = {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            let Some(sess) = locked.downloads.get_mut(&sid) else {
                set_last_error("download session not found", None);
                return Err(());
            };
            sess.handle.abort();
            (Arc::clone(&sess.status), Arc::clone(&sess.cancel))
        };

        cancel.store(true, Ordering::Relaxed);

        runtime().block_on(async move {
            let mut st = status.lock().await;
            if !st.done {
                let mut canceled: u32 = 0;
                for job in st.jobs.iter_mut() {
                    if matches!(job.state, MusicJobState::Pending | MusicJobState::Running) {
                        job.state = MusicJobState::Canceled;
                        canceled = canceled.saturating_add(1);
                    }
                }
                st.totals.canceled = st.totals.canceled.saturating_add(canceled);
                st.done = true;
            }
        });

        let out = OkReply { ok: true };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize cancel reply", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_download_cancel_json", None);
            ptr::null_mut()
        }
    }
}

// -----------------------------
// Bili Video (FFI JSON)
// -----------------------------

fn map_bili_auth_to_core(a: BiliAuthState) -> bili_video::auth::AuthState {
    bili_video::auth::AuthState {
        cookie: a.cookie.and_then(|s| (!s.trim().is_empty()).then_some(s)),
        refresh_token: a
            .refresh_token
            .and_then(|s| (!s.trim().is_empty()).then_some(s)),
    }
}

fn map_bili_auth_to_proto(a: bili_video::auth::AuthState) -> BiliAuthState {
    BiliAuthState {
        cookie: a.cookie.and_then(|s| (!s.trim().is_empty()).then_some(s)),
        refresh_token: a
            .refresh_token
            .and_then(|s| (!s.trim().is_empty()).then_some(s)),
    }
}

fn map_core_web_auth_to_bundle(a: bili_video::auth::AuthState) -> Option<BiliAuthBundle> {
    let cookie = a.cookie.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())?;
    let refresh_token = a
        .refresh_token
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Some(BiliAuthBundle {
        web: Some(BiliWebAuth { cookie, refresh_token }),
        tv: None,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_login_qr_create_json() -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let client = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked.client.clone()
        };

        let qr = runtime()
            .block_on(bili_video::auth::login_qr_create(&client))
            .map_err(|e| {
                set_last_error("bili login qr create failed", Some(e.to_string()));
            })?;

        let sid = qr.qrcode_key.trim().to_string();
        if sid.is_empty() {
            set_last_error("empty qrcode_key", None);
            return Err(());
        }

        {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked.login_sessions.insert(
                sid.clone(),
                BiliLoginSession {
                    created_at_ms: now_unix_ms(),
                    qrcode_key: qr.qrcode_key.clone(),
                },
            );
        }

        let out = BiliLoginQr {
            session_id: sid,
            mime: qr.mime,
            base64: qr.base64,
            url: qr.url,
            qrcode_key: qr.qrcode_key,
            created_at_unix_ms: qr.created_at_unix_ms,
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize BiliLoginQr", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_login_qr_create_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_login_qr_poll_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?.trim().to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let (client, key, expired) = {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            let Some(sess) = locked.login_sessions.get(&sid) else {
                return Ok(serde_json::to_string(&BiliLoginQrPollResult {
                    session_id: sid,
                    state: BiliLoginQrState::Other,
                    message: Some("login session not found".to_string()),
                    auth: None,
                })
                .unwrap());
            };
            let expired = now_unix_ms().saturating_sub(sess.created_at_ms) > 5 * 60 * 1000;
            let key = sess.qrcode_key.clone();
            if expired {
                locked.login_sessions.remove(&sid);
            }
            (locked.client.clone(), key, expired)
        };

        if expired {
            let out = BiliLoginQrPollResult {
                session_id: sid,
                state: BiliLoginQrState::Timeout,
                message: Some("login session timeout".to_string()),
                auth: None,
            };
            return serde_json::to_string(&out).map_err(|e| {
                set_last_error("failed to serialize poll result", Some(e.to_string()));
            });
        }

        let r = runtime()
            .block_on(bili_video::auth::login_qr_poll(&client, &key))
            .map_err(|e| {
                set_last_error("bili login qr poll failed", Some(e.to_string()));
            })?;

        let state = match r.state {
            bili_video::auth::LoginQrState::Scan => BiliLoginQrState::Scan,
            bili_video::auth::LoginQrState::Confirm => BiliLoginQrState::Confirm,
            bili_video::auth::LoginQrState::Done => BiliLoginQrState::Done,
            bili_video::auth::LoginQrState::Timeout => BiliLoginQrState::Timeout,
            bili_video::auth::LoginQrState::Other => BiliLoginQrState::Other,
        };

        if matches!(state, BiliLoginQrState::Done) {
            let st = bili_state();
            if let Ok(mut locked) = st.lock() {
                locked.login_sessions.remove(&sid);
            }
        }

        let out = BiliLoginQrPollResult {
            session_id: sid,
            state,
            message: r.message,
            auth: r.auth.map(map_bili_auth_to_proto),
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize poll result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_login_qr_poll_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_login_qr_create_v2_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliLoginQrCreateV2Params = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        match params.login_type {
            BiliLoginType::Web => {
                let client = {
                    let st = bili_state();
                    let locked = st.lock().map_err(|_| {
                        set_last_error("bili state poisoned", None);
                    })?;
                    locked.client.clone()
                };

                let qr = runtime()
                    .block_on(bili_video::auth::login_qr_create(&client))
                    .map_err(|e| {
                        set_last_error("bili login qr create failed", Some(e.to_string()));
                    })?;

                let sid = qr.qrcode_key.trim().to_string();
                if sid.is_empty() {
                    set_last_error("empty qrcode_key", None);
                    return Err(());
                }

                {
                    let st = bili_state();
                    let mut locked = st.lock().map_err(|_| {
                        set_last_error("bili state poisoned", None);
                    })?;
                    locked.login_sessions.insert(
                        sid.clone(),
                        BiliLoginSession {
                            created_at_ms: now_unix_ms(),
                            qrcode_key: qr.qrcode_key.clone(),
                        },
                    );
                }

                let out = BiliLoginQr {
                    session_id: sid,
                    mime: qr.mime,
                    base64: qr.base64,
                    url: qr.url,
                    qrcode_key: qr.qrcode_key,
                    created_at_unix_ms: qr.created_at_unix_ms,
                };
                serde_json::to_string(&out).map_err(|e| {
                    set_last_error("failed to serialize BiliLoginQr", Some(e.to_string()));
                })
            }
            BiliLoginType::Tv => {
                let client = {
                    let st = bili_state();
                    let locked = st.lock().map_err(|_| {
                        set_last_error("bili state poisoned", None);
                    })?;
                    locked.client.clone()
                };

                let (qr, tv_sess) = runtime()
                    .block_on(bili_video::auth::login_tv_qr_create(&client))
                    .map_err(|e| {
                        set_last_error("bili tv login qr create failed", Some(e.to_string()));
                    })?;

                let sid = qr.qrcode_key.trim().to_string();
                if sid.is_empty() {
                    set_last_error("empty auth_code", None);
                    return Err(());
                }

                {
                    let st = bili_state();
                    let mut locked = st.lock().map_err(|_| {
                        set_last_error("bili state poisoned", None);
                    })?;
                    locked.tv_login_sessions.insert(
                        sid.clone(),
                        BiliTvLoginSession {
                            created_at_ms: now_unix_ms(),
                            sess: tv_sess,
                        },
                    );
                }

                let out = BiliLoginQr {
                    session_id: sid,
                    mime: qr.mime,
                    base64: qr.base64,
                    url: qr.url,
                    qrcode_key: qr.qrcode_key,
                    created_at_unix_ms: qr.created_at_unix_ms,
                };
                serde_json::to_string(&out).map_err(|e| {
                    set_last_error("failed to serialize BiliLoginQr", Some(e.to_string()));
                })
            }
        }
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_login_qr_create_v2_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_login_qr_poll_v2_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?.trim().to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        // Prefer TV sessions if exists.
        {
            let (client, tv_sess, expired) = {
                let st = bili_state();
                let mut locked = st.lock().map_err(|_| {
                    set_last_error("bili state poisoned", None);
                })?;
                let client = locked.client.clone();
                if let Some(sess) = locked.tv_login_sessions.get(&sid) {
                    let expired = now_unix_ms().saturating_sub(sess.created_at_ms) > 5 * 60 * 1000;
                    let s2 = sess.sess.clone();
                    if expired {
                        locked.tv_login_sessions.remove(&sid);
                    }
                    (client, Some(s2), expired)
                } else {
                    (client, None, false)
                }
            };

            if let Some(tv_sess) = tv_sess {
                if expired {
                    let out = BiliLoginQrPollResultV2 {
                        session_id: sid,
                        state: BiliLoginQrState::Timeout,
                        message: Some("login session timeout".to_string()),
                        auth: None,
                    };
                    return serde_json::to_string(&out).map_err(|e| {
                        set_last_error("failed to serialize poll result", Some(e.to_string()));
                    });
                }

                let r = runtime()
                    .block_on(bili_video::auth::login_tv_qr_poll(&client, &tv_sess))
                    .map_err(|e| {
                        set_last_error("bili tv login qr poll failed", Some(e.to_string()));
                    })?;

                let state = match r.state {
                    bili_video::auth::LoginQrState::Scan => BiliLoginQrState::Scan,
                    bili_video::auth::LoginQrState::Confirm => BiliLoginQrState::Confirm,
                    bili_video::auth::LoginQrState::Done => BiliLoginQrState::Done,
                    bili_video::auth::LoginQrState::Timeout => BiliLoginQrState::Timeout,
                    bili_video::auth::LoginQrState::Other => BiliLoginQrState::Other,
                };

                if matches!(state, BiliLoginQrState::Done) {
                    let st = bili_state();
                    if let Ok(mut locked) = st.lock() {
                        locked.tv_login_sessions.remove(&sid);
                    }
                }

                let auth = r.access_token.map(|access_token| BiliAuthBundle {
                    web: None,
                    tv: Some(BiliTvAuth { access_token }),
                });

                let out = BiliLoginQrPollResultV2 {
                    session_id: sid,
                    state,
                    message: r.message,
                    auth,
                };
                return serde_json::to_string(&out).map_err(|e| {
                    set_last_error("failed to serialize poll result", Some(e.to_string()));
                });
            }
        }

        // Fallback to WEB sessions.
        let (client, key, expired) = {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            let Some(sess) = locked.login_sessions.get(&sid) else {
                let out = BiliLoginQrPollResultV2 {
                    session_id: sid,
                    state: BiliLoginQrState::Other,
                    message: Some("login session not found".to_string()),
                    auth: None,
                };
                return serde_json::to_string(&out).map_err(|e| {
                    set_last_error("failed to serialize poll result", Some(e.to_string()));
                });
            };
            let expired = now_unix_ms().saturating_sub(sess.created_at_ms) > 5 * 60 * 1000;
            let key = sess.qrcode_key.clone();
            if expired {
                locked.login_sessions.remove(&sid);
            }
            (locked.client.clone(), key, expired)
        };

        if expired {
            let out = BiliLoginQrPollResultV2 {
                session_id: sid,
                state: BiliLoginQrState::Timeout,
                message: Some("login session timeout".to_string()),
                auth: None,
            };
            return serde_json::to_string(&out).map_err(|e| {
                set_last_error("failed to serialize poll result", Some(e.to_string()));
            });
        }

        let r = runtime()
            .block_on(bili_video::auth::login_qr_poll(&client, &key))
            .map_err(|e| {
                set_last_error("bili login qr poll v2 failed", Some(e.to_string()));
            })?;

        let state = match r.state {
            bili_video::auth::LoginQrState::Scan => BiliLoginQrState::Scan,
            bili_video::auth::LoginQrState::Confirm => BiliLoginQrState::Confirm,
            bili_video::auth::LoginQrState::Done => BiliLoginQrState::Done,
            bili_video::auth::LoginQrState::Timeout => BiliLoginQrState::Timeout,
            bili_video::auth::LoginQrState::Other => BiliLoginQrState::Other,
        };

        if matches!(state, BiliLoginQrState::Done) {
            let st = bili_state();
            if let Ok(mut locked) = st.lock() {
                locked.login_sessions.remove(&sid);
            }
        }

        let out = BiliLoginQrPollResultV2 {
            session_id: sid,
            state,
            message: r.message,
            auth: r.auth.and_then(map_core_web_auth_to_bundle),
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize poll result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_login_qr_poll_v2_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_check_login_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliCheckLoginParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let mut missing: Vec<String> = Vec::new();

        let web_cookie = params
            .auth
            .web
            .as_ref()
            .map(|w| w.cookie.trim().to_string())
            .filter(|s| !s.is_empty());

        if web_cookie.is_none() {
            missing.push("web.cookie".to_string());
        } else {
            let c = web_cookie.as_deref().unwrap_or("");
            if bili_video::cookie_get(c, "SESSDATA").is_none() {
                missing.push("web.cookie.SESSDATA".to_string());
            }
            if bili_video::cookie_get(c, "bili_jct").is_none() {
                missing.push("web.cookie.bili_jct".to_string());
            }
        }

        let tv_token = params
            .auth
            .tv
            .as_ref()
            .map(|t| t.access_token.trim().to_string())
            .filter(|s| !s.is_empty());
        if tv_token.is_none() {
            missing.push("tv.accessToken".to_string());
        }

        let client = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| set_last_error("bili state poisoned", None))?;
            locked.client.clone()
        };

        let mut is_login = false;
        if let Some(cookie) = web_cookie.as_deref() {
            is_login = runtime()
                .block_on(bili_video::auth::check_login_web(&client, cookie))
                .unwrap_or(false);
        }

        let out = BiliCheckLoginResult {
            is_login,
            reason: (!is_login).then_some("not logged-in".to_string()),
            missing_fields: missing,
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize checkLogin result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_check_login_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_refresh_cookie_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliRefreshCookieParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let client = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| set_last_error("bili state poisoned", None))?;
            locked.client.clone()
        };
        let auth = map_bili_auth_to_core(params.auth);
        let out = runtime()
            .block_on(bili_video::auth::refresh_cookie_if_needed(&client, &auth))
            .map_err(|e| {
                set_last_error("bili refresh cookie failed", Some(e.to_string()));
            })?;

        let res = BiliRefreshCookieResult { auth: map_bili_auth_to_proto(out) };
        serde_json::to_string(&res).map_err(|e| {
            set_last_error("failed to serialize refresh result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_refresh_cookie_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_parse_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliParseParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let auth = params.auth.unwrap_or_default();
        let cookie = auth.cookie.as_deref();

        let client = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| set_last_error("bili state poisoned", None))?;
            locked.client.clone()
        };

        let out = runtime()
            .block_on(async move {
                let parsed = bili_video::parse::parse_input(&client, &params.input).await?;
                match parsed {
                    bili_video::parse::ParsedInput::Video(vid) => {
                        let view = bili_video::parse::fetch_view_info(&client, &vid, cookie).await?;
                        let pages = view
                            .pages
                            .into_iter()
                            .map(|p| BiliPage {
                                page_number: p.page_number,
                                cid: p.cid,
                                page_title: p.page_title,
                                duration_s: p.duration_s,
                                dimension: p.dimension,
                            })
                            .collect::<Vec<_>>();
                        Ok(BiliParseResult {
                            videos: vec![BiliParsedVideo {
                                aid: view.aid,
                                bvid: view.bvid,
                                title: view.title,
                                desc: view.desc,
                                pic: view.pic,
                                owner_name: view.owner_name,
                                owner_mid: view.owner_mid,
                                pub_time_unix_s: view.pub_time_unix_s,
                                pages,
                            }],
                        })
                    }
                    bili_video::parse::ParsedInput::BangumiEpisode { ep_id } => {
                        let season = bili_video::pgc::fetch_pgc_season_by_ep_id(&client, &ep_id, cookie).await?;
                        let pages = season
                            .episodes
                            .iter()
                            .enumerate()
                            .map(|(i, e)| BiliPage {
                                page_number: (i as u32) + 1,
                                cid: e.cid.clone(),
                                page_title: e.title.clone(),
                                duration_s: None,
                                dimension: None,
                            })
                            .collect::<Vec<_>>();
                        let first_aid = season.episodes.first().map(|e| e.aid.clone()).unwrap_or_default();
                        Ok(BiliParseResult {
                            videos: vec![BiliParsedVideo {
                                aid: first_aid,
                                bvid: "".to_string(),
                                title: if season.title.trim().is_empty() { format!("ep{ep_id}") } else { season.title },
                                desc: None,
                                pic: season.cover,
                                owner_name: None,
                                owner_mid: None,
                                pub_time_unix_s: None,
                                pages,
                            }],
                        })
                    }
                    bili_video::parse::ParsedInput::BangumiSeason { season_id } => {
                        let season = bili_video::pgc::fetch_pgc_season_by_season_id(&client, &season_id, cookie).await?;
                        let pages = season
                            .episodes
                            .iter()
                            .enumerate()
                            .map(|(i, e)| BiliPage {
                                page_number: (i as u32) + 1,
                                cid: e.cid.clone(),
                                page_title: e.title.clone(),
                                duration_s: None,
                                dimension: None,
                            })
                            .collect::<Vec<_>>();
                        let first_aid = season.episodes.first().map(|e| e.aid.clone()).unwrap_or_default();
                        Ok(BiliParseResult {
                            videos: vec![BiliParsedVideo {
                                aid: first_aid,
                                bvid: "".to_string(),
                                title: if season.title.trim().is_empty() { format!("ss{season_id}") } else { season.title },
                                desc: None,
                                pic: season.cover,
                                owner_name: None,
                                owner_mid: None,
                                pub_time_unix_s: None,
                                pages,
                            }],
                        })
                    }
                }
            })
            .map_err(|e: bili_video::BiliError| {
                set_last_error("bili parse failed", Some(e.to_string()));
            })?;

        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize parse result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_parse_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_download_start_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliDownloadStartParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        if params.options.out_dir.trim().is_empty() {
            set_last_error("options.outDir is empty", None);
            return Err(());
        }

        let session_id = gen_session_id("bili_dl");
        let status = Arc::new(tokio::sync::Mutex::new(BiliDownloadStatus {
            done: false,
            totals: BiliDownloadTotals { total: 0, done: 0, failed: 0, skipped: 0, canceled: 0 },
            jobs: vec![],
        }));
        let cancel = Arc::new(AtomicBool::new(false));

        let client = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked.client.clone()
        };

        let status2 = Arc::clone(&status);
        let cancel2 = Arc::clone(&cancel);
        let params2 = params.clone();
        let handle = runtime().spawn(async move {
            fn recompute_totals(st: &mut BiliDownloadStatus) {
                let mut done: u32 = 0;
                let mut failed: u32 = 0;
                let mut skipped: u32 = 0;
                let mut canceled: u32 = 0;
                for j in &st.jobs {
                    match j.state {
                        BiliJobState::Done => done = done.saturating_add(1),
                        BiliJobState::Failed => failed = failed.saturating_add(1),
                        BiliJobState::Skipped => skipped = skipped.saturating_add(1),
                        BiliJobState::Canceled => canceled = canceled.saturating_add(1),
                        _ => {}
                    }
                }
                st.totals.done = done;
                st.totals.failed = failed;
                st.totals.skipped = skipped;
                st.totals.canceled = canceled;
            }

            let mut auth = map_bili_auth_to_core(params2.auth.unwrap_or_default());
            if auth.cookie.is_some() && auth.refresh_token.is_some() {
                if let Ok(a) = bili_video::auth::refresh_cookie_if_needed(&client, &auth).await {
                    auth = a;
                }
            }
            let cookie = auth.cookie.as_deref();

            let vid = match bili_video::parse::parse_video_id(&params2.input) {
                Ok(v) => v,
                Err(e) => {
                    let mut st = status2.lock().await;
                    st.jobs = vec![BiliDownloadJobStatus {
                        index: 0,
                        page_number: None,
                        cid: None,
                        title: "parse".to_string(),
                        state: BiliJobState::Failed,
                        phase: BiliJobPhase::Parse,
                        bytes_downloaded: 0,
                        bytes_total: None,
                        speed_bps: None,
                        path: None,
                        error: Some(e.to_string()),
                    }];
                    st.totals.total = 1;
                    st.totals.failed = 1;
                    st.done = true;
                    return;
                }
            };

            let view = match bili_video::parse::fetch_view_info(&client, &vid, cookie).await {
                Ok(v) => v,
                Err(e) => {
                    let mut st = status2.lock().await;
                    st.jobs = vec![BiliDownloadJobStatus {
                        index: 0,
                        page_number: None,
                        cid: None,
                        title: "view".to_string(),
                        state: BiliJobState::Failed,
                        phase: BiliJobPhase::Parse,
                        bytes_downloaded: 0,
                        bytes_total: None,
                        speed_bps: None,
                        path: None,
                        error: Some(e.to_string()),
                    }];
                    st.totals.total = 1;
                    st.totals.failed = 1;
                    st.done = true;
                    return;
                }
            };

            let indices = bili_video::select_page::select_page_indices(view.pages.len(), &params2.options.select_page)
                .unwrap_or_else(|_| (0..view.pages.len()).collect());

            {
                let mut st = status2.lock().await;
                st.jobs = indices.iter().enumerate().map(|(i,&pi)| {
                    let p = &view.pages[pi];
                    BiliDownloadJobStatus {
                        index: i as u32,
                        page_number: Some(p.page_number),
                        cid: Some(p.cid.clone()),
                        title: p.page_title.clone(),
                        state: BiliJobState::Pending,
                        phase: BiliJobPhase::Parse,
                        bytes_downloaded: 0,
                        bytes_total: None,
                        speed_bps: None,
                        path: None,
                        error: None,
                    }
                }).collect();
                st.totals.total = st.jobs.len() as u32;
            }

            for (job_idx, &page_idx) in indices.iter().enumerate() {
                if cancel2.load(Ordering::Relaxed) { break; }

                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) {
                        j.state = BiliJobState::Running;
                        j.phase = BiliJobPhase::Parse;
                        j.error = None;
                    }
                }

                let page = &view.pages[page_idx];
                let base_play = match bili_video::playurl::fetch_playurl_dash(&client, &view.bvid, &view.aid, &page.cid, 0, cookie).await {
                    Ok(p) => p,
                    Err(e) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Failed; j.error = Some(e.to_string()); }
                        recompute_totals(&mut st);
                        continue;
                    }
                };
                let qn = bili_video::playurl::choose_qn_by_dfn_priority(&base_play.accept_quality, &base_play.accept_description, &params2.options.dfn_priority)
                    .unwrap_or(base_play.quality);
                let play = if qn != base_play.quality {
                    bili_video::playurl::fetch_playurl_dash(&client, &view.bvid, &view.aid, &page.cid, qn, cookie).await.unwrap_or(base_play)
                } else { base_play };

                let (v, a) = match bili_video::playurl::pick_dash_tracks(&play, &params2.options.encoding_priority) {
                    Ok(x) => x,
                    Err(e) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Failed; j.error = Some(e.to_string()); }
                        recompute_totals(&mut st);
                        continue;
                    }
                };

                let mut dfn = play.quality.to_string();
                for (i, q) in play.accept_quality.iter().enumerate() {
                    if *q == play.quality {
                        if let Some(desc) = play.accept_description.get(i) {
                            if !desc.trim().is_empty() { dfn = desc.trim().to_string(); }
                        }
                    }
                }
                let res = match (v.width, v.height) { (Some(w), Some(h)) => format!("{w}x{h}"), _ => page.dimension.clone().unwrap_or_default() };
                let fps = v.frame_rate.clone().unwrap_or_default();

                let vars = bili_video::template::TemplateVars {
                    video_title: view.title.clone(),
                    page_number: page.page_number,
                    page_title: page.page_title.clone(),
                    bvid: view.bvid.clone(),
                    aid: view.aid.clone(),
                    cid: page.cid.clone(),
                    dfn,
                    res,
                    fps,
                    video_codecs: v.codecs.clone(),
                    audio_codecs: a.codecs.clone(),
                    owner_name: view.owner_name.clone().unwrap_or_default(),
                    owner_mid: view.owner_mid.clone().unwrap_or_default(),
                };

                let out_mp4 = bili_video::template::build_output_path(
                    std::path::Path::new(&params2.options.out_dir),
                    &params2.options.file_pattern,
                    &params2.options.multi_file_pattern,
                    view.pages.len(),
                    &vars,
                    "mp4",
                );
                if out_mp4.exists() {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) {
                        j.state = BiliJobState::Skipped;
                        j.path = Some(out_mp4.to_string_lossy().to_string());
                        j.error = Some("target exists".to_string());
                    }
                    recompute_totals(&mut st);
                    continue;
                }

                let video_tmp = out_mp4.with_extension("video.m4s");
                let audio_tmp = out_mp4.with_extension("audio.m4s");
                let buvid = bili_video::playurl::ensure_buvid_cookie(&client).await.ok();
                let cookie_hdr = bili_video::merge_cookie_header(buvid.as_deref(), cookie);
                let headers = bili_video::header_map_with_cookie(cookie_hdr.as_deref());

                // video
                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.phase = BiliJobPhase::Video; j.bytes_downloaded = 0; j.bytes_total = None; }
                }
                let prog_downloaded = Arc::new(AtomicU64::new(0));
                let prog_total = Arc::new(AtomicU64::new(0));
                let prog_has_total = Arc::new(AtomicBool::new(false));
                let cb: bili_video::download::ProgressCb = {
                    let prog_downloaded = prog_downloaded.clone();
                    let prog_total = prog_total.clone();
                    let prog_has_total = prog_has_total.clone();
                    Arc::new(move |d, t| {
                        prog_downloaded.store(d, Ordering::Relaxed);
                        if let Some(tt) = t {
                            prog_total.store(tt, Ordering::Relaxed);
                            prog_has_total.store(true, Ordering::Relaxed);
                        }
                    })
                };
                let mut tick = tokio::time::interval(std::time::Duration::from_millis(260));
                let dl = bili_video::download::download_to_file_ranged(&client.http, &v.base_url, &headers, &video_tmp, params2.options.concurrency, params2.options.retries, true, Some(&cancel2), Some(cb));
                tokio::pin!(dl);
                let video_res = loop {
                    tokio::select! {
                        r = &mut dl => break r,
                        _ = tick.tick() => {
                            let d = prog_downloaded.load(Ordering::Relaxed);
                            let t = prog_has_total.load(Ordering::Relaxed).then(|| prog_total.load(Ordering::Relaxed));
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) { j.bytes_downloaded = d; j.bytes_total = t; }
                        }
                    }
                };
                if let Err(e) = video_res {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed }; j.error = Some(e.to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                // audio
                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.phase = BiliJobPhase::Audio; j.bytes_downloaded = 0; j.bytes_total = None; }
                }
                let prog_downloaded = Arc::new(AtomicU64::new(0));
                let prog_total = Arc::new(AtomicU64::new(0));
                let prog_has_total = Arc::new(AtomicBool::new(false));
                let cb: bili_video::download::ProgressCb = {
                    let prog_downloaded = prog_downloaded.clone();
                    let prog_total = prog_total.clone();
                    let prog_has_total = prog_has_total.clone();
                    Arc::new(move |d, t| {
                        prog_downloaded.store(d, Ordering::Relaxed);
                        if let Some(tt) = t {
                            prog_total.store(tt, Ordering::Relaxed);
                            prog_has_total.store(true, Ordering::Relaxed);
                        }
                    })
                };
                let mut tick = tokio::time::interval(std::time::Duration::from_millis(260));
                let dl = bili_video::download::download_to_file_ranged(&client.http, &a.base_url, &headers, &audio_tmp, params2.options.concurrency, params2.options.retries, true, Some(&cancel2), Some(cb));
                tokio::pin!(dl);
                let audio_res = loop {
                    tokio::select! {
                        r = &mut dl => break r,
                        _ = tick.tick() => {
                            let d = prog_downloaded.load(Ordering::Relaxed);
                            let t = prog_has_total.load(Ordering::Relaxed).then(|| prog_total.load(Ordering::Relaxed));
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) { j.bytes_downloaded = d; j.bytes_total = t; }
                        }
                    }
                };
                if let Err(e) = audio_res {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed }; j.error = Some(e.to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                // subtitles
                let mut sub_paths: Vec<std::path::PathBuf> = vec![];
                if params2.options.download_subtitle && !cancel2.load(Ordering::Relaxed) {
                    let _ = {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.phase = BiliJobPhase::Subtitle; }
                    };
                    if let Ok(subs) = bili_video::subtitle::fetch_subtitles(&client, &view.bvid, &page.cid, cookie).await {
                        for s in subs {
                            if cancel2.load(Ordering::Relaxed) { break; }
                            if let Ok(srt) = bili_video::subtitle::download_subtitle_srt(&client, &s.url, cookie).await {
                                let lang = music::util::sanitize_component(&s.lang);
                                let path = out_mp4.with_extension(format!("{lang}.srt"));
                                let _ = tokio::fs::write(&path, srt).await;
                                sub_paths.push(path);
                            }
                        }
                    }
                }

                if cancel2.load(Ordering::Relaxed) {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Canceled; j.error = Some("canceled".to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                if params2.options.skip_mux {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Done; j.phase = BiliJobPhase::Mux; j.path = Some(out_mp4.to_string_lossy().to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Muxing; j.phase = BiliJobPhase::Mux; j.bytes_downloaded = 0; j.bytes_total = None; }
                }
                let mux_res = bili_video::mux::mux_ffmpeg(&params2.options.ffmpeg_path, &video_tmp, &audio_tmp, &sub_paths, &out_mp4, true, Some(&cancel2)).await;
                let _ = tokio::fs::remove_file(&video_tmp).await;
                let _ = tokio::fs::remove_file(&audio_tmp).await;

                match mux_res {
                    Ok(()) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Done; j.phase = BiliJobPhase::Mux; j.path = Some(out_mp4.to_string_lossy().to_string()); }
                        recompute_totals(&mut st);
                    }
                    Err(e) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed }; j.error = Some(e.to_string()); }
                        recompute_totals(&mut st);
                    }
                }
            }

            let mut st = status2.lock().await;
            if cancel2.load(Ordering::Relaxed) {
                for j in st.jobs.iter_mut() {
                    if matches!(j.state, BiliJobState::Pending | BiliJobState::Running | BiliJobState::Muxing) {
                        j.state = BiliJobState::Canceled;
                    }
                }
                recompute_totals(&mut st);
            }
            st.done = true;
        });

        {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked.downloads.insert(
                session_id.clone(),
                BiliDownloadSession {
                    created_at_ms: now_unix_ms(),
                    input: params.input.clone(),
                    api: params.api,
                    status,
                    cancel,
                    handle,
                },
            );
        }

        serde_json::to_string(&BiliDownloadStartResult { session_id }).map_err(|e| {
            set_last_error("failed to serialize start result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_download_start_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_download_status_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?.trim().to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let status = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            let Some(sess) = locked.downloads.get(&sid) else {
                set_last_error("download session not found", None);
                return Err(());
            };
            Arc::clone(&sess.status)
        };

        let out = runtime().block_on(async move { status.lock().await.clone() });
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize download status", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_download_status_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_download_cancel_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?.trim().to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let (status, cancel) = {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            let Some(sess) = locked.downloads.get_mut(&sid) else {
                set_last_error("download session not found", None);
                return Err(());
            };
            sess.handle.abort();
            (Arc::clone(&sess.status), Arc::clone(&sess.cancel))
        };

        cancel.store(true, Ordering::Relaxed);

        runtime().block_on(async move {
            let mut st = status.lock().await;
            if !st.done {
                for j in st.jobs.iter_mut() {
                    if matches!(j.state, BiliJobState::Pending | BiliJobState::Running | BiliJobState::Muxing) {
                        j.state = BiliJobState::Canceled;
                    }
                }
                // recompute totals
                let mut done: u32 = 0;
                let mut failed: u32 = 0;
                let mut skipped: u32 = 0;
                let mut canceled: u32 = 0;
                for j in &st.jobs {
                    match j.state {
                        BiliJobState::Done => done = done.saturating_add(1),
                        BiliJobState::Failed => failed = failed.saturating_add(1),
                        BiliJobState::Skipped => skipped = skipped.saturating_add(1),
                        BiliJobState::Canceled => canceled = canceled.saturating_add(1),
                        _ => {}
                    }
                }
                st.totals.done = done;
                st.totals.failed = failed;
                st.totals.skipped = skipped;
                st.totals.canceled = canceled;
                st.done = true;
            }
        });

        serde_json::to_string(&OkReply { ok: true }).map_err(|e| {
            set_last_error("failed to serialize cancel reply", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_download_cancel_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_task_add_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliTaskAddParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        if params.options.out_dir.trim().is_empty() {
            set_last_error("options.outDir is empty", None);
            return Err(());
        }
        if matches!(params.api, BiliApiType::App) {
            set_last_error("api=app is not supported yet (M3)", None);
            return Err(());
        }

        let input = params.input.trim().to_string();
        if input.is_empty() {
            set_last_error("input is empty", None);
            return Err(());
        }
        let input2 = input.clone();

        let web_cookie = params
            .auth
            .as_ref()
            .and_then(|b| b.web.as_ref())
            .map(|w| w.cookie.trim().to_string())
            .filter(|s| !s.is_empty());
        let web_refresh = params
            .auth
            .as_ref()
            .and_then(|b| b.web.as_ref())
            .and_then(|w| w.refresh_token.clone())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let tv_token = params
            .auth
            .as_ref()
            .and_then(|b| b.tv.as_ref())
            .map(|t| t.access_token.trim().to_string())
            .filter(|s| !s.is_empty());

        let session_id = gen_session_id("bili_task");
        let status = Arc::new(tokio::sync::Mutex::new(BiliDownloadStatus {
            done: false,
            totals: BiliDownloadTotals { total: 0, done: 0, failed: 0, skipped: 0, canceled: 0 },
            jobs: vec![],
        }));
        let cancel = Arc::new(AtomicBool::new(false));

        let client = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked.client.clone()
        };

        let status2 = Arc::clone(&status);
        let cancel2 = Arc::clone(&cancel);
        let api = params.api;
        let options = params.options.clone();
        let out_dir = params.options.out_dir.trim().to_string();

        let handle = runtime().spawn(async move {
            fn recompute_totals(st: &mut BiliDownloadStatus) {
                let mut done: u32 = 0;
                let mut failed: u32 = 0;
                let mut skipped: u32 = 0;
                let mut canceled: u32 = 0;
                for j in &st.jobs {
                    match j.state {
                        BiliJobState::Done => done = done.saturating_add(1),
                        BiliJobState::Failed => failed = failed.saturating_add(1),
                        BiliJobState::Skipped => skipped = skipped.saturating_add(1),
                        BiliJobState::Canceled => canceled = canceled.saturating_add(1),
                        _ => {}
                    }
                }
                st.totals.done = done;
                st.totals.failed = failed;
                st.totals.skipped = skipped;
                st.totals.canceled = canceled;
            }

            // Web auth refresh (best-effort).
            let mut web_auth = bili_video::auth::AuthState {
                cookie: web_cookie,
                refresh_token: web_refresh,
            };
            if web_auth.cookie.is_some() && web_auth.refresh_token.is_some() {
                if let Ok(a) = bili_video::auth::refresh_cookie_if_needed(&client, &web_auth).await {
                    web_auth = a;
                }
            }
            let cookie = web_auth.cookie.as_deref();

            #[derive(Debug, Clone)]
            struct JobInput {
                page_number: u32,
                cid: String,
                title: String,
                dimension: Option<String>,
                aid: String,
                bvid: String,
                ep_id: Option<String>,
                video_title: String,
                owner_name: String,
                owner_mid: String,
            }

            let (all_count, jobs): (usize, Vec<JobInput>) = match bili_video::parse::parse_input(&client, &input2).await {
                Ok(bili_video::parse::ParsedInput::Video(vid)) => {
                    let view = match bili_video::parse::fetch_view_info(&client, &vid, cookie).await {
                        Ok(v) => v,
                        Err(e) => {
                            let mut st = status2.lock().await;
                            st.jobs = vec![BiliDownloadJobStatus {
                                index: 0,
                                page_number: None,
                                cid: None,
                                title: "view".to_string(),
                                state: BiliJobState::Failed,
                                phase: BiliJobPhase::Parse,
                                bytes_downloaded: 0,
                                bytes_total: None,
                                speed_bps: None,
                                path: None,
                                error: Some(e.to_string()),
                            }];
                            st.totals.total = 1;
                            st.totals.failed = 1;
                            st.done = true;
                            return;
                        }
                    };

                    let indices = bili_video::select_page::select_page_indices(view.pages.len(), &options.select_page)
                        .unwrap_or_else(|_| (0..view.pages.len()).collect());

                    let jobs = indices
                        .iter()
                        .map(|&pi| {
                            let p = &view.pages[pi];
                            JobInput {
                                page_number: p.page_number,
                                cid: p.cid.clone(),
                                title: p.page_title.clone(),
                                dimension: p.dimension.clone(),
                                aid: view.aid.clone(),
                                bvid: view.bvid.clone(),
                                ep_id: None,
                                video_title: view.title.clone(),
                                owner_name: view.owner_name.clone().unwrap_or_default(),
                                owner_mid: view.owner_mid.clone().unwrap_or_default(),
                            }
                        })
                        .collect::<Vec<_>>();

                    (view.pages.len(), jobs)
                }
                Ok(bili_video::parse::ParsedInput::BangumiEpisode { ep_id }) => {
                    let season = match bili_video::pgc::fetch_pgc_season_by_ep_id(&client, &ep_id, cookie).await {
                        Ok(s) => s,
                        Err(e) => {
                            let mut st = status2.lock().await;
                            st.jobs = vec![BiliDownloadJobStatus {
                                index: 0,
                                page_number: None,
                                cid: None,
                                title: "pgc".to_string(),
                                state: BiliJobState::Failed,
                                phase: BiliJobPhase::Parse,
                                bytes_downloaded: 0,
                                bytes_total: None,
                                speed_bps: None,
                                path: None,
                                error: Some(e.to_string()),
                            }];
                            st.totals.total = 1;
                            st.totals.failed = 1;
                            st.done = true;
                            return;
                        }
                    };

                    let indices = bili_video::select_page::select_page_indices(season.episodes.len(), &options.select_page)
                        .unwrap_or_else(|_| (0..season.episodes.len()).collect());
                    let jobs = indices
                        .iter()
                        .filter_map(|&ei| season.episodes.get(ei).map(|e| (ei, e)))
                        .map(|(ei, e)| JobInput {
                            page_number: (ei as u32) + 1,
                            cid: e.cid.clone(),
                            title: e.title.clone(),
                            dimension: None,
                            aid: e.aid.clone(),
                            bvid: "".to_string(),
                            ep_id: Some(e.ep_id.clone()),
                            video_title: if season.title.trim().is_empty() { format!("ep{ep_id}") } else { season.title.clone() },
                            owner_name: "".to_string(),
                            owner_mid: "".to_string(),
                        })
                        .collect::<Vec<_>>();
                    (season.episodes.len(), jobs)
                }
                Ok(bili_video::parse::ParsedInput::BangumiSeason { season_id }) => {
                    let season = match bili_video::pgc::fetch_pgc_season_by_season_id(&client, &season_id, cookie).await {
                        Ok(s) => s,
                        Err(e) => {
                            let mut st = status2.lock().await;
                            st.jobs = vec![BiliDownloadJobStatus {
                                index: 0,
                                page_number: None,
                                cid: None,
                                title: "pgc".to_string(),
                                state: BiliJobState::Failed,
                                phase: BiliJobPhase::Parse,
                                bytes_downloaded: 0,
                                bytes_total: None,
                                speed_bps: None,
                                path: None,
                                error: Some(e.to_string()),
                            }];
                            st.totals.total = 1;
                            st.totals.failed = 1;
                            st.done = true;
                            return;
                        }
                    };

                    let indices = bili_video::select_page::select_page_indices(season.episodes.len(), &options.select_page)
                        .unwrap_or_else(|_| (0..season.episodes.len()).collect());
                    let jobs = indices
                        .iter()
                        .filter_map(|&ei| season.episodes.get(ei).map(|e| (ei, e)))
                        .map(|(ei, e)| JobInput {
                            page_number: (ei as u32) + 1,
                            cid: e.cid.clone(),
                            title: e.title.clone(),
                            dimension: None,
                            aid: e.aid.clone(),
                            bvid: "".to_string(),
                            ep_id: Some(e.ep_id.clone()),
                            video_title: if season.title.trim().is_empty() { format!("ss{season_id}") } else { season.title.clone() },
                            owner_name: "".to_string(),
                            owner_mid: "".to_string(),
                        })
                        .collect::<Vec<_>>();
                    (season.episodes.len(), jobs)
                }
                Err(e) => {
                    let mut st = status2.lock().await;
                    st.jobs = vec![BiliDownloadJobStatus {
                        index: 0,
                        page_number: None,
                        cid: None,
                        title: "parse".to_string(),
                        state: BiliJobState::Failed,
                        phase: BiliJobPhase::Parse,
                        bytes_downloaded: 0,
                        bytes_total: None,
                        speed_bps: None,
                        path: None,
                        error: Some(e.to_string()),
                    }];
                    st.totals.total = 1;
                    st.totals.failed = 1;
                    st.done = true;
                    return;
                }
            };

            {
                let mut st = status2.lock().await;
                st.jobs = jobs
                    .iter()
                    .enumerate()
                    .map(|(i, j)| BiliDownloadJobStatus {
                        index: i as u32,
                        page_number: Some(j.page_number),
                        cid: Some(j.cid.clone()),
                        title: j.title.clone(),
                        state: BiliJobState::Pending,
                        phase: BiliJobPhase::Parse,
                        bytes_downloaded: 0,
                        bytes_total: None,
                        speed_bps: None,
                        path: None,
                        error: None,
                    })
                    .collect();
                st.totals.total = st.jobs.len() as u32;
            }

            for (job_idx, job) in jobs.iter().enumerate() {
                if cancel2.load(Ordering::Relaxed) { break; }

                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) {
                        j.state = BiliJobState::Running;
                        j.phase = BiliJobPhase::Parse;
                        j.bytes_downloaded = 0;
                        j.bytes_total = None;
                        j.speed_bps = None;
                        j.error = None;
                    }
                }

                async fn fetch_play(
                    client: &bili_video::BiliClient,
                    api: BiliApiType,
                    is_pgc: bool,
                    aid: &str,
                    bvid: &str,
                    cid: &str,
                    ep_id: Option<&str>,
                    qn: i32,
                    cookie: Option<&str>,
                    tv_token: Option<&str>,
                ) -> Result<(bili_video::playurl::PlayurlInfo, bool), bili_video::BiliError> {
                    let web = async {
                        if is_pgc {
                            bili_video::playurl::fetch_playurl_dash_pgc_web(client, aid, cid, ep_id.unwrap_or(""), qn, cookie).await
                        } else {
                            bili_video::playurl::fetch_playurl_dash(client, bvid, aid, cid, qn, cookie).await
                        }
                    };
                    let tv = async {
                        if is_pgc {
                            bili_video::playurl::fetch_playurl_dash_pgc_tv(client, aid, cid, ep_id.unwrap_or(""), qn, tv_token).await
                        } else {
                            bili_video::playurl::fetch_playurl_dash_tv(client, aid, cid, qn, tv_token).await
                        }
                    };
                    match api {
                        BiliApiType::Web | BiliApiType::Auto => match web.await {
                            Ok(p) => Ok((p, false)),
                            Err(e) => {
                                if tv_token.is_some() && bili_video::api_error_code(&e) == Some(-101) {
                                    Ok((tv.await?, true))
                                } else {
                                    Err(e)
                                }
                            }
                        },
                        BiliApiType::Tv => Ok((tv.await?, false)),
                        BiliApiType::Intl => Err(bili_video::BiliError::InvalidInput("api=intl is not supported yet".to_string())),
                        BiliApiType::App => Err(bili_video::BiliError::InvalidInput("api=app is not supported yet".to_string())),
                    }
                }

                let is_pgc = job.ep_id.is_some();
                let (base_play, used_tv0) = match fetch_play(
                    &client,
                    api,
                    is_pgc,
                    &job.aid,
                    &job.bvid,
                    &job.cid,
                    job.ep_id.as_deref(),
                    0,
                    cookie,
                    tv_token.as_deref(),
                )
                .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Failed; j.error = Some(e.to_string()); }
                        recompute_totals(&mut st);
                        continue;
                    }
                };
                let mut used_tv_fallback = used_tv0;
                let effective_api = if used_tv_fallback { BiliApiType::Tv } else { api };

                let qn = bili_video::playurl::choose_qn_by_dfn_priority(&base_play.accept_quality, &base_play.accept_description, &options.dfn_priority)
                    .unwrap_or(base_play.quality);

                let play = if qn != base_play.quality {
                    match fetch_play(
                        &client,
                        effective_api,
                        is_pgc,
                        &job.aid,
                        &job.bvid,
                        &job.cid,
                        job.ep_id.as_deref(),
                        qn,
                        cookie,
                        tv_token.as_deref(),
                    )
                    .await
                    {
                        Ok((p, used)) => {
                            used_tv_fallback |= used;
                            p
                        }
                        Err(_) => base_play,
                    }
                } else {
                    base_play
                };

                let (v, a) = match bili_video::playurl::pick_dash_tracks(&play, &options.encoding_priority) {
                    Ok(x) => x,
                    Err(e) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Failed; j.error = Some(e.to_string()); }
                        recompute_totals(&mut st);
                        continue;
                    }
                };

                let mut dfn = play.quality.to_string();
                for (i, q) in play.accept_quality.iter().enumerate() {
                    if *q == play.quality {
                        if let Some(desc) = play.accept_description.get(i) {
                            if !desc.trim().is_empty() { dfn = desc.trim().to_string(); }
                        }
                    }
                }

                let res = match (v.width, v.height) { (Some(w), Some(h)) => format!("{w}x{h}"), _ => job.dimension.clone().unwrap_or_default() };
                let fps = v.frame_rate.clone().unwrap_or_default();

                let vars = bili_video::template::TemplateVars {
                    video_title: job.video_title.clone(),
                    page_number: job.page_number,
                    page_title: job.title.clone(),
                    bvid: job.bvid.clone(),
                    aid: job.aid.clone(),
                    cid: job.cid.clone(),
                    dfn,
                    res,
                    fps,
                    video_codecs: v.codecs.clone(),
                    audio_codecs: a.codecs.clone(),
                    owner_name: job.owner_name.clone(),
                    owner_mid: job.owner_mid.clone(),
                };

                let out_mp4 = bili_video::template::build_output_path(
                    std::path::Path::new(&out_dir),
                    &options.file_pattern,
                    &options.multi_file_pattern,
                    all_count,
                    &vars,
                    "mp4",
                );

                if out_mp4.exists() {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Skipped; j.phase = BiliJobPhase::Parse; j.path = Some(out_mp4.to_string_lossy().to_string()); j.error = Some("target exists".to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                let video_tmp = out_mp4.with_extension("video.m4s");
                let audio_tmp = out_mp4.with_extension("audio.m4s");

                let buvid = bili_video::playurl::ensure_buvid_cookie(&client).await.ok();
                let cookie_hdr = bili_video::merge_cookie_header(buvid.as_deref(), cookie);
                let mut headers = bili_video::header_map_with_cookie(cookie_hdr.as_deref());
                if let Some(ep) = job.ep_id.as_deref() {
                    let referer = format!("https://www.bilibili.com/bangumi/play/ep{ep}");
                    if let Ok(v) = reqwest::header::HeaderValue::from_str(&referer) {
                        headers.insert(reqwest::header::REFERER, v);
                    }
                }

                // video
                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.phase = BiliJobPhase::Video; j.bytes_downloaded = 0; j.bytes_total = None; j.speed_bps = None; }
                }
                let prog_downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
                let prog_t0 = std::sync::Arc::new(std::sync::Mutex::new((std::time::Instant::now(), 0u64)));

                let video_prog: bili_video::download::ProgressCb = Arc::new({
                    let status2 = status2.clone();
                    let prog_downloaded = prog_downloaded.clone();
                    let prog_t0 = prog_t0.clone();
                    move |d, t| {
                        prog_downloaded.store(d, Ordering::Relaxed);
                        if let Ok(mut g) = prog_t0.lock() {
                            let now = std::time::Instant::now();
                            let dt = now.duration_since(g.0).as_secs_f64();
                            let speed = if dt > 0.2 {
                                let diff = d.saturating_sub(g.1);
                                g.0 = now;
                                g.1 = d;
                                Some((diff as f64 / dt) as u64)
                            } else { None };
                            let status2 = status2.clone();
                            tokio::spawn(async move {
                                let mut st = status2.lock().await;
                                if let Some(j) = st.jobs.get_mut(job_idx) {
                                    j.bytes_downloaded = d;
                                    j.bytes_total = t;
                                    j.speed_bps = speed;
                                }
                            });
                        }
                    }
                });

                let video_res = bili_video::download::download_to_file_ranged(
                    &client.http,
                    &v.base_url,
                    &headers,
                    &video_tmp,
                    options.concurrency,
                    options.retries,
                    true,
                    Some(&cancel2),
                    Some(video_prog),
                )
                .await;
                if let Err(e) = video_res {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed }; j.error = Some(e.to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                // audio
                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.phase = BiliJobPhase::Audio; j.bytes_downloaded = prog_downloaded.load(Ordering::Relaxed); j.bytes_total = None; j.speed_bps = None; }
                }
                let audio_prog: bili_video::download::ProgressCb = Arc::new({
                    let status2 = status2.clone();
                    move |d, t| {
                        let status2 = status2.clone();
                        tokio::spawn(async move {
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) { j.bytes_downloaded = d; j.bytes_total = t; }
                        });
                    }
                });
                let audio_res = bili_video::download::download_to_file_ranged(
                    &client.http,
                    &a.base_url,
                    &headers,
                    &audio_tmp,
                    options.concurrency,
                    options.retries,
                    true,
                    Some(&cancel2),
                    Some(audio_prog),
                )
                .await;
                if let Err(e) = audio_res {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed }; j.error = Some(e.to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                // subtitles
                let mut sub_paths: Vec<std::path::PathBuf> = vec![];
                if options.download_subtitle && job.ep_id.is_none() && !cancel2.load(Ordering::Relaxed) {
                    let _ = {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.phase = BiliJobPhase::Subtitle; }
                    };
                    if !job.bvid.trim().is_empty() {
                        if let Ok(subs) = bili_video::subtitle::fetch_subtitles(&client, &job.bvid, &job.cid, cookie).await {
                        for s in subs {
                            if cancel2.load(Ordering::Relaxed) { break; }
                            if let Ok(srt) = bili_video::subtitle::download_subtitle_srt(&client, &s.url, cookie).await {
                                let lang = music::util::sanitize_component(&s.lang);
                                let path = out_mp4.with_extension(format!("{lang}.srt"));
                                let _ = tokio::fs::write(&path, srt).await;
                                sub_paths.push(path);
                            }
                        }
                    }
                    }
                }

                if cancel2.load(Ordering::Relaxed) {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Canceled; j.error = Some("canceled".to_string()); }
                    recompute_totals(&mut st);
                    continue;
                }

                if options.skip_mux {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) {
                        j.state = BiliJobState::Done;
                        j.phase = BiliJobPhase::Mux;
                        j.path = Some(out_mp4.to_string_lossy().to_string());
                        if used_tv_fallback && j.error.is_none() {
                            j.error = Some("info: web playurl returned -101, used tv token fallback".to_string());
                        }
                    }
                    recompute_totals(&mut st);
                    continue;
                }

                {
                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) { j.state = BiliJobState::Muxing; j.phase = BiliJobPhase::Mux; j.bytes_downloaded = 0; j.bytes_total = None; }
                }
                let mux_res = bili_video::mux::mux_ffmpeg(&options.ffmpeg_path, &video_tmp, &audio_tmp, &sub_paths, &out_mp4, true, Some(&cancel2)).await;
                let _ = tokio::fs::remove_file(&video_tmp).await;
                let _ = tokio::fs::remove_file(&audio_tmp).await;

                match mux_res {
                    Ok(()) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.state = BiliJobState::Done;
                            j.phase = BiliJobPhase::Mux;
                            j.path = Some(out_mp4.to_string_lossy().to_string());
                            if used_tv_fallback && j.error.is_none() {
                                j.error = Some("info: web playurl returned -101, used tv token fallback".to_string());
                            }
                        }
                        recompute_totals(&mut st);
                    }
                    Err(e) => {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) { j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed }; j.error = Some(e.to_string()); }
                        recompute_totals(&mut st);
                    }
                }
            }

            let mut st = status2.lock().await;
            if cancel2.load(Ordering::Relaxed) {
                for j in st.jobs.iter_mut() {
                    if matches!(j.state, BiliJobState::Pending | BiliJobState::Running | BiliJobState::Muxing) {
                        j.state = BiliJobState::Canceled;
                    }
                }
                recompute_totals(&mut st);
            }
            st.done = true;
        });

        {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked.downloads.insert(
                session_id.clone(),
                BiliDownloadSession {
                    created_at_ms: now_unix_ms(),
                    input,
                    api,
                    status,
                    cancel,
                    handle,
                },
            );
        }

        serde_json::to_string(&BiliTaskAddResult { task_id: session_id }).map_err(|e| {
            set_last_error("failed to serialize taskAdd result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_task_add_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_tasks_get_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let _params: BiliTasksGetParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let snapshot = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked
                .downloads
                .iter()
                .map(|(id, sess)| {
                    (
                        id.clone(),
                        sess.input.clone(),
                        sess.api,
                        sess.created_at_ms,
                        Arc::clone(&sess.status),
                    )
                })
                .collect::<Vec<_>>()
        };

        let mut running: Vec<BiliTask> = Vec::new();
        let mut finished: Vec<BiliTask> = Vec::new();

        for (task_id, input, api, created_at_ms, status) in snapshot {
            let st = runtime().block_on(async { status.lock().await.clone() });
            let done = st.done;
            let totals = st.totals.clone();
            let task = BiliTask {
                task_id,
                input,
                api,
                created_at_unix_ms: created_at_ms,
                done,
                totals,
            };
            if done {
                finished.push(task);
            } else {
                running.push(task);
            }
        }

        running.sort_by(|a, b| b.created_at_unix_ms.cmp(&a.created_at_unix_ms));
        finished.sort_by(|a, b| b.created_at_unix_ms.cmp(&a.created_at_unix_ms));

        let out = BiliTasksGetResult { running, finished };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize tasksGet result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_tasks_get_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_task_get_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliTaskGetParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let tid = params.task_id.trim().to_string();
        if tid.is_empty() {
            set_last_error("taskId is empty", None);
            return Err(());
        }

        let (input, api, created_at_ms, status) = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            let Some(sess) = locked.downloads.get(&tid) else {
                set_last_error("task not found", None);
                return Err(());
            };
            (
                sess.input.clone(),
                sess.api,
                sess.created_at_ms,
                Arc::clone(&sess.status),
            )
        };

        let st = runtime().block_on(async { status.lock().await.clone() });
        let done = st.done;
        let totals = st.totals.clone();
        let task = BiliTask {
            task_id: tid,
            input,
            api,
            created_at_unix_ms: created_at_ms,
            done,
            totals,
        };

        let out = BiliTaskDetail { task, status: st };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize taskGet result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_task_get_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_task_cancel_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliTaskCancelParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let sid = params.task_id.trim().to_string();
        if sid.is_empty() {
            set_last_error("taskId is empty", None);
            return Err(());
        }

        let (status, cancel) = {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            let Some(sess) = locked.downloads.get_mut(&sid) else {
                set_last_error("task not found", None);
                return Err(());
            };
            sess.handle.abort();
            (Arc::clone(&sess.status), Arc::clone(&sess.cancel))
        };

        cancel.store(true, Ordering::Relaxed);
        runtime().block_on(async move {
            let mut st = status.lock().await;
            if !st.done {
                for j in st.jobs.iter_mut() {
                    if matches!(j.state, BiliJobState::Pending | BiliJobState::Running | BiliJobState::Muxing) {
                        j.state = BiliJobState::Canceled;
                    }
                }
                let mut done: u32 = 0;
                let mut failed: u32 = 0;
                let mut skipped: u32 = 0;
                let mut canceled: u32 = 0;
                for j in &st.jobs {
                    match j.state {
                        BiliJobState::Done => done = done.saturating_add(1),
                        BiliJobState::Failed => failed = failed.saturating_add(1),
                        BiliJobState::Skipped => skipped = skipped.saturating_add(1),
                        BiliJobState::Canceled => canceled = canceled.saturating_add(1),
                        _ => {}
                    }
                }
                st.totals.done = done;
                st.totals.failed = failed;
                st.totals.skipped = skipped;
                st.totals.canceled = canceled;
                st.done = true;
            }
        });

        serde_json::to_string(&OkReply { ok: true }).map_err(|e| {
            set_last_error("failed to serialize cancel reply", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_task_cancel_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_bili_tasks_remove_finished_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: BiliTasksRemoveFinishedParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let only_failed = params.only_failed.unwrap_or(false);
        let filter_id = params.task_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        let snapshot = {
            let st = bili_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            locked
                .downloads
                .iter()
                .filter(|(k, _)| filter_id.as_ref().map(|id| *k == id).unwrap_or(true))
                .map(|(k, sess)| (k.clone(), Arc::clone(&sess.status)))
                .collect::<Vec<_>>()
        };

        let mut to_remove: Vec<String> = Vec::new();
        for (k, status) in snapshot {
            let st = runtime().block_on(async { status.lock().await.clone() });
            if !st.done {
                continue;
            }
            if only_failed && st.totals.failed == 0 && st.totals.canceled == 0 {
                continue;
            }
            to_remove.push(k);
        }

        if !to_remove.is_empty() {
            let st = bili_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("bili state poisoned", None);
            })?;
            for k in to_remove {
                locked.downloads.remove(&k);
            }
        }

        serde_json::to_string(&OkReply { ok: true }).map_err(|e| {
            set_last_error("failed to serialize removeFinished reply", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_bili_tasks_remove_finished_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_music_download_blocking_json(
    start_params_json_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let json = require_cstr(start_params_json_utf8, "start_params_json_utf8")?;
        let params: MusicDownloadStartParams = serde_json::from_str(json).map_err(|e| {
            set_last_error("invalid start_params_json_utf8", Some(e.to_string()));
        })?;

        let MusicDownloadStartParams {
            config,
            auth,
            target,
            options,
        } = params;

        let cfg_core = map_music_provider_config_to_core(config);
        let client = {
            let st = music_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("music state poisoned", None);
            })?;
            locked.cfg = cfg_core.clone();
            locked.client.set_config(cfg_core.clone());
            locked.client.clone()
        };

        let out = runtime()
            .block_on(async move {
                let out_dir = options.out_dir.trim().to_string();
                if out_dir.is_empty() {
                    return Err("options.outDir is empty".to_string());
                }
                let requested_quality = options.quality_id.trim().to_string();
                if requested_quality.is_empty() {
                    return Err("options.qualityId is empty".to_string());
                }

                let mut auth = map_music_auth_to_core(auth);

                let target_service = match &target {
                    MusicDownloadTarget::Track { track } => track.service,
                    MusicDownloadTarget::Album { service, .. } => *service,
                    MusicDownloadTarget::ArtistAll { service, .. } => *service,
                };
                if matches!(target_service, MusicService::Netease) && auth.netease_cookie.is_none()
                {
                    if let Ok(c) = music::providers::netease::fetch_anonymous_cookie(
                        &client.http,
                        &cfg_core,
                        client.timeout,
                    )
                    .await
                    {
                        auth.netease_cookie = Some(c);
                    }
                }

                let mut tracks: Vec<(MusicTrack, Option<u32>)> = Vec::new();
                match target {
                    MusicDownloadTarget::Track { track } => tracks.push((track, None)),
                    MusicDownloadTarget::Album { service, album_id } => {
                        let list = client
                            .album_tracks(map_music_service_to_core(service), &album_id)
                            .await
                            .map_err(|e| e.to_string())?;
                        for (idx, t) in list.into_iter().enumerate() {
                            tracks.push((map_music_track_to_proto(t), Some((idx as u32) + 1)));
                        }
                    }
                    MusicDownloadTarget::ArtistAll { service, artist_id } => {
                        let albums = client
                            .artist_albums(map_music_service_to_core(service), &artist_id)
                            .await
                            .map_err(|e| e.to_string())?;
                        let mut seen = std::collections::HashSet::<String>::new();
                        for alb in albums {
                            let album_title = alb.title.clone();
                            let list = client
                                .album_tracks(map_music_service_to_core(service), &alb.id)
                                .await
                                .unwrap_or_default();
                            for (idx, mut t) in list.into_iter().enumerate() {
                                if !seen.insert(t.id.clone()) {
                                    continue;
                                }
                                if t.album.is_none() {
                                    t.album = Some(album_title.clone());
                                }
                                tracks.push((map_music_track_to_proto(t), Some((idx as u32) + 1)));
                            }
                        }
                    }
                }

                let out_dir = PathBuf::from(out_dir);
                let overwrite = options.overwrite;
                let retries = options.retries.min(10);

                let total = u32::try_from(tracks.len()).unwrap_or(u32::MAX);
                let mut status = MusicDownloadStatus {
                    done: total == 0,
                    totals: MusicDownloadTotals {
                        total,
                        done: 0,
                        failed: 0,
                        skipped: 0,
                        canceled: 0,
                    },
                    jobs: tracks
                        .iter()
                        .enumerate()
                        .map(|(i, (t, _))| MusicDownloadJobResult {
                            index: i as u32,
                            track_id: Some(t.id.clone()),
                            state: MusicJobState::Pending,
                            path: None,
                            bytes: None,
                            error: None,
                        })
                        .collect(),
                };

                let path_template = options
                    .path_template
                    .as_deref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                for (idx, (track, track_no)) in tracks.into_iter().enumerate() {
                    if let Some(job) = status.jobs.get_mut(idx) {
                        job.state = MusicJobState::Running;
                    }

                    let chosen_quality = choose_quality_id(&track, &requested_quality)
                        .unwrap_or_else(|| requested_quality.clone());
                    let core_svc = map_music_service_to_core(track.service);

                    let (url, ext) = client
                        .track_download_url(core_svc, &track.id, &chosen_quality, &auth)
                        .await
                        .map_err(|e| e.to_string())?;
                    let path = if let Some(tpl) = path_template.as_deref() {
                        music::util::build_track_path_by_template(
                            &out_dir,
                            tpl,
                            &track.artists,
                            track.album.as_deref(),
                            track_no,
                            &track.title,
                            &ext,
                        )
                    } else {
                        music::util::build_track_path(
                            &out_dir,
                            &track.artists,
                            track.album.as_deref(),
                            track_no,
                            &track.title,
                            &ext,
                        )
                    };
                    if path.exists() && !overwrite {
                        if let Some(job) = status.jobs.get_mut(idx) {
                            job.state = MusicJobState::Skipped;
                            job.path = Some(path.to_string_lossy().to_string());
                            job.error = Some("skipped: target exists".to_string());
                        }
                        status.totals.skipped = status.totals.skipped.saturating_add(1);
                        continue;
                    }
                    match music::download::download_url_to_file(
                        &client.http,
                        &url,
                        &path,
                        client.timeout,
                        retries,
                        overwrite,
                    )
                    .await
                    {
                        Ok(bytes) => {
                            let _ = try_download_lyrics_for_track(
                                &client.http,
                                &track,
                                &path,
                                overwrite,
                            )
                            .await;
                            if let Some(job) = status.jobs.get_mut(idx) {
                                job.state = MusicJobState::Done;
                                job.path = Some(path.to_string_lossy().to_string());
                                job.bytes = Some(bytes);
                            }
                            status.totals.done = status.totals.done.saturating_add(1);
                        }
                        Err(e) => {
                            if let Some(job) = status.jobs.get_mut(idx) {
                                job.state = MusicJobState::Failed;
                                job.error = Some(e.to_string());
                            }
                            status.totals.failed = status.totals.failed.saturating_add(1);
                        }
                    }
                }

                status.done = true;
                Ok(status)
            })
            .map_err(|e| {
                set_last_error("music download blocking failed", Some(e));
            })?;

        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize download status", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_music_download_blocking_json", None);
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
        ensure_rustls_provider();
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn livestream_http() -> &'static reqwest::Client {
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        ensure_rustls_provider();
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

fn live_dir_client() -> &'static live_directory::LiveDirectoryClient {
    static CLIENT: OnceLock<live_directory::LiveDirectoryClient> = OnceLock::new();
    CLIENT
        .get_or_init(|| live_directory::LiveDirectoryClient::new().expect("live directory client"))
}

fn map_dir_category(c: live_directory::LiveCategory) -> LiveDirCategory {
    LiveDirCategory {
        id: c.id,
        name: c.name,
        children: c
            .children
            .into_iter()
            .map(|x| LiveDirSubCategory {
                id: x.id,
                parent_id: x.parent_id,
                name: x.name,
                pic: x.pic,
            })
            .collect(),
    }
}

fn map_dir_room(x: live_directory::LiveRoomCard) -> LiveDirRoomCard {
    LiveDirRoomCard {
        site: x.site.as_str().to_string(),
        room_id: x.room_id,
        input: x.input,
        title: x.title,
        cover: x.cover,
        user_name: x.user_name,
        online: x.online,
    }
}

// -----------------------------
// Music (FFI)
// -----------------------------

fn now_unix_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn gen_session_id(prefix: &str) -> String {
    format!("{prefix}_{}_{:x}", now_unix_ms(), fastrand::u64(..))
}

fn map_music_service_to_core(s: MusicService) -> music::model::MusicService {
    match s {
        MusicService::Qq => music::model::MusicService::Qq,
        MusicService::Kugou => music::model::MusicService::Kugou,
        MusicService::Netease => music::model::MusicService::Netease,
        MusicService::Kuwo => music::model::MusicService::Kuwo,
    }
}

fn map_music_service_to_proto(s: music::model::MusicService) -> MusicService {
    match s {
        music::model::MusicService::Qq => MusicService::Qq,
        music::model::MusicService::Kugou => MusicService::Kugou,
        music::model::MusicService::Netease => MusicService::Netease,
        music::model::MusicService::Kuwo => MusicService::Kuwo,
    }
}

fn map_music_track_to_proto(t: music::model::MusicTrack) -> MusicTrack {
    MusicTrack {
        service: map_music_service_to_proto(t.service),
        id: t.id,
        title: t.title,
        artists: t.artists,
        artist_ids: t.artist_ids,
        album: t.album,
        album_id: t.album_id,
        duration_ms: t.duration_ms,
        cover_url: t.cover_url,
        qualities: t
            .qualities
            .into_iter()
            .map(|q| chaos_proto::MusicQuality {
                id: q.id,
                label: q.label,
                format: q.format,
                bitrate_kbps: q.bitrate_kbps,
                lossless: q.lossless,
            })
            .collect(),
    }
}

fn map_music_album_to_proto(a: music::model::MusicAlbum) -> MusicAlbum {
    MusicAlbum {
        service: map_music_service_to_proto(a.service),
        id: a.id,
        title: a.title,
        artist: a.artist,
        artist_id: a.artist_id,
        cover_url: a.cover_url,
        publish_time: a.publish_time,
        track_count: a.track_count,
    }
}

fn map_music_artist_to_proto(a: music::model::MusicArtist) -> MusicArtist {
    MusicArtist {
        service: map_music_service_to_proto(a.service),
        id: a.id,
        name: a.name,
        cover_url: a.cover_url,
        album_count: a.album_count,
    }
}

fn map_music_provider_config_to_core(cfg: MusicProviderConfig) -> music::model::ProviderConfig {
    music::model::ProviderConfig {
        kugou_base_url: cfg
            .kugou_base_url
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        netease_base_urls: cfg
            .netease_base_urls
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        netease_anonymous_cookie_url: cfg
            .netease_anonymous_cookie_url
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    }
}

fn map_music_auth_to_core(auth: MusicAuthState) -> music::model::AuthState {
    music::model::AuthState {
        qq: auth.qq.map(|c| music::model::QqMusicCookie {
            openid: c.openid,
            refresh_token: c.refresh_token,
            access_token: c.access_token,
            expired_at: c.expired_at,
            musicid: c.musicid,
            musickey: c.musickey,
            musickey_create_time: c.musickey_create_time,
            first_login: c.first_login,
            refresh_key: c.refresh_key,
            login_type: c.login_type,
            str_musicid: c.str_musicid,
            nick: c.nick,
            logo: c.logo,
            encrypt_uin: c.encrypt_uin,
        }),
        kugou: auth.kugou.map(|u| music::model::KugouUserInfo {
            token: u.token,
            userid: u.userid,
        }),
        netease_cookie: auth.netease_cookie,
    }
}

fn map_music_qq_cookie_to_core(c: QqMusicCookie) -> music::model::QqMusicCookie {
    music::model::QqMusicCookie {
        openid: c.openid,
        refresh_token: c.refresh_token,
        access_token: c.access_token,
        expired_at: c.expired_at,
        musicid: c.musicid,
        musickey: c.musickey,
        musickey_create_time: c.musickey_create_time,
        first_login: c.first_login,
        refresh_key: c.refresh_key,
        login_type: c.login_type,
        str_musicid: c.str_musicid,
        nick: c.nick,
        logo: c.logo,
        encrypt_uin: c.encrypt_uin,
    }
}

fn map_music_qq_cookie_to_proto(c: music::model::QqMusicCookie) -> QqMusicCookie {
    QqMusicCookie {
        openid: c.openid,
        refresh_token: c.refresh_token,
        access_token: c.access_token,
        expired_at: c.expired_at,
        musicid: c.musicid,
        musickey: c.musickey,
        refresh_key: c.refresh_key,
        login_type: c.login_type,
        str_musicid: c.str_musicid,
        nick: c.nick,
        logo: c.logo,
        encrypt_uin: c.encrypt_uin,
        musickey_create_time: c.musickey_create_time,
        first_login: c.first_login,
    }
}

fn choose_quality_id(track: &MusicTrack, requested: &str) -> Option<String> {
    let req = requested.trim();
    if req.is_empty() {
        return None;
    }
    if track.qualities.iter().any(|q| q.id == req) {
        return Some(req.to_string());
    }
    for q in music::util::quality_fallback_order() {
        if track.qualities.iter().any(|x| x.id == q) {
            return Some(q.to_string());
        }
    }
    None
}

#[derive(Debug)]
struct QqLoginSession {
    created_at_ms: i64,
    login_type: MusicLoginType,
    identifier: String,
    http: reqwest::Client,
}

#[derive(Debug)]
struct KugouLoginSession {
    created_at_ms: i64,
    login_type: MusicLoginType,
    identifier: String,
}

#[derive(Debug)]
struct MusicDownloadSession {
    status: Arc<tokio::sync::Mutex<MusicDownloadStatus>>,
    cancel: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
struct MusicFfiState {
    client: music::client::MusicClient,
    cfg: music::model::ProviderConfig,
    qq_sessions: HashMap<String, QqLoginSession>,
    kugou_sessions: HashMap<String, KugouLoginSession>,
    downloads: HashMap<String, MusicDownloadSession>,
}

fn music_state() -> &'static Mutex<MusicFfiState> {
    static STATE: OnceLock<Mutex<MusicFfiState>> = OnceLock::new();
    STATE.get_or_init(|| {
        let cfg = music::model::ProviderConfig::default();
        let client = music::client::MusicClient::new(cfg.clone()).expect("music client");
        Mutex::new(MusicFfiState {
            client,
            cfg,
            qq_sessions: HashMap::new(),
            kugou_sessions: HashMap::new(),
            downloads: HashMap::new(),
        })
    })
}

// -----------------------------
// Bili Video (FFI)
// -----------------------------

#[derive(Debug)]
struct BiliLoginSession {
    created_at_ms: i64,
    qrcode_key: String,
}

#[derive(Debug)]
struct BiliTvLoginSession {
    created_at_ms: i64,
    sess: bili_video::auth::TvLoginSession,
}

#[derive(Debug)]
struct BiliDownloadSession {
    created_at_ms: i64,
    input: String,
    api: BiliApiType,
    status: Arc<tokio::sync::Mutex<BiliDownloadStatus>>,
    cancel: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
struct BiliFfiState {
    client: bili_video::BiliClient,
    login_sessions: HashMap<String, BiliLoginSession>,
    tv_login_sessions: HashMap<String, BiliTvLoginSession>,
    downloads: HashMap<String, BiliDownloadSession>,
}

fn bili_state() -> &'static Mutex<BiliFfiState> {
    static STATE: OnceLock<Mutex<BiliFfiState>> = OnceLock::new();
    STATE.get_or_init(|| {
        let client = bili_video::BiliClient::new().expect("bili client");
        Mutex::new(BiliFfiState {
            client,
            login_sessions: HashMap::new(),
            tv_login_sessions: HashMap::new(),
            downloads: HashMap::new(),
        })
    })
}

// -----------------------------
// TTS (FFI)
// -----------------------------

struct TtsSession {
    status: Arc<tokio::sync::Mutex<TtsSftStatus>>,
    cancel: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

struct TtsFfiState {
    engines: HashMap<String, Arc<chaos_tts::CosyVoiceEngine>>,
    sessions: HashMap<String, TtsSession>,
    sem: Arc<tokio::sync::Semaphore>,
}

fn tts_state() -> &'static Mutex<TtsFfiState> {
    static STATE: OnceLock<Mutex<TtsFfiState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(TtsFfiState {
            engines: HashMap::new(),
            sessions: HashMap::new(),
            sem: Arc::new(tokio::sync::Semaphore::new(1)),
        })
    })
}

fn tts_get_engine(model_dir: &str) -> Result<Arc<chaos_tts::CosyVoiceEngine>, String> {
    let key = model_dir.trim().to_string();
    if key.is_empty() {
        return Err("modelDir is empty".into());
    }

    {
        let locked = tts_state().lock().map_err(|_| "tts state poisoned".to_string())?;
        if let Some(e) = locked.engines.get(&key) {
            return Ok(e.clone());
        }
    }

    let engine = {
        let pack = chaos_tts::CosyVoicePack::load(&key).map_err(|e| e.to_string())?;
        let engine = chaos_tts::CosyVoiceEngine::load(pack).map_err(|e| e.to_string())?;
        Arc::new(engine)
    };

    let mut locked = tts_state().lock().map_err(|_| "tts state poisoned".to_string())?;
    locked.engines.entry(key).or_insert_with(|| engine.clone());
    Ok(engine)
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
pub extern "C" fn chaos_lyrics_search_json(
    title_utf8: *const c_char,
    album_utf8_or_null: *const c_char,
    artist_utf8_or_null: *const c_char,
    duration_ms_or_0: u32,
    limit: u32,
    strict_match: u8,
    services_csv_utf8_or_null: *const c_char,
    timeout_ms: u32,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let title = require_cstr(title_utf8, "title_utf8")?.trim().to_string();
        if title.is_empty() {
            set_last_error("title_utf8 is empty", None);
            return Err(());
        }

        let artist = optional_cstr(artist_utf8_or_null, "artist_utf8_or_null")?
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let album = optional_cstr(album_utf8_or_null, "album_utf8_or_null")?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let term = if artist.trim().is_empty() {
            lyrics::model::LyricsSearchTerm::Keyword { keyword: title }
        } else {
            lyrics::model::LyricsSearchTerm::Info {
                title,
                artist,
                album,
            }
        };

        let mut req = lyrics::model::LyricsSearchRequest::new(term);
        req.limit = (limit.max(1) as usize).max(1);
        req.duration_ms = if duration_ms_or_0 == 0 {
            None
        } else {
            Some(duration_ms_or_0 as u64)
        };

        let mut opt = lyrics::model::LyricsSearchOptions::default();
        opt.timeout_ms = timeout_ms.max(1) as u64;
        opt.strict_match = strict_match != 0;

        if let Some(csv) = optional_cstr(services_csv_utf8_or_null, "services_csv_utf8_or_null")?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            let mut out = Vec::new();
            for part in csv.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                let s = lyrics::model::LyricsService::from_str(p).map_err(|e| {
                    set_last_error("invalid services_csv_utf8_or_null", Some(e.to_string()));
                })?;
                out.push(s);
            }
            if !out.is_empty() {
                opt.services = out;
            }
        }

        let items = runtime()
            .block_on(lyrics::core::search(&req, opt))
            .map_err(|e| {
                set_last_error("lyrics search failed", Some(e.to_string()));
            })?;

        serde_json::to_string(&items).map_err(|e| {
            set_last_error(
                "failed to serialize lyrics search result",
                Some(e.to_string()),
            );
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_lyrics_search_json", None);
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
pub extern "C" fn chaos_live_dir_categories_json(site_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let site_s = require_cstr(site_utf8, "site_utf8")?.trim().to_string();
        if site_s.is_empty() {
            set_last_error("site_utf8 is empty", None);
            return Err(());
        }
        let site = parse_site_utf8(&site_s)?;
        let items = runtime()
            .block_on(live_dir_client().get_categories(site))
            .map_err(|e| set_last_error("live dir categories failed", Some(e.to_string())))?;
        let out: Vec<LiveDirCategory> = items.into_iter().map(map_dir_category).collect();
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize categories", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_live_dir_categories_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_live_dir_recommend_rooms_json(
    site_utf8: *const c_char,
    page: u32,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let site_s = require_cstr(site_utf8, "site_utf8")?.trim().to_string();
        if site_s.is_empty() {
            set_last_error("site_utf8 is empty", None);
            return Err(());
        }
        let site = parse_site_utf8(&site_s)?;
        let list = runtime()
            .block_on(live_dir_client().get_recommend_rooms(site, page.max(1)))
            .map_err(|e| set_last_error("live dir recommend failed", Some(e.to_string())))?;
        let out = LiveDirRoomListResult {
            has_more: list.has_more,
            items: list.items.into_iter().map(map_dir_room).collect(),
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize recommend rooms", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_live_dir_recommend_rooms_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_live_dir_category_rooms_json(
    site_utf8: *const c_char,
    parent_id_utf8_or_null: *const c_char,
    category_id_utf8: *const c_char,
    page: u32,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let site_s = require_cstr(site_utf8, "site_utf8")?.trim().to_string();
        if site_s.is_empty() {
            set_last_error("site_utf8 is empty", None);
            return Err(());
        }
        let category_id = require_cstr(category_id_utf8, "category_id_utf8")?
            .trim()
            .to_string();
        if category_id.is_empty() {
            set_last_error("category_id_utf8 is empty", None);
            return Err(());
        }
        let parent_id = optional_cstr(parent_id_utf8_or_null, "parent_id_utf8_or_null")?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let site = parse_site_utf8(&site_s)?;
        let list = runtime()
            .block_on(live_dir_client().get_category_rooms(
                site,
                parent_id.as_deref(),
                &category_id,
                page.max(1),
            ))
            .map_err(|e| set_last_error("live dir category rooms failed", Some(e.to_string())))?;

        let out = LiveDirRoomListResult {
            has_more: list.has_more,
            items: list.items.into_iter().map(map_dir_room).collect(),
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize category rooms", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_live_dir_category_rooms_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_live_dir_search_rooms_json(
    site_utf8: *const c_char,
    keyword_utf8: *const c_char,
    page: u32,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let site_s = require_cstr(site_utf8, "site_utf8")?.trim().to_string();
        if site_s.is_empty() {
            set_last_error("site_utf8 is empty", None);
            return Err(());
        }
        let keyword = require_cstr(keyword_utf8, "keyword_utf8")?
            .trim()
            .to_string();
        if keyword.is_empty() {
            set_last_error("keyword_utf8 is empty", None);
            return Err(());
        }
        let site = parse_site_utf8(&site_s)?;
        let list = runtime()
            .block_on(live_dir_client().search_rooms(site, &keyword, page.max(1)))
            .map_err(|e| set_last_error("live dir search failed", Some(e.to_string())))?;
        let out = LiveDirRoomListResult {
            has_more: list.has_more,
            items: list.items.into_iter().map(map_dir_room).collect(),
        };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize search rooms", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_live_dir_search_rooms_json", None);
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

        // Important: resolve_variant expects the *canonical* room_id (e.g. Douyu rid, Bili long id).
        // So we decode once to obtain (site, canonical room_id), then resolve the requested variant.
        let (site_hint, room_hint) = chaos_core::danmaku::sites::parse_target_hint(&input)
            .map_err(|e| {
                set_last_error("invalid input_utf8", Some(e.to_string()));
            })?;

        let man = runtime()
            .block_on(livestream::platforms::decode_manifest(
                livestream_http(),
                livestream_cfg(),
                site_hint,
                &room_hint,
                &input,
                livestream::ResolveOptions::default(),
            ))
            .map_err(|e| {
                set_last_error("livestream decode failed", Some(e.to_string()));
            })?;

        let v = runtime()
            .block_on(livestream::platforms::resolve_variant(
                livestream_http(),
                livestream_cfg(),
                man.site,
                &man.room_id,
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

fn parse_site_utf8(s: &str) -> Result<danmaku::model::Site, ()> {
    let v = s.trim().to_ascii_lowercase();
    match v.as_str() {
        "bili_live" | "bililive" | "bilibili" | "bili" | "bl" => Ok(danmaku::model::Site::BiliLive),
        "douyu" | "dy" => Ok(danmaku::model::Site::Douyu),
        "huya" | "hy" => Ok(danmaku::model::Site::Huya),
        _ => {
            set_last_error("invalid site_utf8", Some(format!("unsupported site: {s}")));
            Err(())
        }
    }
}

/// Resolve a stream variant using explicit `(site, room_id, variant_id)`.
///
/// Prefer this over `chaos_livestream_resolve_variant_json(input, variant_id)` when you already
/// have the canonical room id from `LiveManifest.room_id`.
#[unsafe(no_mangle)]
pub extern "C" fn chaos_livestream_resolve_variant2_json(
    site_utf8: *const c_char,
    room_id_utf8: *const c_char,
    variant_id_utf8: *const c_char,
) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let site_s = require_cstr(site_utf8, "site_utf8")?;
        let site = parse_site_utf8(site_s)?;

        let room_id = require_cstr(room_id_utf8, "room_id_utf8")?
            .trim()
            .to_string();
        if room_id.is_empty() {
            set_last_error("room_id_utf8 is empty", None);
            return Err(());
        }
        let variant_id = require_cstr(variant_id_utf8, "variant_id_utf8")?
            .trim()
            .to_string();
        if variant_id.is_empty() {
            set_last_error("variant_id_utf8 is empty", None);
            return Err(());
        }

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
            set_last_error("panic in chaos_livestream_resolve_variant2_json", None);
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

// -----------------------------
// TTS (CosyVoice SFT) - FFI JSON
// -----------------------------

#[unsafe(no_mangle)]
pub extern "C" fn chaos_tts_sft_start_json(params_json_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let raw = require_cstr(params_json_utf8, "params_json_utf8")?;
        let params: TtsSftStartParams = serde_json::from_str(raw).map_err(|e| {
            set_last_error("invalid params_json_utf8", Some(e.to_string()));
        })?;

        let model_dir = params.model_dir.trim().to_string();
        if model_dir.is_empty() {
            set_last_error("modelDir is empty", None);
            return Err(());
        }
        let spk_id = params.spk_id.trim().to_string();
        if spk_id.is_empty() {
            set_last_error("spkId is empty", None);
            return Err(());
        }

        let session_id = gen_session_id("tts_sft");
        let status = Arc::new(tokio::sync::Mutex::new(TtsSftStatus {
            done: false,
            state: TtsJobState::Pending,
            stage: Some("loading".to_string()),
            error: None,
            result: None,
        }));
        let cancel = Arc::new(AtomicBool::new(false));

        let sem = {
            let st = tts_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("tts state poisoned", None);
            })?;
            locked.sem.clone()
        };

        let status2 = status.clone();
        let cancel2 = cancel.clone();

        let prompt_strategy = params.prompt_strategy.unwrap_or(TtsPromptStrategy::Inject);
        let guide_sep = params.guide_sep.unwrap_or_else(|| " ".to_string());
        let speed = params.speed.unwrap_or(1.0).max(0.01);
        let seed = params.seed.unwrap_or(1986);
        let temperature = params.temperature.unwrap_or(1.0).max(1e-6);
        let top_p = params.top_p.unwrap_or(0.6).clamp(1e-6, 1.0);
        let top_k = params.top_k.unwrap_or(10).max(1);
        let win_size = params.win_size.unwrap_or(10).max(1);
        let tau_r = params.tau_r.unwrap_or(1.0).max(0.0);
        let text_frontend = params.text_frontend.unwrap_or(true);

        let text = params.text.clone();
        let prompt_text = params.prompt_text.clone();

        let handle = runtime().spawn(async move {
            let _permit = sem
                .clone()
                .acquire_owned()
                .await
                .expect("semaphore acquire");

            {
                let mut st = status2.lock().await;
                st.state = TtsJobState::Running;
                st.stage = Some("loading".to_string());
            }

            let model_dir_for_engine = model_dir.clone();
            let engine = match tokio::task::spawn_blocking(move || tts_get_engine(&model_dir_for_engine)).await {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    let mut st = status2.lock().await;
                    st.done = true;
                    st.state = TtsJobState::Failed;
                    st.stage = Some("loading".to_string());
                    st.error = Some(e);
                    return;
                }
                Err(e) => {
                    let mut st = status2.lock().await;
                    st.done = true;
                    st.state = TtsJobState::Failed;
                    st.stage = Some("loading".to_string());
                    st.error = Some(e.to_string());
                    return;
                }
            };

            {
                let mut st = status2.lock().await;
                st.stage = Some("llm".to_string());
            }

            let prompt_strategy2 = match prompt_strategy {
                TtsPromptStrategy::Inject => chaos_tts::PromptStrategy::Inject,
                TtsPromptStrategy::GuidePrefix => chaos_tts::PromptStrategy::GuidePrefix,
            };

            let params2 = chaos_tts::TtsSftParams {
                model_dir: model_dir.clone(),
                spk_id: spk_id.clone(),
                text,
                prompt_text,
                prompt_strategy: prompt_strategy2,
                guide_sep,
                speed: speed as f32,
                seed,
                sampling: chaos_tts::SamplingConfig {
                    temperature: temperature as f32,
                    top_p: top_p as f32,
                    top_k: top_k as usize,
                    win_size: win_size as usize,
                    tau_r: tau_r as f32,
                },
                text_frontend,
            };

            let cancel_for_run = cancel2.clone();
            let res = tokio::task::spawn_blocking(move || {
                engine.synthesize_sft_with_cancel(&params2, Some(cancel_for_run.as_ref()))
            })
            .await;

            match res {
                Ok(Ok(r)) => {
                    let mut st = status2.lock().await;
                    st.done = true;
                    st.state = TtsJobState::Done;
                    st.stage = Some("done".to_string());
                    st.result = Some(TtsAudioResult {
                        mime: r.mime,
                        wav_base64: r.wav_base64,
                        sample_rate: r.sample_rate,
                        channels: r.channels,
                        duration_ms: r.duration_ms,
                    });
                }
                Ok(Err(e)) => {
                    let canceled = cancel2.load(Ordering::Relaxed)
                        || e.to_string().to_lowercase().contains("canceled");
                    let mut st = status2.lock().await;
                    st.done = true;
                    st.state = if canceled { TtsJobState::Canceled } else { TtsJobState::Failed };
                    st.stage = Some(if canceled { "canceled" } else { "failed" }.to_string());
                    st.error = Some(e.to_string());
                }
                Err(e) => {
                    let mut st = status2.lock().await;
                    st.done = true;
                    st.state = TtsJobState::Failed;
                    st.stage = Some("failed".to_string());
                    st.error = Some(e.to_string());
                }
            }
        });

        {
            let st = tts_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("tts state poisoned", None);
            })?;
            locked.sessions.insert(
                session_id.clone(),
                TtsSession {
                    status,
                    cancel,
                    handle,
                },
            );
        }

        let out = TtsSftStartResult { session_id };
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize tts start result", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_tts_sft_start_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_tts_sft_status_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?
            .trim()
            .to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let status = {
            let st = tts_state();
            let locked = st.lock().map_err(|_| {
                set_last_error("tts state poisoned", None);
            })?;
            let Some(sess) = locked.sessions.get(&sid) else {
                set_last_error("tts session not found", None);
                return Err(());
            };
            Arc::clone(&sess.status)
        };

        let out = runtime().block_on(async move { status.lock().await.clone() });
        serde_json::to_string(&out).map_err(|e| {
            set_last_error("failed to serialize tts status", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_tts_sft_status_json", None);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn chaos_tts_sft_cancel_json(session_id_utf8: *const c_char) -> *mut c_char {
    let res = std::panic::catch_unwind(|| -> Result<String, ()> {
        let sid = require_cstr(session_id_utf8, "session_id_utf8")?
            .trim()
            .to_string();
        if sid.is_empty() {
            set_last_error("session_id_utf8 is empty", None);
            return Err(());
        }

        let (cancel, status) = {
            let st = tts_state();
            let mut locked = st.lock().map_err(|_| {
                set_last_error("tts state poisoned", None);
            })?;
            let Some(sess) = locked.sessions.get_mut(&sid) else {
                set_last_error("tts session not found", None);
                return Err(());
            };
            sess.handle.abort();
            (Arc::clone(&sess.cancel), Arc::clone(&sess.status))
        };

        cancel.store(true, Ordering::Relaxed);
        runtime().block_on(async move {
            let mut st = status.lock().await;
            st.done = true;
            st.state = TtsJobState::Canceled;
            st.stage = Some("canceled".to_string());
            st.error = None;
        });

        serde_json::to_string(&OkReply { ok: true }).map_err(|e| {
            set_last_error("failed to serialize ok reply", Some(e.to_string()));
        })
    });

    match res {
        Ok(Ok(s)) => ok_json(s),
        Ok(Err(())) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic in chaos_tts_sft_cancel_json", None);
            ptr::null_mut()
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
    fn tts_start_rejects_bad_json() {
        let bad = c("{not json}");
        let p = chaos_tts_sft_start_json(bad.as_ptr());
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
    fn livestream_resolve2_rejects_null_args() {
        let p = chaos_livestream_resolve_variant2_json(ptr::null(), ptr::null(), ptr::null());
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

    #[test]
    fn live_dir_categories_rejects_null_site() {
        let p = chaos_live_dir_categories_json(ptr::null());
        assert!(p.is_null());
        let err = chaos_ffi_last_error_json();
        assert!(!err.is_null());
        chaos_ffi_string_free(err);
    }
}
