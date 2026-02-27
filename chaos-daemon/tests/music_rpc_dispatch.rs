use chaos_daemon::{
    ChaosService, DaemonNotif, read_lsp_frame, run_jsonrpc_over_lsp, write_lsp_frame,
};
use chaos_proto::*;
use serde_json::{Value, json};
use tokio::io::BufReader;

struct TestSvc;

impl ChaosService for TestSvc {
    fn version(&self) -> String {
        "test".to_string()
    }

    async fn livestream_decode_manifest(
        &self,
        _params: LivestreamDecodeManifestParams,
    ) -> Result<LivestreamDecodeManifestResult, String> {
        Ok(LivestreamDecodeManifestResult {
            site: "bili_live".to_string(),
            room_id: "1".to_string(),
            raw_input: "x".to_string(),
            info: LivestreamInfo {
                title: "t".to_string(),
                name: None,
                avatar: None,
                cover: None,
                is_living: true,
            },
            playback: LivestreamPlaybackHints {
                referer: None,
                user_agent: None,
            },
            variants: vec![],
        })
    }

    async fn live_dir_categories(
        &self,
        _params: LiveDirCategoriesParams,
    ) -> Result<Vec<LiveDirCategory>, String> {
        Ok(vec![])
    }

    async fn live_dir_recommend_rooms(
        &self,
        _params: LiveDirRecommendRoomsParams,
    ) -> Result<LiveDirRoomListResult, String> {
        Ok(LiveDirRoomListResult {
            has_more: false,
            items: vec![],
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
            supported: false,
            now_playing: None,
            sessions: vec![],
            picked_app_id: None,
            retrieved_at_unix_ms: 0,
        })
    }

    async fn lyrics_search(
        &self,
        _params: LyricsSearchParams,
    ) -> Result<Vec<LyricsSearchResult>, String> {
        Ok(vec![])
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
        _notif_tx: tokio::sync::mpsc::UnboundedSender<DaemonNotif>,
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
    ) -> Result<
        (
            LiveOpenResult,
            tokio::sync::mpsc::UnboundedReceiver<DanmakuMessage>,
        ),
        String,
    > {
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok((
            LiveOpenResult {
                session_id: "s".to_string(),
                site: "bili_live".to_string(),
                room_id: "1".to_string(),
                title: "t".to_string(),
                variant_id: "v".to_string(),
                variant_label: "v".to_string(),
                url: "u".to_string(),
                backup_urls: vec![],
                referer: None,
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
            tokio::sync::mpsc::UnboundedReceiver<DanmakuMessage>,
        ),
        String,
    > {
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok((
            DanmakuConnectResult {
                session_id: "d".to_string(),
                site: "bili_live".to_string(),
                room_id: "1".to_string(),
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
            base64: "".to_string(),
            width: None,
        })
    }

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
            url: "http://example.invalid/x.mp3".to_string(),
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
            kugou_user: None,
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
            session_id: "sid".to_string(),
            mime: "image/png".to_string(),
            base64: "".to_string(),
            url: "https://example.invalid".to_string(),
            qrcode_key: "key".to_string(),
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

    async fn bili_login_qr_create_v2(
        &self,
        params: BiliLoginQrCreateV2Params,
    ) -> Result<BiliLoginQr, String> {
        // For tests, route to legacy behavior.
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

    async fn bili_parse(&self, params: BiliParseParams) -> Result<BiliParseResult, String> {
        Ok(BiliParseResult {
            videos: vec![BiliParsedVideo {
                aid: "1".to_string(),
                bvid: params.input,
                title: "t".to_string(),
                desc: None,
                pic: None,
                owner_name: None,
                owner_mid: None,
                pub_time_unix_s: None,
                pages: vec![],
            }],
        })
    }

    async fn bili_download_start(
        &self,
        _params: BiliDownloadStartParams,
    ) -> Result<BiliDownloadStartResult, String> {
        Ok(BiliDownloadStartResult {
            session_id: "bili_dl".to_string(),
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

async fn rpc_call(
    w: &mut (impl tokio::io::AsyncWrite + Unpin),
    r: &mut BufReader<impl tokio::io::AsyncRead + Unpin>,
    id: i64,
    method: &str,
    params: Value,
) -> Value {
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let bytes = serde_json::to_vec(&req).unwrap();
    write_lsp_frame(w, &bytes).await.unwrap();

    let frame = read_lsp_frame(r, 1024 * 1024).await.unwrap();
    serde_json::from_slice(&frame).unwrap()
}

#[tokio::test]
async fn music_methods_are_dispatchable() {
    let (client, server) = tokio::io::duplex(1024 * 1024);
    let (cr, mut cw) = tokio::io::split(client);
    let mut br = BufReader::new(cr);

    let server_task = tokio::spawn(async move {
        let svc = TestSvc;
        run_jsonrpc_over_lsp(&svc, server, "token").await
    });

    // ping(auth)
    let resp = rpc_call(
        &mut cw,
        &mut br,
        1,
        "daemon.ping",
        json!({ "authToken": "token" }),
    )
    .await;
    assert!(resp.get("result").is_some());

    // music.config.set
    let resp = rpc_call(&mut cw, &mut br, 2, "music.config.set", json!({})).await;
    assert_eq!(
        resp.pointer("/result/ok").and_then(|v| v.as_bool()),
        Some(true)
    );

    // music.searchTracks
    let resp = rpc_call(
        &mut cw,
        &mut br,
        3,
        "music.searchTracks",
        json!({ "service":"qq", "keyword":"k", "page":1, "pageSize":10 }),
    )
    .await;
    assert!(resp.get("result").is_some());

    // music.trackPlayUrl
    let resp = rpc_call(
        &mut cw,
        &mut br,
        4,
        "music.trackPlayUrl",
        json!({ "service":"qq", "trackId":"1" }),
    )
    .await;
    assert_eq!(
        resp.pointer("/result/ext").and_then(|v| v.as_str()),
        Some("mp3")
    );

    // bili.loginQrCreate
    let resp = rpc_call(&mut cw, &mut br, 5, "bili.loginQrCreate", json!({})).await;
    assert_eq!(
        resp.pointer("/result/sessionId").and_then(|v| v.as_str()),
        Some("sid")
    );

    server_task.abort();
}
