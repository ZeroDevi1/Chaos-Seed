use chaos_daemon::{
    ChaosService, DaemonNotif, read_lsp_frame, run_jsonrpc_over_lsp, write_lsp_frame,
};
use chaos_proto::{
    BiliApiType,
    BiliCheckLoginParams,
    BiliCheckLoginResult,
    // bilibili video download (MVP)
    BiliDownloadCancelParams,
    BiliDownloadStartParams,
    BiliDownloadStartResult,
    BiliDownloadStatus,
    BiliDownloadStatusParams,
    BiliDownloadTotals,
    BiliLoginQr,
    BiliLoginQrCreateParams,
    BiliLoginQrCreateV2Params,
    BiliLoginQrPollParams,
    BiliLoginQrPollResult,
    BiliLoginQrPollResultV2,
    BiliLoginQrState,
    BiliParseParams,
    BiliParseResult,
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
    DanmakuConnectParams,
    DanmakuConnectResult,
    DanmakuDisconnectParams,
    DanmakuFetchImageParams,
    DanmakuFetchImageResult,
    DanmakuMessage,
    // music
    KugouUserInfo,
    LiveCloseParams,
    LiveDirCategoriesParams,
    LiveDirCategory,
    LiveDirCategoryRoomsParams,
    LiveDirRecommendRoomsParams,
    LiveDirRoomCard,
    LiveDirRoomListResult,
    LiveDirSearchRoomsParams,
    LiveDirSubCategory,
    LiveOpenParams,
    LiveOpenResult,
    LivestreamDecodeManifestParams,
    LivestreamDecodeManifestResult,
    LivestreamInfo,
    LivestreamPlaybackHints,
    LivestreamVariant,
    LlmChatParams,
    LlmChatResult,
    // llm + voice chat
    LlmConfigSetParams,
    LyricsSearchParams,
    LyricsSearchResult,
    MusicAlbum,
    MusicAlbumTracksParams,
    MusicArtist,
    MusicArtistAlbumsParams,
    MusicDownloadCancelParams,
    MusicDownloadStartParams,
    MusicDownloadStartResult,
    MusicDownloadStatus,
    MusicDownloadStatusParams,
    MusicDownloadTotals,
    MusicLoginQr,
    MusicLoginQrCreateParams,
    MusicLoginQrPollParams,
    MusicLoginQrPollResult,
    MusicLoginQrState,
    MusicProviderConfig,
    MusicRefreshCookieParams,
    MusicSearchParams,
    MusicTrack,
    MusicTrackPlayUrlParams,
    MusicTrackPlayUrlResult,
    NowPlayingSession,
    NowPlayingSnapshot,
    NowPlayingSnapshotParams,
    OkReply,
    QqMusicCookie,
    // tts
    TtsAudioResult,
    TtsJobState,
    TtsSftCancelParams,
    TtsSftStartParams,
    TtsSftStartResult,
    TtsSftStatus,
    TtsSftStatusParams,
    VoiceChatStreamCancelParams,
    VoiceChatStreamStartParams,
    VoiceChatStreamStartResult,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

struct FakeSvc {
    tx: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<DanmakuMessage>>>>,
}

impl FakeSvc {
    fn new() -> Self {
        Self {
            tx: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn push_msg(&self, msg: DanmakuMessage) {
        let tx = self
            .tx
            .lock()
            .expect("tx mutex")
            .get(&msg.session_id)
            .cloned();
        if let Some(tx) = tx {
            let _ = tx.send(msg);
        }
    }
}

impl ChaosService for FakeSvc {
    fn version(&self) -> String {
        "0.0.0-test".to_string()
    }

    async fn livestream_decode_manifest(
        &self,
        params: LivestreamDecodeManifestParams,
    ) -> Result<LivestreamDecodeManifestResult, String> {
        Ok(LivestreamDecodeManifestResult {
            site: "bili_live".to_string(),
            room_id: "1".to_string(),
            raw_input: params.input,
            info: LivestreamInfo {
                title: "t".to_string(),
                name: Some("n".to_string()),
                avatar: None,
                cover: Some("https://example.com/c.jpg".to_string()),
                is_living: true,
            },
            playback: LivestreamPlaybackHints {
                referer: Some("https://live.bilibili.com/1/".to_string()),
                user_agent: None,
            },
            variants: vec![LivestreamVariant {
                id: "v".to_string(),
                label: "原画".to_string(),
                quality: 10000,
                rate: None,
                url: None,
                backup_urls: vec![],
            }],
        })
    }

    async fn live_dir_categories(
        &self,
        _params: LiveDirCategoriesParams,
    ) -> Result<Vec<LiveDirCategory>, String> {
        Ok(vec![LiveDirCategory {
            id: "1".to_string(),
            name: "网游".to_string(),
            children: vec![LiveDirSubCategory {
                id: "11".to_string(),
                parent_id: "1".to_string(),
                name: "英雄联盟".to_string(),
                pic: None,
            }],
        }])
    }

    async fn live_dir_recommend_rooms(
        &self,
        _params: LiveDirRecommendRoomsParams,
    ) -> Result<LiveDirRoomListResult, String> {
        Ok(LiveDirRoomListResult {
            has_more: false,
            items: vec![LiveDirRoomCard {
                site: "bili_live".to_string(),
                room_id: "1".to_string(),
                input: "bilibili:1".to_string(),
                title: "t".to_string(),
                cover: None,
                user_name: None,
                online: Some(1),
            }],
        })
    }

    async fn live_dir_category_rooms(
        &self,
        _params: LiveDirCategoryRoomsParams,
    ) -> Result<LiveDirRoomListResult, String> {
        Ok(LiveDirRoomListResult {
            has_more: false,
            items: vec![],
        })
    }

    async fn live_dir_search_rooms(
        &self,
        _params: LiveDirSearchRoomsParams,
    ) -> Result<LiveDirRoomListResult, String> {
        Ok(LiveDirRoomListResult {
            has_more: false,
            items: vec![],
        })
    }

    async fn now_playing_snapshot(
        &self,
        _params: NowPlayingSnapshotParams,
    ) -> Result<NowPlayingSnapshot, String> {
        Ok(NowPlayingSnapshot {
            supported: true,
            now_playing: Some(NowPlayingSession {
                app_id: "app".to_string(),
                is_current: true,
                playback_status: "Playing".to_string(),
                title: Some("t".to_string()),
                artist: Some("a".to_string()),
                album_title: None,
                position_ms: Some(1000),
                duration_ms: Some(2000),
                genres: vec![],
                song_id: None,
                thumbnail: None,
                error: None,
            }),
            sessions: vec![],
            picked_app_id: Some("app".to_string()),
            retrieved_at_unix_ms: 1,
        })
    }

    async fn lyrics_search(
        &self,
        params: LyricsSearchParams,
    ) -> Result<Vec<LyricsSearchResult>, String> {
        Ok(vec![LyricsSearchResult {
            service: "qq".to_string(),
            service_token: "tok".to_string(),
            title: Some(params.title),
            artist: params.artist,
            album: params.album,
            duration_ms: params.duration_ms,
            match_percentage: 80,
            quality: 1.0,
            matched: true,
            has_translation: false,
            has_inline_timetags: true,
            lyrics_original: "[00:01.00]hello".to_string(),
            lyrics_translation: None,
            debug: None,
        }])
    }

    async fn tts_sft_start(&self, _params: TtsSftStartParams) -> Result<TtsSftStartResult, String> {
        Ok(TtsSftStartResult {
            session_id: "tts".to_string(),
        })
    }

    async fn tts_sft_status(&self, _params: TtsSftStatusParams) -> Result<TtsSftStatus, String> {
        Ok(TtsSftStatus {
            done: true,
            state: TtsJobState::Done,
            stage: Some("done".to_string()),
            error: None,
            result: Some(TtsAudioResult {
                mime: "audio/wav".to_string(),
                wav_base64: "".to_string(),
                sample_rate: 24000,
                channels: 1,
                duration_ms: 0,
            }),
        })
    }

    async fn tts_sft_cancel(&self, _params: TtsSftCancelParams) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }

    // ----- llm -----

    async fn llm_config_set(&self, _params: LlmConfigSetParams) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }

    async fn llm_chat(&self, params: LlmChatParams) -> Result<LlmChatResult, String> {
        // 测试用：回显最后一条消息（若无则返回空字符串）。
        let text = params
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        Ok(LlmChatResult { text })
    }

    // ----- voice chat stream -----

    async fn voice_chat_stream_start(
        &self,
        _params: VoiceChatStreamStartParams,
        _notif_tx: mpsc::UnboundedSender<DaemonNotif>,
    ) -> Result<VoiceChatStreamStartResult, String> {
        Ok(VoiceChatStreamStartResult {
            session_id: "voice_chat_test".to_string(),
            sample_rate: 24_000,
            channels: 1,
            format: "pcm16le".to_string(),
        })
    }

    async fn voice_chat_stream_cancel(
        &self,
        _params: VoiceChatStreamCancelParams,
    ) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }

    async fn live_open(
        &self,
        _params: LiveOpenParams,
    ) -> Result<(LiveOpenResult, mpsc::UnboundedReceiver<DanmakuMessage>), String> {
        let (tx, rx) = mpsc::unbounded_channel::<DanmakuMessage>();
        self.tx
            .lock()
            .expect("tx mutex")
            .insert("sess".to_string(), tx);
        Ok((
            LiveOpenResult {
                session_id: "sess".to_string(),
                site: "bili_live".to_string(),
                room_id: "1".to_string(),
                title: "t".to_string(),
                variant_id: "v".to_string(),
                variant_label: "lbl".to_string(),
                url: "https://example.com/x.m3u8".to_string(),
                backup_urls: vec![],
                referer: Some("https://live.bilibili.com/1/".to_string()),
                user_agent: None,
            },
            rx,
        ))
    }

    async fn live_close(&self, _params: LiveCloseParams) -> Result<(), String> {
        Ok(())
    }

    async fn danmaku_connect(
        &self,
        _params: DanmakuConnectParams,
    ) -> Result<
        (
            DanmakuConnectResult,
            mpsc::UnboundedReceiver<DanmakuMessage>,
        ),
        String,
    > {
        let (tx, rx) = mpsc::unbounded_channel::<DanmakuMessage>();
        self.tx
            .lock()
            .expect("tx mutex")
            .insert("dmsess".to_string(), tx);
        Ok((
            DanmakuConnectResult {
                session_id: "dmsess".to_string(),
                site: "bili_live".to_string(),
                room_id: "2".to_string(),
            },
            rx,
        ))
    }

    async fn danmaku_disconnect(&self, _params: DanmakuDisconnectParams) -> Result<(), String> {
        Ok(())
    }

    async fn danmaku_fetch_image(
        &self,
        _params: DanmakuFetchImageParams,
    ) -> Result<DanmakuFetchImageResult, String> {
        Ok(DanmakuFetchImageResult {
            mime: "image/png".to_string(),
            base64: "AA==".to_string(),
            width: Some(24),
        })
    }

    // ----- music -----

    async fn music_config_set(&self, _params: MusicProviderConfig) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }

    async fn music_search_tracks(
        &self,
        _params: MusicSearchParams,
    ) -> Result<Vec<MusicTrack>, String> {
        Ok(vec![])
    }

    async fn music_search_albums(
        &self,
        _params: MusicSearchParams,
    ) -> Result<Vec<MusicAlbum>, String> {
        Ok(vec![])
    }

    async fn music_search_artists(
        &self,
        _params: MusicSearchParams,
    ) -> Result<Vec<MusicArtist>, String> {
        Ok(vec![])
    }

    async fn music_album_tracks(
        &self,
        _params: MusicAlbumTracksParams,
    ) -> Result<Vec<MusicTrack>, String> {
        Ok(vec![])
    }

    async fn music_artist_albums(
        &self,
        _params: MusicArtistAlbumsParams,
    ) -> Result<Vec<MusicAlbum>, String> {
        Ok(vec![])
    }

    async fn music_track_play_url(
        &self,
        _params: MusicTrackPlayUrlParams,
    ) -> Result<MusicTrackPlayUrlResult, String> {
        Ok(MusicTrackPlayUrlResult {
            url: "https://example.com/a.mp3".to_string(),
            ext: "mp3".to_string(),
        })
    }

    async fn music_qq_login_qr_create(
        &self,
        params: MusicLoginQrCreateParams,
    ) -> Result<MusicLoginQr, String> {
        Ok(MusicLoginQr {
            session_id: "qq".to_string(),
            login_type: params.login_type,
            mime: "image/png".to_string(),
            base64: "".to_string(),
            identifier: "id".to_string(),
            created_at_unix_ms: 0,
        })
    }

    async fn music_qq_login_qr_poll(
        &self,
        params: MusicLoginQrPollParams,
    ) -> Result<MusicLoginQrPollResult, String> {
        Ok(MusicLoginQrPollResult {
            session_id: params.session_id,
            state: MusicLoginQrState::Scan,
            message: None,
            cookie: None,
            kugou_user: None,
        })
    }

    async fn music_qq_refresh_cookie(
        &self,
        params: MusicRefreshCookieParams,
    ) -> Result<QqMusicCookie, String> {
        Ok(params.cookie)
    }

    async fn music_kugou_login_qr_create(
        &self,
        params: MusicLoginQrCreateParams,
    ) -> Result<MusicLoginQr, String> {
        Ok(MusicLoginQr {
            session_id: "kugou".to_string(),
            login_type: params.login_type,
            mime: "image/png".to_string(),
            base64: "".to_string(),
            identifier: "id".to_string(),
            created_at_unix_ms: 0,
        })
    }

    async fn music_kugou_login_qr_poll(
        &self,
        params: MusicLoginQrPollParams,
    ) -> Result<MusicLoginQrPollResult, String> {
        Ok(MusicLoginQrPollResult {
            session_id: params.session_id,
            state: MusicLoginQrState::Scan,
            message: None,
            cookie: None,
            kugou_user: Some(KugouUserInfo {
                token: "t".to_string(),
                userid: "u".to_string(),
            }),
        })
    }

    async fn music_download_start(
        &self,
        _params: MusicDownloadStartParams,
    ) -> Result<MusicDownloadStartResult, String> {
        Ok(MusicDownloadStartResult {
            session_id: "dl".to_string(),
        })
    }

    async fn music_download_status(
        &self,
        _params: MusicDownloadStatusParams,
    ) -> Result<MusicDownloadStatus, String> {
        Ok(MusicDownloadStatus {
            done: true,
            totals: MusicDownloadTotals {
                total: 0,
                done: 0,
                failed: 0,
                skipped: 0,
                canceled: 0,
            },
            jobs: vec![],
        })
    }

    async fn music_download_cancel(
        &self,
        _params: MusicDownloadCancelParams,
    ) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }

    async fn bili_login_qr_create(
        &self,
        _params: BiliLoginQrCreateParams,
    ) -> Result<BiliLoginQr, String> {
        Ok(BiliLoginQr {
            session_id: "bili".to_string(),
            mime: "image/png".to_string(),
            base64: "".to_string(),
            url: "https://example.com/qr".to_string(),
            qrcode_key: "k".to_string(),
            created_at_unix_ms: 0,
        })
    }

    async fn bili_login_qr_poll(
        &self,
        params: BiliLoginQrPollParams,
    ) -> Result<BiliLoginQrPollResult, String> {
        Ok(BiliLoginQrPollResult {
            session_id: params.session_id,
            state: BiliLoginQrState::Scan,
            message: None,
            auth: None,
        })
    }

    async fn bili_refresh_cookie(
        &self,
        params: BiliRefreshCookieParams,
    ) -> Result<BiliRefreshCookieResult, String> {
        Ok(BiliRefreshCookieResult { auth: params.auth })
    }

    async fn bili_parse(&self, _params: BiliParseParams) -> Result<BiliParseResult, String> {
        Ok(BiliParseResult { videos: vec![] })
    }

    async fn bili_download_start(
        &self,
        _params: BiliDownloadStartParams,
    ) -> Result<BiliDownloadStartResult, String> {
        Ok(BiliDownloadStartResult {
            session_id: "bdl".to_string(),
        })
    }

    async fn bili_download_status(
        &self,
        _params: BiliDownloadStatusParams,
    ) -> Result<BiliDownloadStatus, String> {
        Ok(BiliDownloadStatus {
            done: true,
            totals: BiliDownloadTotals {
                total: 0,
                done: 0,
                failed: 0,
                skipped: 0,
                canceled: 0,
            },
            jobs: vec![],
        })
    }

    async fn bili_download_cancel(
        &self,
        _params: BiliDownloadCancelParams,
    ) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }

    async fn bili_login_qr_create_v2(
        &self,
        params: BiliLoginQrCreateV2Params,
    ) -> Result<BiliLoginQr, String> {
        let _ = params;
        self.bili_login_qr_create(BiliLoginQrCreateParams {}).await
    }

    async fn bili_login_qr_poll_v2(
        &self,
        params: BiliLoginQrPollParams,
    ) -> Result<BiliLoginQrPollResultV2, String> {
        Ok(BiliLoginQrPollResultV2 {
            session_id: params.session_id,
            state: BiliLoginQrState::Scan,
            message: None,
            auth: None,
        })
    }

    async fn bili_check_login(
        &self,
        _params: BiliCheckLoginParams,
    ) -> Result<BiliCheckLoginResult, String> {
        Ok(BiliCheckLoginResult {
            is_login: false,
            reason: Some("not implemented in tests".to_string()),
            missing_fields: vec![],
        })
    }

    async fn bili_task_add(&self, _params: BiliTaskAddParams) -> Result<BiliTaskAddResult, String> {
        Ok(BiliTaskAddResult {
            task_id: "t".to_string(),
        })
    }

    async fn bili_tasks_get(
        &self,
        _params: BiliTasksGetParams,
    ) -> Result<BiliTasksGetResult, String> {
        Ok(BiliTasksGetResult {
            running: vec![],
            finished: vec![],
        })
    }

    async fn bili_task_get(&self, params: BiliTaskGetParams) -> Result<BiliTaskDetail, String> {
        Ok(BiliTaskDetail {
            task: BiliTask {
                task_id: params.task_id,
                input: "".to_string(),
                api: BiliApiType::Auto,
                created_at_unix_ms: 0,
                done: true,
                totals: BiliDownloadTotals {
                    total: 0,
                    done: 0,
                    failed: 0,
                    skipped: 0,
                    canceled: 0,
                },
            },
            status: BiliDownloadStatus {
                done: true,
                totals: BiliDownloadTotals {
                    total: 0,
                    done: 0,
                    failed: 0,
                    skipped: 0,
                    canceled: 0,
                },
                jobs: vec![],
            },
        })
    }

    async fn bili_task_cancel(&self, _params: BiliTaskCancelParams) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }

    async fn bili_tasks_remove_finished(
        &self,
        _params: BiliTasksRemoveFinishedParams,
    ) -> Result<OkReply, String> {
        Ok(OkReply { ok: true })
    }
}

#[tokio::test]
async fn jsonrpc_request_response_and_notification_flow() {
    let svc = Arc::new(FakeSvc::new());
    let auth = "token";

    let (client, server) = tokio::io::duplex(64 * 1024);

    let svc2 = svc.clone();
    let server_task = tokio::spawn(async move {
        run_jsonrpc_over_lsp(&*svc2, server, auth).await.unwrap();
    });

    let (r, mut w) = tokio::io::split(client);
    let mut br = BufReader::new(r);

    // 1) ping
    let ping = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "daemon.ping",
        "params": { "authToken": auth }
    });
    let ping_bytes = serde_json::to_vec(&ping).unwrap();
    write_lsp_frame(&mut w, &ping_bytes).await.unwrap();
    let resp1 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 1024))
        .await
        .unwrap()
        .unwrap();
    let v1: serde_json::Value = serde_json::from_slice(&resp1).unwrap();
    assert_eq!(v1["id"], 1);
    assert_eq!(v1["result"]["version"], "0.0.0-test");

    // 2) nowPlaying.snapshot
    let now = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "nowPlaying.snapshot",
        "params": {}
    });
    let now_bytes = serde_json::to_vec(&now).unwrap();
    write_lsp_frame(&mut w, &now_bytes).await.unwrap();
    let resp10 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 16 * 1024))
        .await
        .unwrap()
        .unwrap();
    let v10: serde_json::Value = serde_json::from_slice(&resp10).unwrap();
    assert_eq!(v10["id"], 10);
    assert_eq!(v10["result"]["supported"], true);
    assert_eq!(v10["result"]["nowPlaying"]["title"], "t");

    // 3) lyrics.search
    let lyr = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "lyrics.search",
        "params": { "title": "Hello", "artist": "Adele" }
    });
    let lyr_bytes = serde_json::to_vec(&lyr).unwrap();
    write_lsp_frame(&mut w, &lyr_bytes).await.unwrap();
    let resp11 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 32 * 1024))
        .await
        .unwrap()
        .unwrap();
    let v11: serde_json::Value = serde_json::from_slice(&resp11).unwrap();
    assert_eq!(v11["id"], 11);
    assert!(v11["result"].is_array());
    assert_eq!(v11["result"][0]["matchPercentage"], 80);

    // 3.5) liveDir.categories
    let cats = json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "liveDir.categories",
        "params": { "site": "bili_live" }
    });
    let cats_bytes = serde_json::to_vec(&cats).unwrap();
    write_lsp_frame(&mut w, &cats_bytes).await.unwrap();
    let resp99 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 16 * 1024))
        .await
        .unwrap()
        .unwrap();
    let v99: serde_json::Value = serde_json::from_slice(&resp99).unwrap();
    assert_eq!(v99["id"], 99);
    assert!(v99["result"].is_array());
    assert_eq!(v99["result"][0]["children"][0]["id"], "11");

    // 4) danmaku.connect
    let dm = json!({
        "jsonrpc": "2.0",
        "id": 12,
        "method": "danmaku.connect",
        "params": { "input": "bilibili:2" }
    });
    let dm_bytes = serde_json::to_vec(&dm).unwrap();
    write_lsp_frame(&mut w, &dm_bytes).await.unwrap();
    let resp12 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v12: serde_json::Value = serde_json::from_slice(&resp12).unwrap();
    assert_eq!(v12["id"], 12);
    assert_eq!(v12["result"]["sessionId"], "dmsess");

    // 5) notification (danmaku session)
    svc.push_msg(DanmakuMessage {
        session_id: "dmsess".to_string(),
        received_at_ms: 1,
        user: "u".to_string(),
        text: "dm".to_string(),
        image_url: None,
        image_width: None,
    });
    let notif_dm = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let vndm: serde_json::Value = serde_json::from_slice(&notif_dm).unwrap();
    assert_eq!(vndm["method"], "danmaku.message");
    assert_eq!(vndm["params"]["sessionId"], "dmsess");
    assert_eq!(vndm["params"]["text"], "dm");

    // 6) live.open (should not disconnect danmaku)
    let open = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "live.open",
        "params": { "input": "bilibili:1", "preferredQuality": "highest" }
    });
    let open_bytes = serde_json::to_vec(&open).unwrap();
    write_lsp_frame(&mut w, &open_bytes).await.unwrap();
    let resp2 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v2: serde_json::Value = serde_json::from_slice(&resp2).unwrap();
    assert_eq!(v2["id"], 2);
    assert_eq!(v2["result"]["sessionId"], "sess");

    // 7) notification (live session)
    svc.push_msg(DanmakuMessage {
        session_id: "sess".to_string(),
        received_at_ms: 1,
        user: "u".to_string(),
        text: "hi".to_string(),
        image_url: None,
        image_width: None,
    });
    let notif_live = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let vnl: serde_json::Value = serde_json::from_slice(&notif_live).unwrap();
    assert_eq!(vnl["method"], "danmaku.message");
    assert_eq!(vnl["params"]["sessionId"], "sess");
    assert_eq!(vnl["params"]["text"], "hi");

    // 8) decode manifest
    let dec = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "livestream.decodeManifest",
        "params": { "input": "bilibili:1" }
    });
    let dec_bytes = serde_json::to_vec(&dec).unwrap();
    write_lsp_frame(&mut w, &dec_bytes).await.unwrap();
    let resp5 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 16 * 1024))
        .await
        .unwrap()
        .unwrap();
    let v5: serde_json::Value = serde_json::from_slice(&resp5).unwrap();
    assert_eq!(v5["id"], 5);
    assert_eq!(v5["result"]["site"], "bili_live");
    assert!(v5["result"]["variants"].is_array());

    // 9) danmaku session is still active after live.open
    svc.push_msg(DanmakuMessage {
        session_id: "dmsess".to_string(),
        received_at_ms: 2,
        user: "u2".to_string(),
        text: "dm2".to_string(),
        image_url: None,
        image_width: None,
    });
    let notif_dm2 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let vndm2: serde_json::Value = serde_json::from_slice(&notif_dm2).unwrap();
    assert_eq!(vndm2["method"], "danmaku.message");
    assert_eq!(vndm2["params"]["sessionId"], "dmsess");
    assert_eq!(vndm2["params"]["text"], "dm2");

    // 10) fetch image
    let img = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "danmaku.fetchImage",
        "params": { "sessionId": "sess", "url": "https://example.com/a.png" }
    });
    let img_bytes = serde_json::to_vec(&img).unwrap();
    write_lsp_frame(&mut w, &img_bytes).await.unwrap();
    let resp3 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v3: serde_json::Value = serde_json::from_slice(&resp3).unwrap();
    assert_eq!(v3["id"], 3);
    assert_eq!(v3["result"]["base64"], "AA==");

    // 11) close live
    let close = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "live.close",
        "params": { "sessionId": "sess" }
    });
    let close_bytes = serde_json::to_vec(&close).unwrap();
    write_lsp_frame(&mut w, &close_bytes).await.unwrap();
    let resp4 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v4: serde_json::Value = serde_json::from_slice(&resp4).unwrap();
    assert_eq!(v4["id"], 4);
    assert_eq!(v4["result"]["ok"], true);

    // 12) danmaku.disconnect
    let dm_close = json!({
        "jsonrpc": "2.0",
        "id": 13,
        "method": "danmaku.disconnect",
        "params": { "sessionId": "dmsess" }
    });
    let dm_close_bytes = serde_json::to_vec(&dm_close).unwrap();
    write_lsp_frame(&mut w, &dm_close_bytes).await.unwrap();
    let resp13 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v13: serde_json::Value = serde_json::from_slice(&resp13).unwrap();
    assert_eq!(v13["id"], 13);
    assert_eq!(v13["result"]["ok"], true);

    drop(w);
    drop(br);
    let _ = timeout(Duration::from_secs(3), server_task).await;
}
