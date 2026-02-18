mod cli;

#[cfg(windows)]
mod win {
    use crate::cli::{CliOptions, TransportMode};
    use chaos_app::ChaosApp;
    use chaos_core::{bili_video, lyrics, music, now_playing};
    use chaos_daemon::run_jsonrpc_over_lsp;
    use chaos_proto::{
        DanmakuConnectParams, DanmakuConnectResult, DanmakuDisconnectParams,
        DanmakuFetchImageParams, LiveCloseParams, LiveDirCategoriesParams, LiveDirCategory,
        LiveDirCategoryRoomsParams, LiveDirRecommendRoomsParams, LiveDirRoomListResult,
        LiveDirSearchRoomsParams, LiveOpenParams, LivestreamDecodeManifestParams,
        LivestreamDecodeManifestResult, LyricsSearchParams, LyricsSearchResult, NowPlayingSession,
        NowPlayingSnapshot, NowPlayingSnapshotParams, NowPlayingThumbnail, PreferredQuality,
        // music
        KugouUserInfo, MusicAlbum, MusicAlbumTracksParams, MusicArtist, MusicArtistAlbumsParams,
        MusicAuthState, MusicDownloadCancelParams, MusicDownloadJobResult,
        MusicDownloadStartParams, MusicDownloadStartResult, MusicDownloadStatus, MusicDownloadStatusParams,
        MusicDownloadTarget, MusicDownloadTotals, MusicJobState, MusicLoginQr, MusicLoginQrCreateParams,
        MusicLoginQrPollParams, MusicLoginQrPollResult, MusicLoginQrState, MusicLoginType,
        MusicProviderConfig, MusicRefreshCookieParams, MusicSearchParams, MusicService, MusicTrack,
        OkReply, QqMusicCookie,
        // bili
        BiliApiType, BiliAuthState, BiliDownloadCancelParams, BiliDownloadJobStatus,
        BiliDownloadOptions, BiliDownloadStartParams, BiliDownloadStartResult, BiliDownloadStatus,
        BiliDownloadStatusParams, BiliDownloadTotals, BiliJobPhase, BiliJobState, BiliLoginQr,
        BiliLoginQrCreateParams, BiliLoginQrPollParams, BiliLoginQrPollResult, BiliLoginQrState,
        BiliPage, BiliParseParams, BiliParseResult, BiliParsedVideo, BiliRefreshCookieParams,
        BiliRefreshCookieResult,
    };
    use std::env;
    use std::str::FromStr;
    use std::collections::{HashMap, HashSet};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::io::{AsyncRead, AsyncWrite, Stdin, Stdout};
    use tokio::sync::mpsc;
    use tokio::sync::Mutex;
    use tokio::fs;

    const DEFAULT_NETEASE_BASE_URLS: &[&str] = &[
        "http://plugin.changsheng.space:3000",
        "https://wyy.xhily.com",
        "http://111.229.38.178:3333",
        "http://dg-t.cn:3000",
        "https://zm.armoe.cn",
    ];
    const DEFAULT_NETEASE_ANON_URL: &str = "/register/anonimous";

    fn now_unix_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn gen_session_id(prefix: &str) -> String {
        // Stable enough for an in-memory session id.
        format!("{prefix}_{}_{:x}", now_unix_ms(), fastrand::u64(..))
    }

    fn map_service_to_core(s: MusicService) -> music::model::MusicService {
        match s {
            MusicService::Qq => music::model::MusicService::Qq,
            MusicService::Kugou => music::model::MusicService::Kugou,
            MusicService::Netease => music::model::MusicService::Netease,
            MusicService::Kuwo => music::model::MusicService::Kuwo,
        }
    }

    fn map_service_to_proto(s: music::model::MusicService) -> MusicService {
        match s {
            music::model::MusicService::Qq => MusicService::Qq,
            music::model::MusicService::Kugou => MusicService::Kugou,
            music::model::MusicService::Netease => MusicService::Netease,
            music::model::MusicService::Kuwo => MusicService::Kuwo,
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
        fs::write(&lrc_path, content).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    fn map_quality_to_proto(q: music::model::MusicQuality) -> chaos_proto::MusicQuality {
        chaos_proto::MusicQuality {
            id: q.id,
            label: q.label,
            format: q.format,
            bitrate_kbps: q.bitrate_kbps,
            lossless: q.lossless,
        }
    }

    fn map_track_to_proto(t: music::model::MusicTrack) -> MusicTrack {
        MusicTrack {
            service: map_service_to_proto(t.service),
            id: t.id,
            title: t.title,
            artists: t.artists,
            artist_ids: t.artist_ids,
            album: t.album,
            album_id: t.album_id,
            duration_ms: t.duration_ms,
            cover_url: t.cover_url,
            qualities: t.qualities.into_iter().map(map_quality_to_proto).collect(),
        }
    }

    fn map_album_to_proto(a: music::model::MusicAlbum) -> MusicAlbum {
        MusicAlbum {
            service: map_service_to_proto(a.service),
            id: a.id,
            title: a.title,
            artist: a.artist,
            artist_id: a.artist_id,
            cover_url: a.cover_url,
            publish_time: a.publish_time,
            track_count: a.track_count,
        }
    }

    fn map_artist_to_proto(a: music::model::MusicArtist) -> MusicArtist {
        MusicArtist {
            service: map_service_to_proto(a.service),
            id: a.id,
            name: a.name,
            cover_url: a.cover_url,
            album_count: a.album_count,
        }
    }

    fn map_provider_config_to_core(cfg: MusicProviderConfig) -> music::model::ProviderConfig {
        let mut netease: Vec<String> = cfg
            .netease_base_urls
            .into_iter()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if netease.is_empty() {
            netease = DEFAULT_NETEASE_BASE_URLS.iter().map(|s| s.to_string()).collect();
        }

        let anon = cfg
            .netease_anonymous_cookie_url
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| Some(DEFAULT_NETEASE_ANON_URL.to_string()));

        music::model::ProviderConfig {
            kugou_base_url: cfg
                .kugou_base_url
                .as_deref()
                .map(|s| s.trim().trim_end_matches('/').to_string())
                .filter(|s| !s.is_empty()),
            netease_base_urls: netease,
            netease_anonymous_cookie_url: anon,
        }
    }

    fn map_auth_to_core(auth: MusicAuthState) -> music::model::AuthState {
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

    fn map_qq_cookie_to_proto(c: music::model::QqMusicCookie) -> QqMusicCookie {
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

    fn map_bili_auth_to_core(a: Option<BiliAuthState>) -> bili_video::auth::AuthState {
        let a = a.unwrap_or_default();
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
    struct DownloadSession {
        status: Arc<Mutex<MusicDownloadStatus>>,
        cancel: Arc<AtomicBool>,
        handle: tokio::task::JoinHandle<()>,
    }

    #[derive(Debug)]
    struct MusicManager {
        client: Mutex<music::client::MusicClient>,
        cfg: Mutex<music::model::ProviderConfig>,
        qq_sessions: Mutex<HashMap<String, QqLoginSession>>,
        kugou_sessions: Mutex<HashMap<String, KugouLoginSession>>,
        downloads: Mutex<HashMap<String, DownloadSession>>,
    }

    impl MusicManager {
        fn new() -> Result<Self, String> {
            let cfg = music::model::ProviderConfig::default();
            let client = music::client::MusicClient::new(cfg.clone()).map_err(|e| e.to_string())?;
            Ok(Self {
                client: Mutex::new(client),
                cfg: Mutex::new(cfg),
                qq_sessions: Mutex::new(HashMap::new()),
                kugou_sessions: Mutex::new(HashMap::new()),
                downloads: Mutex::new(HashMap::new()),
            })
        }

        async fn set_config(&self, cfg: music::model::ProviderConfig) {
            {
                let mut c = self.cfg.lock().await;
                *c = cfg.clone();
            }
            let mut cli = self.client.lock().await;
            cli.set_config(cfg);
        }

        async fn get_client(&self) -> music::client::MusicClient {
            self.client.lock().await.clone()
        }

        async fn get_cfg(&self) -> music::model::ProviderConfig {
            self.cfg.lock().await.clone()
        }
    }

    #[derive(Debug)]
    struct BiliLoginSession {
        created_at_ms: i64,
        qrcode_key: String,
    }

    #[derive(Debug)]
    struct BiliDownloadSession {
        status: Arc<Mutex<BiliDownloadStatus>>,
        cancel: Arc<AtomicBool>,
        handle: tokio::task::JoinHandle<()>,
    }

    #[derive(Debug)]
    struct BiliManager {
        client: Mutex<bili_video::BiliClient>,
        login_sessions: Mutex<HashMap<String, BiliLoginSession>>,
        downloads: Mutex<HashMap<String, BiliDownloadSession>>,
    }

    impl BiliManager {
        fn new() -> Result<Self, String> {
            let client = bili_video::BiliClient::new().map_err(|e| e.to_string())?;
            Ok(Self {
                client: Mutex::new(client),
                login_sessions: Mutex::new(HashMap::new()),
                downloads: Mutex::new(HashMap::new()),
            })
        }

        async fn get_client(&self) -> bili_video::BiliClient {
            self.client.lock().await.clone()
        }
    }

    struct Svc {
        app: std::sync::Arc<ChaosApp>,
        music: Arc<MusicManager>,
        bili: Arc<BiliManager>,
    }

    impl chaos_daemon::ChaosService for Svc {
        fn version(&self) -> String {
            env!("CARGO_PKG_VERSION").to_string()
        }

        async fn livestream_decode_manifest(
            &self,
            params: LivestreamDecodeManifestParams,
        ) -> Result<LivestreamDecodeManifestResult, String> {
            self.app
                .decode_manifest(&params.input)
                .await
                .map_err(|e| e.to_string())
        }

        async fn live_dir_categories(
            &self,
            params: LiveDirCategoriesParams,
        ) -> Result<Vec<LiveDirCategory>, String> {
            self.app
                .live_dir_categories(&params.site)
                .await
                .map_err(|e| e.to_string())
        }

        async fn live_dir_recommend_rooms(
            &self,
            params: LiveDirRecommendRoomsParams,
        ) -> Result<LiveDirRoomListResult, String> {
            self.app
                .live_dir_recommend_rooms(&params.site, params.page)
                .await
                .map_err(|e| e.to_string())
        }

        async fn live_dir_category_rooms(
            &self,
            params: LiveDirCategoryRoomsParams,
        ) -> Result<LiveDirRoomListResult, String> {
            self.app
                .live_dir_category_rooms(
                    &params.site,
                    params.parent_id.as_deref(),
                    &params.category_id,
                    params.page,
                )
                .await
                .map_err(|e| e.to_string())
        }

        async fn live_dir_search_rooms(
            &self,
            params: LiveDirSearchRoomsParams,
        ) -> Result<LiveDirRoomListResult, String> {
            self.app
                .live_dir_search_rooms(&params.site, &params.keyword, params.page)
                .await
                .map_err(|e| e.to_string())
        }

        async fn now_playing_snapshot(
            &self,
            params: NowPlayingSnapshotParams,
        ) -> Result<NowPlayingSnapshot, String> {
            let include_thumbnail = params.include_thumbnail.unwrap_or(false);
            let max_thumbnail_bytes = params
                .max_thumbnail_bytes
                .unwrap_or(262_144)
                .clamp(1, 2_500_000) as usize;
            let max_sessions = params.max_sessions.unwrap_or(32).clamp(1, 128) as usize;

            let snap = tokio::task::spawn_blocking(move || {
                now_playing::snapshot(now_playing::NowPlayingOptions {
                    include_thumbnail,
                    max_thumbnail_bytes,
                    max_sessions,
                })
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            fn map_thumb(t: &now_playing::NowPlayingThumbnail) -> NowPlayingThumbnail {
                NowPlayingThumbnail {
                    mime: t.mime.clone(),
                    base64: t.base64.clone(),
                }
            }

            fn map_session(s: &now_playing::NowPlayingSession) -> NowPlayingSession {
                NowPlayingSession {
                    app_id: s.app_id.clone(),
                    is_current: s.is_current,
                    playback_status: s.playback_status.clone(),
                    title: s.title.clone(),
                    artist: s.artist.clone(),
                    album_title: s.album_title.clone(),
                    position_ms: s.position_ms,
                    duration_ms: s.duration_ms,
                    genres: s.genres.clone(),
                    song_id: s.song_id.clone(),
                    thumbnail: s.thumbnail.as_ref().map(map_thumb),
                    error: s.error.clone(),
                }
            }

            Ok(NowPlayingSnapshot {
                supported: snap.supported,
                now_playing: snap.now_playing.as_ref().map(map_session),
                sessions: snap.sessions.iter().map(map_session).collect(),
                picked_app_id: snap.picked_app_id.clone(),
                retrieved_at_unix_ms: snap.retrieved_at_unix_ms,
            })
        }

        async fn lyrics_search(
            &self,
            params: LyricsSearchParams,
        ) -> Result<Vec<LyricsSearchResult>, String> {
            let title = params.title.trim().to_string();
            if title.is_empty() {
                return Err("title is empty".to_string());
            }

            let artist = params.artist.as_deref().unwrap_or("").trim().to_string();
            let album = params.album.as_deref().unwrap_or("").trim().to_string();

            let term = if artist.is_empty() {
                lyrics::model::LyricsSearchTerm::Keyword { keyword: title }
            } else {
                lyrics::model::LyricsSearchTerm::Info {
                    title,
                    artist,
                    album: (!album.is_empty()).then_some(album),
                }
            };

            let mut req = lyrics::model::LyricsSearchRequest::new(term);
            req.duration_ms = params.duration_ms.filter(|v| *v > 0);
            if let Some(limit) = params.limit {
                req.limit = (limit as usize).clamp(1, 50);
            }

            let mut opt = lyrics::model::LyricsSearchOptions::default();
            if let Some(v) = params.timeout_ms {
                opt.timeout_ms = v.max(1);
            }
            if let Some(v) = params.strict_match {
                opt.strict_match = v;
            }

            if let Some(services) = params.services {
                let mut out = Vec::new();
                for s in services {
                    let s = s.trim().to_string();
                    if s.is_empty() {
                        continue;
                    }
                    let svc = lyrics::model::LyricsService::from_str(&s).map_err(|e| e)?;
                    out.push(svc);
                }
                if !out.is_empty() {
                    opt.services = out;
                }
            }

            let items = lyrics::core::search(&req, opt)
                .await
                .map_err(|e| e.to_string())?;
            Ok(items
                .into_iter()
                .map(|x| LyricsSearchResult {
                    service: x.service.as_str().to_string(),
                    service_token: x.service_token,
                    title: x.title,
                    artist: x.artist,
                    album: x.album,
                    duration_ms: x.duration_ms,
                    match_percentage: x.match_percentage,
                    quality: x.quality,
                    matched: x.matched,
                    has_translation: x.has_translation,
                    has_inline_timetags: x.has_inline_timetags,
                    lyrics_original: x.lyrics_original,
                    lyrics_translation: x.lyrics_translation,
                    debug: x.debug,
                })
                .collect())
        }

        async fn live_open(
            &self,
            params: LiveOpenParams,
        ) -> Result<
            (
                chaos_proto::LiveOpenResult,
                tokio::sync::mpsc::UnboundedReceiver<chaos_proto::DanmakuMessage>,
            ),
            String,
        > {
            let prefer = params.preferred_quality.unwrap_or_default();
            let prefer_lowest = matches!(prefer, PreferredQuality::Lowest);
            self.app
                .open_live(&params.input, prefer_lowest, params.variant_id.as_deref())
                .await
                .map_err(|e| e.to_string())
        }

        async fn live_close(&self, params: LiveCloseParams) -> Result<(), String> {
            self.app
                .close_live(&params.session_id)
                .await
                .map_err(|e| e.to_string())
        }

        async fn danmaku_connect(
            &self,
            params: DanmakuConnectParams,
        ) -> Result<
            (
                DanmakuConnectResult,
                mpsc::UnboundedReceiver<chaos_proto::DanmakuMessage>,
            ),
            String,
        > {
            let (session_id, site, room_id, rx) = self
                .app
                .danmaku_connect(&params.input)
                .await
                .map_err(|e| e.to_string())?;
            Ok((
                DanmakuConnectResult {
                    session_id,
                    site,
                    room_id,
                },
                rx,
            ))
        }

        async fn danmaku_disconnect(&self, params: DanmakuDisconnectParams) -> Result<(), String> {
            self.app
                .danmaku_disconnect(&params.session_id)
                .await
                .map_err(|e| e.to_string())
        }

        async fn danmaku_fetch_image(
            &self,
            params: DanmakuFetchImageParams,
        ) -> Result<chaos_proto::DanmakuFetchImageResult, String> {
            self.app
                .fetch_image(params)
                .await
                .map_err(|e| e.to_string())
        }

        // ----- music -----

        async fn music_config_set(&self, params: MusicProviderConfig) -> Result<OkReply, String> {
            let cfg = map_provider_config_to_core(params);
            self.music.set_config(cfg).await;
            Ok(OkReply { ok: true })
        }

        async fn music_search_tracks(
            &self,
            params: MusicSearchParams,
        ) -> Result<Vec<MusicTrack>, String> {
            let keyword = params.keyword.trim().to_string();
            if keyword.is_empty() {
                return Ok(vec![]);
            }
            let page = params.page.max(1);
            let page_size = params.page_size.clamp(1, 50).max(1);
            let svc = map_service_to_core(params.service);
            let client = self.music.get_client().await;
            let out = client
                .search_tracks(svc, &keyword, page, page_size)
                .await
                .map_err(|e| e.to_string())?;
            Ok(out.into_iter().map(map_track_to_proto).collect())
        }

        async fn music_search_albums(
            &self,
            params: MusicSearchParams,
        ) -> Result<Vec<MusicAlbum>, String> {
            let keyword = params.keyword.trim().to_string();
            if keyword.is_empty() {
                return Ok(vec![]);
            }
            let page = params.page.max(1);
            let page_size = params.page_size.clamp(1, 50).max(1);
            let svc = map_service_to_core(params.service);
            let client = self.music.get_client().await;
            let out = client
                .search_albums(svc, &keyword, page, page_size)
                .await
                .map_err(|e| e.to_string())?;
            Ok(out.into_iter().map(map_album_to_proto).collect())
        }

        async fn music_search_artists(
            &self,
            params: MusicSearchParams,
        ) -> Result<Vec<MusicArtist>, String> {
            let keyword = params.keyword.trim().to_string();
            if keyword.is_empty() {
                return Ok(vec![]);
            }
            let page = params.page.max(1);
            let page_size = params.page_size.clamp(1, 50).max(1);
            let svc = map_service_to_core(params.service);
            let client = self.music.get_client().await;
            let out = client
                .search_artists(svc, &keyword, page, page_size)
                .await
                .map_err(|e| e.to_string())?;
            Ok(out.into_iter().map(map_artist_to_proto).collect())
        }

        async fn music_album_tracks(
            &self,
            params: MusicAlbumTracksParams,
        ) -> Result<Vec<MusicTrack>, String> {
            let album_id = params.album_id.trim().to_string();
            if album_id.is_empty() {
                return Ok(vec![]);
            }
            let svc = map_service_to_core(params.service);
            let client = self.music.get_client().await;
            let out = client
                .album_tracks(svc, &album_id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(out.into_iter().map(map_track_to_proto).collect())
        }

        async fn music_artist_albums(
            &self,
            params: MusicArtistAlbumsParams,
        ) -> Result<Vec<MusicAlbum>, String> {
            let artist_id = params.artist_id.trim().to_string();
            if artist_id.is_empty() {
                return Ok(vec![]);
            }
            let svc = map_service_to_core(params.service);
            let client = self.music.get_client().await;
            let out = client
                .artist_albums(svc, &artist_id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(out.into_iter().map(map_album_to_proto).collect())
        }

        async fn music_track_play_url(
            &self,
            params: chaos_proto::MusicTrackPlayUrlParams,
        ) -> Result<chaos_proto::MusicTrackPlayUrlResult, String> {
            let track_id = params.track_id.trim().to_string();
            if track_id.is_empty() {
                return Err("trackId is empty".to_string());
            }

            let svc = map_service_to_core(params.service);
            let q = params
                .quality_id
                .unwrap_or_else(|| "mp3_128".to_string())
                .trim()
                .to_string();
            let auth = map_auth_to_core(params.auth);

            let client = self.music.get_client().await;
            let (url, ext) = client
                .track_download_url(svc, &track_id, &q, &auth)
                .await
                .map_err(|e| e.to_string())?;
            Ok(chaos_proto::MusicTrackPlayUrlResult { url, ext })
        }

        async fn music_qq_login_qr_create(
            &self,
            params: MusicLoginQrCreateParams,
        ) -> Result<MusicLoginQr, String> {
            let session_id = gen_session_id("qqlogin");
            let created_at_unix_ms = now_unix_ms();
            let login_type = params.login_type;

            let http = music::providers::qq_login::new_login_client().map_err(|e| e.to_string())?;
            let (identifier, mime, bytes) = music::providers::qq_login::create_login_qr(
                &http,
                match login_type {
                    MusicLoginType::Qq => music::model::MusicLoginType::Qq,
                    MusicLoginType::Wechat => music::model::MusicLoginType::Wechat,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
            let base64 =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);

            {
                let mut sessions = self.music.qq_sessions.lock().await;
                sessions.insert(
                    session_id.clone(),
                    QqLoginSession {
                        created_at_ms: created_at_unix_ms,
                        login_type,
                        identifier: identifier.clone(),
                        http,
                    },
                );
            }

            Ok(MusicLoginQr {
                session_id,
                login_type,
                mime,
                base64,
                identifier,
                created_at_unix_ms,
            })
        }

        async fn music_qq_login_qr_poll(
            &self,
            params: MusicLoginQrPollParams,
        ) -> Result<MusicLoginQrPollResult, String> {
            let sid = params.session_id.trim().to_string();
            if sid.is_empty() {
                return Err("sessionId is empty".to_string());
            }

            let mut sessions = self.music.qq_sessions.lock().await;
            let Some(sess) = sessions.get(&sid) else {
                return Err("session not found".to_string());
            };
            if now_unix_ms().saturating_sub(sess.created_at_ms) > 5 * 60 * 1000 {
                sessions.remove(&sid);
                return Ok(MusicLoginQrPollResult {
                    session_id: sid,
                    state: MusicLoginQrState::Timeout,
                    message: Some("login session timeout".to_string()),
                    cookie: None,
                    kugou_user: None,
                });
            }

            let core_login_type = match sess.login_type {
                MusicLoginType::Qq => music::model::MusicLoginType::Qq,
                MusicLoginType::Wechat => music::model::MusicLoginType::Wechat,
            };
            let (state, msg, sig_or_code, uin) = music::providers::qq_login::poll_login_qr(
                &sess.http,
                core_login_type,
                &sess.identifier,
            )
            .await
            .map_err(|e| e.to_string())?;
            let state_proto = match state {
                music::model::MusicLoginQrState::Scan => MusicLoginQrState::Scan,
                music::model::MusicLoginQrState::Confirm => MusicLoginQrState::Confirm,
                music::model::MusicLoginQrState::Done => MusicLoginQrState::Done,
                music::model::MusicLoginQrState::Timeout => MusicLoginQrState::Timeout,
                music::model::MusicLoginQrState::Refuse => MusicLoginQrState::Refuse,
                music::model::MusicLoginQrState::Other => MusicLoginQrState::Other,
            };

            if state_proto != MusicLoginQrState::Done {
                return Ok(MusicLoginQrPollResult {
                    session_id: sid,
                    state: state_proto,
                    message: msg,
                    cookie: None,
                    kugou_user: None,
                });
            }

            let cookie = match sess.login_type {
                MusicLoginType::Qq => {
                    let sigx = sig_or_code.ok_or_else(|| "missing ptsigx".to_string())?;
                    let uin = uin.ok_or_else(|| "missing uin".to_string())?;
                    let code = music::providers::qq_login::authorize_qq_and_get_code(
                        &sess.http,
                        &sigx,
                        &uin,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    let c = music::providers::qq_login::exchange_code_for_cookie(
                        &sess.http,
                        &code,
                        music::model::MusicLoginType::Qq,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    map_qq_cookie_to_proto(c)
                }
                MusicLoginType::Wechat => {
                    let wx_code = sig_or_code.ok_or_else(|| "missing wx_code".to_string())?;
                    let c = music::providers::qq_login::exchange_code_for_cookie(
                        &sess.http,
                        &wx_code,
                        music::model::MusicLoginType::Wechat,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    map_qq_cookie_to_proto(c)
                }
            };

            sessions.remove(&sid);
            Ok(MusicLoginQrPollResult {
                session_id: sid,
                state: MusicLoginQrState::Done,
                message: None,
                cookie: Some(cookie),
                kugou_user: None,
            })
        }

        async fn music_qq_refresh_cookie(
            &self,
            params: MusicRefreshCookieParams,
        ) -> Result<QqMusicCookie, String> {
            let http = music::providers::qq_login::new_login_client().map_err(|e| e.to_string())?;
            let core_cookie = music::model::QqMusicCookie {
                openid: params.cookie.openid,
                refresh_token: params.cookie.refresh_token,
                access_token: params.cookie.access_token,
                expired_at: params.cookie.expired_at,
                musicid: params.cookie.musicid,
                musickey: params.cookie.musickey,
                musickey_create_time: params.cookie.musickey_create_time,
                first_login: params.cookie.first_login,
                refresh_key: params.cookie.refresh_key,
                login_type: params.cookie.login_type,
                str_musicid: params.cookie.str_musicid,
                nick: params.cookie.nick,
                logo: params.cookie.logo,
                encrypt_uin: params.cookie.encrypt_uin,
            };
            let out = music::providers::qq_login::refresh_cookie(&http, &core_cookie)
                .await
                .map_err(|e| e.to_string())?;
            Ok(map_qq_cookie_to_proto(out))
        }

        async fn music_kugou_login_qr_create(
            &self,
            params: MusicLoginQrCreateParams,
        ) -> Result<MusicLoginQr, String> {
            let session_id = gen_session_id("kugoulogin");
            let created_at_unix_ms = now_unix_ms();
            let login_type = params.login_type;

            let client = self.music.get_client().await;
            let cfg = self.music.get_cfg().await;

            let (identifier, mime, base64) = match login_type {
                MusicLoginType::Qq => {
                    let qr = music::providers::kugou::kugou_qr_create(
                        &client.http,
                        &cfg,
                        client.timeout,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    (qr.key, "image/png".to_string(), qr.image_base64)
                }
                MusicLoginType::Wechat => {
                    let (uuid, data_uri) = music::providers::kugou::kugou_wx_qr_create(
                        &client.http,
                        &cfg,
                        client.timeout,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
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

            {
                let mut sessions = self.music.kugou_sessions.lock().await;
                sessions.insert(
                    session_id.clone(),
                    KugouLoginSession {
                        created_at_ms: created_at_unix_ms,
                        login_type,
                        identifier: identifier.clone(),
                    },
                );
            }

            Ok(MusicLoginQr {
                session_id,
                login_type,
                mime,
                base64,
                identifier,
                created_at_unix_ms,
            })
        }

        async fn music_kugou_login_qr_poll(
            &self,
            params: MusicLoginQrPollParams,
        ) -> Result<MusicLoginQrPollResult, String> {
            let sid = params.session_id.trim().to_string();
            if sid.is_empty() {
                return Err("sessionId is empty".to_string());
            }

            let mut sessions = self.music.kugou_sessions.lock().await;
            let Some(sess) = sessions.get(&sid) else {
                return Err("session not found".to_string());
            };
            if now_unix_ms().saturating_sub(sess.created_at_ms) > 5 * 60 * 1000 {
                sessions.remove(&sid);
                return Ok(MusicLoginQrPollResult {
                    session_id: sid,
                    state: MusicLoginQrState::Timeout,
                    message: Some("login session timeout".to_string()),
                    cookie: None,
                    kugou_user: None,
                });
            }

            let client = self.music.get_client().await;
            let cfg = self.music.get_cfg().await;

            let user = match sess.login_type {
                MusicLoginType::Qq => music::providers::kugou::kugou_qr_poll(
                    &client.http,
                    &cfg,
                    &sess.identifier,
                    client.timeout,
                )
                .await
                .map_err(|e| e.to_string())?,
                MusicLoginType::Wechat => music::providers::kugou::kugou_wx_qr_poll(
                    &client.http,
                    &cfg,
                    &sess.identifier,
                    client.timeout,
                )
                .await
                .map_err(|e| e.to_string())?,
            };

            if let Some(u) = user {
                sessions.remove(&sid);
                return Ok(MusicLoginQrPollResult {
                    session_id: sid,
                    state: MusicLoginQrState::Done,
                    message: None,
                    cookie: None,
                    kugou_user: Some(KugouUserInfo {
                        token: u.token,
                        userid: u.userid,
                    }),
                });
            }

            Ok(MusicLoginQrPollResult {
                session_id: sid,
                state: MusicLoginQrState::Scan,
                message: None,
                cookie: None,
                kugou_user: None,
            })
        }

        async fn music_download_start(
            &self,
            params: MusicDownloadStartParams,
        ) -> Result<MusicDownloadStartResult, String> {
            let out_dir = params.options.out_dir.trim().to_string();
            if out_dir.is_empty() {
                return Err("options.outDir is empty".to_string());
            }
            let quality_id = params.options.quality_id.trim().to_string();
            if quality_id.is_empty() {
                return Err("options.qualityId is empty".to_string());
            }

            let cfg = map_provider_config_to_core(params.config);
            self.music.set_config(cfg.clone()).await;
            let client = self.music.get_client().await;

            let mut auth = map_auth_to_core(params.auth);

            let target_service = match &params.target {
                MusicDownloadTarget::Track { track } => track.service,
                MusicDownloadTarget::Album { service, .. } => *service,
                MusicDownloadTarget::ArtistAll { service, .. } => *service,
            };
            if matches!(target_service, MusicService::Netease) && auth.netease_cookie.is_none() {
                if let Ok(c) = music::providers::netease::fetch_anonymous_cookie(
                    &client.http,
                    &cfg,
                    client.timeout,
                )
                .await
                {
                    auth.netease_cookie = Some(c);
                }
            }

            let mut items: Vec<(MusicTrack, Option<u32>)> = Vec::new();
            match params.target {
                MusicDownloadTarget::Track { track } => items.push((track, None)),
                MusicDownloadTarget::Album { service, album_id } => {
                    let tracks = client
                        .album_tracks(map_service_to_core(service), &album_id)
                        .await
                        .map_err(|e| e.to_string())?;
                    for (idx, t) in tracks.into_iter().enumerate() {
                        items.push((map_track_to_proto(t), Some((idx as u32) + 1)));
                    }
                }
                MusicDownloadTarget::ArtistAll { service, artist_id } => {
                    let albums = client
                        .artist_albums(map_service_to_core(service), &artist_id)
                        .await
                        .map_err(|e| e.to_string())?;
                    let mut seen: HashSet<String> = HashSet::new();
                    for alb in albums {
                        let album_title = alb.title.clone();
                        let tracks = client
                            .album_tracks(map_service_to_core(service), &alb.id)
                            .await
                            .unwrap_or_default();
                        for (idx, mut t) in tracks.into_iter().enumerate() {
                            if !seen.insert(t.id.clone()) {
                                continue;
                            }
                            if t.album.is_none() {
                                t.album = Some(album_title.clone());
                            }
                            items.push((map_track_to_proto(t), Some((idx as u32) + 1)));
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

            let session_id = gen_session_id("musicdl");
            let status = Arc::new(Mutex::new(status));
            let cancel = Arc::new(AtomicBool::new(false));

            let out_dir = PathBuf::from(out_dir);
            let opts = params.options;
            let core_auth = auth.clone();
            let req_quality = quality_id;
            let path_template = opts
                .path_template
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            let st = Arc::clone(&status);
            let cancel_flag = Arc::clone(&cancel);
            let handle = tokio::spawn(async move {
                if items.is_empty() {
                    let mut s = st.lock().await;
                    s.done = true;
                    return;
                }

                let concurrency = opts.concurrency.max(1).min(16) as usize;
                let retries = opts.retries.min(10);
                let overwrite = opts.overwrite;

                let (tx, rx) =
                    tokio::sync::mpsc::channel::<(u32, MusicTrack, Option<u32>)>(items.len().max(1));
                for (idx, (t, no)) in items.into_iter().enumerate() {
                    let _ = tx.send((idx as u32, t, no)).await;
                }
                drop(tx);
                let rx = Arc::new(Mutex::new(rx));

                let mut joinset = tokio::task::JoinSet::new();
                for _ in 0..concurrency {
                    let rx = Arc::clone(&rx);
                    let st = Arc::clone(&st);
                    let cancel = Arc::clone(&cancel_flag);
                    let client = client.clone();
                    let auth = core_auth.clone();
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

                            let core_svc = map_service_to_core(track.service);
                            let chosen_quality = choose_quality_id(&track, &req_quality)
                                .unwrap_or_else(|| req_quality.clone());

                            let res: Result<(PathBuf, Option<u64>, Option<String>), String> = async {
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
                                    return Ok((path, None, Some("skipped: target exists".to_string())));
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
                                    s.totals.skipped = s.totals.skipped.saturating_add(inc_skipped);
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
                                    s.totals.failed = s.totals.failed.saturating_add(inc_failed);
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
                let mut downloads = self.music.downloads.lock().await;
                downloads.insert(
                    session_id.clone(),
                    DownloadSession {
                        status,
                        cancel,
                        handle,
                    },
                );
            }

            Ok(MusicDownloadStartResult { session_id })
        }

        async fn music_download_status(
            &self,
            params: MusicDownloadStatusParams,
        ) -> Result<MusicDownloadStatus, String> {
            let sid = params.session_id.trim().to_string();
            if sid.is_empty() {
                return Err("sessionId is empty".to_string());
            }
            let downloads = self.music.downloads.lock().await;
            let Some(sess) = downloads.get(&sid) else {
                return Err("download session not found".to_string());
            };
            Ok(sess.status.lock().await.clone())
        }

        async fn music_download_cancel(
            &self,
            params: MusicDownloadCancelParams,
        ) -> Result<OkReply, String> {
            let sid = params.session_id.trim().to_string();
            if sid.is_empty() {
                return Err("sessionId is empty".to_string());
            }

            let mut downloads = self.music.downloads.lock().await;
            let Some(sess) = downloads.get_mut(&sid) else {
                return Err("download session not found".to_string());
            };

            sess.cancel.store(true, Ordering::Relaxed);
            sess.handle.abort();

            let mut st = sess.status.lock().await;
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

            Ok(OkReply { ok: true })
        }

        async fn bili_login_qr_create(
            &self,
            _params: BiliLoginQrCreateParams,
        ) -> Result<BiliLoginQr, String> {
            let client = self.bili.get_client().await;
            let qr = bili_video::auth::login_qr_create(&client)
                .await
                .map_err(|e| e.to_string())?;

            let sid = qr.qrcode_key.trim().to_string();
            if sid.is_empty() {
                return Err("empty qrcode_key".to_string());
            }
            let mut sessions = self.bili.login_sessions.lock().await;
            sessions.insert(
                sid.clone(),
                BiliLoginSession {
                    created_at_ms: now_unix_ms(),
                    qrcode_key: qr.qrcode_key.clone(),
                },
            );

            Ok(BiliLoginQr {
                session_id: sid,
                mime: qr.mime,
                base64: qr.base64,
                url: qr.url,
                qrcode_key: qr.qrcode_key,
                created_at_unix_ms: qr.created_at_unix_ms,
            })
        }

        async fn bili_login_qr_poll(
            &self,
            params: BiliLoginQrPollParams,
        ) -> Result<BiliLoginQrPollResult, String> {
            let sid = params.session_id.trim().to_string();
            if sid.is_empty() {
                return Err("sessionId is empty".to_string());
            }

            let mut sessions = self.bili.login_sessions.lock().await;
            let Some(sess) = sessions.get(&sid) else {
                return Ok(BiliLoginQrPollResult {
                    session_id: sid,
                    state: BiliLoginQrState::Other,
                    message: Some("login session not found".to_string()),
                    auth: None,
                });
            };
            if now_unix_ms().saturating_sub(sess.created_at_ms) > 5 * 60 * 1000 {
                sessions.remove(&sid);
                return Ok(BiliLoginQrPollResult {
                    session_id: sid,
                    state: BiliLoginQrState::Timeout,
                    message: Some("login session timeout".to_string()),
                    auth: None,
                });
            }
            let key = sess.qrcode_key.clone();
            drop(sessions);

            let client = self.bili.get_client().await;
            let r = bili_video::auth::login_qr_poll(&client, &key)
                .await
                .map_err(|e| e.to_string())?;

            let state = match r.state {
                bili_video::auth::LoginQrState::Scan => BiliLoginQrState::Scan,
                bili_video::auth::LoginQrState::Confirm => BiliLoginQrState::Confirm,
                bili_video::auth::LoginQrState::Done => BiliLoginQrState::Done,
                bili_video::auth::LoginQrState::Timeout => BiliLoginQrState::Timeout,
                bili_video::auth::LoginQrState::Other => BiliLoginQrState::Other,
            };

            if matches!(state, BiliLoginQrState::Done) {
                let mut sessions = self.bili.login_sessions.lock().await;
                sessions.remove(&sid);
            }

            Ok(BiliLoginQrPollResult {
                session_id: sid,
                state,
                message: r.message,
                auth: r.auth.map(map_bili_auth_to_proto),
            })
        }

        async fn bili_refresh_cookie(
            &self,
            params: BiliRefreshCookieParams,
        ) -> Result<BiliRefreshCookieResult, String> {
            let auth = map_bili_auth_to_core(Some(params.auth));
            let client = self.bili.get_client().await;
            let out = bili_video::auth::refresh_cookie_if_needed(&client, &auth)
                .await
                .map_err(|e| e.to_string())?;
            Ok(BiliRefreshCookieResult {
                auth: map_bili_auth_to_proto(out),
            })
        }

        async fn bili_parse(&self, params: BiliParseParams) -> Result<BiliParseResult, String> {
            let input = params.input.trim().to_string();
            if input.is_empty() {
                return Err("input is empty".to_string());
            }
            let auth = map_bili_auth_to_core(params.auth);
            let cookie = auth.cookie.as_deref();

            let client = self.bili.get_client().await;
            let vid = bili_video::parse::parse_video_id(&input).map_err(|e| e.to_string())?;
            let view = bili_video::parse::fetch_view_info(&client, &vid, cookie)
                .await
                .map_err(|e| e.to_string())?;

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

        async fn bili_download_start(
            &self,
            params: BiliDownloadStartParams,
        ) -> Result<BiliDownloadStartResult, String> {
            let out_dir = params.options.out_dir.trim().to_string();
            if out_dir.is_empty() {
                return Err("options.outDir is empty".to_string());
            }
            if !params.options.skip_mux {
                let p = params.options.ffmpeg_path.trim();
                if p.is_empty() {
                    return Err("options.ffmpegPath is empty".to_string());
                }
            }

            let session_id = gen_session_id("bili_dl");
            let status = Arc::new(Mutex::new(BiliDownloadStatus {
                done: false,
                totals: BiliDownloadTotals {
                    total: 0,
                    done: 0,
                    failed: 0,
                    skipped: 0,
                    canceled: 0,
                },
                jobs: vec![],
            }));
            let cancel = Arc::new(AtomicBool::new(false));

            let mgr = self.bili.clone();
            let status2 = status.clone();
            let cancel2 = cancel.clone();
            let handle = tokio::spawn(async move {
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

                let client = mgr.get_client().await;

                let mut auth = map_bili_auth_to_core(params.auth);
                if auth.cookie.is_some() && auth.refresh_token.is_some() {
                    if let Ok(a) = bili_video::auth::refresh_cookie_if_needed(&client, &auth).await {
                        auth = a;
                    }
                }

                let cookie = auth.cookie.as_deref();
                let vid = match bili_video::parse::parse_video_id(&params.input) {
                    Ok(v) => v,
                    Err(e) => {
                        let mut st = status2.lock().await;
                        st.done = true;
                        st.totals.failed = 1;
                        st.totals.total = 1;
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
                        return;
                    }
                };
                let view = match bili_video::parse::fetch_view_info(&client, &vid, cookie).await {
                    Ok(v) => v,
                    Err(e) => {
                        let mut st = status2.lock().await;
                        st.done = true;
                        st.totals.failed = 1;
                        st.totals.total = 1;
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
                        return;
                    }
                };

                let indices = bili_video::select_page::select_page_indices(
                    view.pages.len(),
                    &params.options.select_page,
                )
                .unwrap_or_else(|_| (0..view.pages.len()).collect());

                {
                    let mut st = status2.lock().await;
                    st.jobs = indices
                        .iter()
                        .enumerate()
                        .map(|(i, &pi)| {
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
                        })
                        .collect();
                    st.totals.total = st.jobs.len() as u32;
                }

                for (job_idx, &page_idx) in indices.iter().enumerate() {
                    if cancel2.load(Ordering::Relaxed) {
                        break;
                    }

                    let mut st = status2.lock().await;
                    if let Some(j) = st.jobs.get_mut(job_idx) {
                        j.state = BiliJobState::Running;
                        j.phase = BiliJobPhase::Parse;
                        j.bytes_downloaded = 0;
                        j.bytes_total = None;
                        j.speed_bps = None;
                        j.error = None;
                    }
                    drop(st);

                    let page = &view.pages[page_idx];

                    let base_play = match bili_video::playurl::fetch_playurl_dash(
                        &client,
                        &view.bvid,
                        &view.aid,
                        &page.cid,
                        0,
                        cookie,
                    )
                    .await
                    {
                        Ok(p) => p,
                        Err(e) => {
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) {
                                j.state = BiliJobState::Failed;
                                j.error = Some(e.to_string());
                            }
                            recompute_totals(&mut st);
                            continue;
                        }
                    };

                    let qn = bili_video::playurl::choose_qn_by_dfn_priority(
                        &base_play.accept_quality,
                        &base_play.accept_description,
                        &params.options.dfn_priority,
                    )
                    .unwrap_or(base_play.quality);

                    let play = if qn != base_play.quality {
                        bili_video::playurl::fetch_playurl_dash(
                            &client,
                            &view.bvid,
                            &view.aid,
                            &page.cid,
                            qn,
                            cookie,
                        )
                        .await
                        .unwrap_or(base_play)
                    } else {
                        base_play
                    };

                    let (v, a) = match bili_video::playurl::pick_dash_tracks(&play, &params.options.encoding_priority) {
                        Ok(x) => x,
                        Err(e) => {
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) {
                                j.state = BiliJobState::Failed;
                                j.error = Some(e.to_string());
                            }
                            recompute_totals(&mut st);
                            continue;
                        }
                    };

                    let mut dfn = play.quality.to_string();
                    for (i, q) in play.accept_quality.iter().enumerate() {
                        if *q == play.quality {
                            if let Some(desc) = play.accept_description.get(i) {
                                if !desc.trim().is_empty() {
                                    dfn = desc.trim().to_string();
                                }
                            }
                        }
                    }

                    let res = match (v.width, v.height) {
                        (Some(w), Some(h)) => format!("{w}x{h}"),
                        _ => page.dimension.clone().unwrap_or_default(),
                    };
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
                        std::path::Path::new(&out_dir),
                        &params.options.file_pattern,
                        &params.options.multi_file_pattern,
                        view.pages.len(),
                        &vars,
                        "mp4",
                    );

                    if out_mp4.exists() {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.state = BiliJobState::Skipped;
                            j.phase = BiliJobPhase::Parse;
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

                    // ----- video download -----
                    {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.phase = BiliJobPhase::Video;
                            j.bytes_downloaded = 0;
                            j.bytes_total = None;
                            j.speed_bps = None;
                        }
                    }
                    let prog_downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
                    let prog_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
                    let prog_has_total = Arc::new(std::sync::atomic::AtomicBool::new(false));
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
                    let dl = bili_video::download::download_to_file_ranged(
                        &client.http,
                        &v.base_url,
                        &headers,
                        &video_tmp,
                        params.options.concurrency,
                        params.options.retries,
                        true,
                        Some(&cancel2),
                        Some(cb),
                    );
                    tokio::pin!(dl);
                    let video_res = loop {
                        tokio::select! {
                            r = &mut dl => break r,
                            _ = tick.tick() => {
                                if cancel2.load(Ordering::Relaxed) { continue; }
                                let d = prog_downloaded.load(Ordering::Relaxed);
                                let t = prog_has_total.load(Ordering::Relaxed).then(|| prog_total.load(Ordering::Relaxed));
                                let mut st = status2.lock().await;
                                if let Some(j) = st.jobs.get_mut(job_idx) {
                                    j.bytes_downloaded = d;
                                    j.bytes_total = t;
                                }
                            }
                        }
                    };
                    if let Err(e) = video_res {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed };
                            j.error = Some(e.to_string());
                        }
                        recompute_totals(&mut st);
                        continue;
                    }

                    // ----- audio download -----
                    {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.phase = BiliJobPhase::Audio;
                            j.bytes_downloaded = 0;
                            j.bytes_total = None;
                            j.speed_bps = None;
                        }
                    }
                    let prog_downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
                    let prog_total = Arc::new(std::sync::atomic::AtomicU64::new(0));
                    let prog_has_total = Arc::new(std::sync::atomic::AtomicBool::new(false));
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
                    let dl = bili_video::download::download_to_file_ranged(
                        &client.http,
                        &a.base_url,
                        &headers,
                        &audio_tmp,
                        params.options.concurrency,
                        params.options.retries,
                        true,
                        Some(&cancel2),
                        Some(cb),
                    );
                    tokio::pin!(dl);
                    let audio_res = loop {
                        tokio::select! {
                            r = &mut dl => break r,
                            _ = tick.tick() => {
                                if cancel2.load(Ordering::Relaxed) { continue; }
                                let d = prog_downloaded.load(Ordering::Relaxed);
                                let t = prog_has_total.load(Ordering::Relaxed).then(|| prog_total.load(Ordering::Relaxed));
                                let mut st = status2.lock().await;
                                if let Some(j) = st.jobs.get_mut(job_idx) {
                                    j.bytes_downloaded = d;
                                    j.bytes_total = t;
                                }
                            }
                        }
                    };
                    if let Err(e) = audio_res {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed };
                            j.error = Some(e.to_string());
                        }
                        recompute_totals(&mut st);
                        continue;
                    }

                    // ----- subtitle download -----
                    let mut sub_paths: Vec<std::path::PathBuf> = Vec::new();
                    if params.options.download_subtitle && !cancel2.load(Ordering::Relaxed) {
                        {
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) {
                                j.phase = BiliJobPhase::Subtitle;
                            }
                        }
                        if let Ok(subs) = bili_video::subtitle::fetch_subtitles(&client, &view.bvid, &page.cid, cookie).await {
                            for s in subs {
                                if cancel2.load(Ordering::Relaxed) {
                                    break;
                                }
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
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.state = BiliJobState::Canceled;
                            j.error = Some("canceled".to_string());
                        }
                        recompute_totals(&mut st);
                        continue;
                    }

                    // ----- mux -----
                    if params.options.skip_mux {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.state = BiliJobState::Done;
                            j.phase = BiliJobPhase::Mux;
                            j.path = Some(out_mp4.to_string_lossy().to_string());
                        }
                        recompute_totals(&mut st);
                        continue;
                    }

                    {
                        let mut st = status2.lock().await;
                        if let Some(j) = st.jobs.get_mut(job_idx) {
                            j.state = BiliJobState::Muxing;
                            j.phase = BiliJobPhase::Mux;
                            j.bytes_downloaded = 0;
                            j.bytes_total = None;
                        }
                    }

                    let mux_res = bili_video::mux::mux_ffmpeg(
                        &params.options.ffmpeg_path,
                        &video_tmp,
                        &audio_tmp,
                        &sub_paths,
                        &out_mp4,
                        true,
                        Some(&cancel2),
                    )
                    .await;

                    // cleanup tmp
                    let _ = tokio::fs::remove_file(&video_tmp).await;
                    let _ = tokio::fs::remove_file(&audio_tmp).await;

                    match mux_res {
                        Ok(()) => {
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) {
                                j.state = BiliJobState::Done;
                                j.phase = BiliJobPhase::Mux;
                                j.path = Some(out_mp4.to_string_lossy().to_string());
                            }
                            recompute_totals(&mut st);
                        }
                        Err(e) => {
                            let mut st = status2.lock().await;
                            if let Some(j) = st.jobs.get_mut(job_idx) {
                                j.state = if cancel2.load(Ordering::Relaxed) { BiliJobState::Canceled } else { BiliJobState::Failed };
                                j.error = Some(e.to_string());
                            }
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
                let mut downloads = self.bili.downloads.lock().await;
                downloads.insert(
                    session_id.clone(),
                    BiliDownloadSession {
                        status,
                        cancel,
                        handle,
                    },
                );
            }

            Ok(BiliDownloadStartResult { session_id })
        }

        async fn bili_download_status(
            &self,
            params: BiliDownloadStatusParams,
        ) -> Result<BiliDownloadStatus, String> {
            let sid = params.session_id.trim().to_string();
            if sid.is_empty() {
                return Err("sessionId is empty".to_string());
            }
            let downloads = self.bili.downloads.lock().await;
            let Some(sess) = downloads.get(&sid) else {
                return Err("download session not found".to_string());
            };
            Ok(sess.status.lock().await.clone())
        }

        async fn bili_download_cancel(
            &self,
            params: BiliDownloadCancelParams,
        ) -> Result<OkReply, String> {
            let sid = params.session_id.trim().to_string();
            if sid.is_empty() {
                return Err("sessionId is empty".to_string());
            }

            let mut downloads = self.bili.downloads.lock().await;
            let Some(sess) = downloads.get_mut(&sid) else {
                return Err("download session not found".to_string());
            };

            sess.cancel.store(true, Ordering::Relaxed);
            sess.handle.abort();

            let mut st = sess.status.lock().await;
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

            Ok(OkReply { ok: true })
        }
    }

    pub async fn main() -> anyhow::Result<()> {
        // Single object that supports tokio::io::split for LSP framing.
        struct StdioTransport {
            stdin: Stdin,
            stdout: Stdout,
        }

        impl AsyncRead for StdioTransport {
            fn poll_read(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> Poll<std::io::Result<()>> {
                Pin::new(&mut self.stdin).poll_read(cx, buf)
            }
        }

        impl AsyncWrite for StdioTransport {
            fn poll_write(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<std::io::Result<usize>> {
                Pin::new(&mut self.stdout).poll_write(cx, buf)
            }

            fn poll_flush(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                Pin::new(&mut self.stdout).poll_flush(cx)
            }

            fn poll_shutdown(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                Pin::new(&mut self.stdout).poll_shutdown(cx)
            }
        }

        async fn run_stdio(auth_token: &str) -> anyhow::Result<()> {
            // When running over stdio, stdout must be reserved for JSON-RPC frames.
            // Any logs should go to stderr.
            let rw = StdioTransport {
                stdin: tokio::io::stdin(),
                stdout: tokio::io::stdout(),
            };

            let app = Arc::new(ChaosApp::new().map_err(|e| anyhow::anyhow!("{e}"))?);
            let music = Arc::new(MusicManager::new().map_err(|e| anyhow::anyhow!("{e}"))?);
            let bili = Arc::new(BiliManager::new().map_err(|e| anyhow::anyhow!("{e}"))?);
            let svc = Svc { app, music, bili };

            run_jsonrpc_over_lsp(&svc, rw, auth_token)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(())
        }

        async fn run_named_pipe(pipe_name: &str, auth_token: &str) -> anyhow::Result<()> {
            let full_name = if pipe_name.starts_with(r"\\.\pipe\") {
                pipe_name.to_string()
            } else {
                format!(r"\\.\pipe\{pipe_name}")
            };

            let server = tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(true)
                .create(full_name)?;
            server.connect().await?;

            let app = Arc::new(ChaosApp::new().map_err(|e| anyhow::anyhow!("{e}"))?);
            let music = Arc::new(MusicManager::new().map_err(|e| anyhow::anyhow!("{e}"))?);
            let bili = Arc::new(BiliManager::new().map_err(|e| anyhow::anyhow!("{e}"))?);
            let svc = Svc { app, music, bili };

            run_jsonrpc_over_lsp(&svc, server, auth_token)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(())
        }

        let opt = CliOptions::parse(env::args().skip(1)).map_err(|e| anyhow::anyhow!("{e}"))?;
        match opt.transport {
            TransportMode::Stdio => run_stdio(&opt.auth_token).await,
            TransportMode::NamedPipe { pipe_name } => run_named_pipe(&pipe_name, &opt.auth_token).await,
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("chaos-daemon is Windows-only. Build and run it on Windows.");
}

// Keep CLI parsing code covered (and avoid dead_code warnings) on non-Windows builds.
// This does not change runtime behavior since `chaos-daemon` still exits early on non-Windows.
#[cfg(not(windows))]
#[allow(dead_code)]
fn _cli_parse_smoke_test_for_non_windows_builds() {
    let _ = crate::cli::CliOptions::parse(["--stdio", "--auth-token", "token"]);
}

#[cfg(windows)]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    win::main().await
}
