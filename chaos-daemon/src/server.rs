use crate::lsp::{read_lsp_frame, write_lsp_frame};
use crate::rpc::{JsonRpcError, JsonRpcResponse, RpcErrorCode};
use chaos_proto::{
    DaemonPingParams, DaemonPingResult, DanmakuConnectParams, DanmakuConnectResult,
    DanmakuDisconnectParams, DanmakuFetchImageParams, LiveCloseParams, LiveDirCategoriesParams,
    LiveDirCategory, LiveDirCategoryRoomsParams, LiveDirRecommendRoomsParams,
    LiveDirRoomListResult, LiveDirSearchRoomsParams, LiveOpenParams,
    LivestreamDecodeManifestParams, LivestreamDecodeManifestResult, LyricsSearchParams,
    LyricsSearchResult, METHOD_DAEMON_PING, METHOD_DANMAKU_CONNECT, METHOD_DANMAKU_DISCONNECT,
    METHOD_DANMAKU_FETCH_IMAGE, METHOD_LIVE_CLOSE, METHOD_LIVE_DIR_CATEGORIES,
    METHOD_LIVE_DIR_CATEGORY_ROOMS, METHOD_LIVE_DIR_RECOMMEND_ROOMS, METHOD_LIVE_DIR_SEARCH_ROOMS,
    METHOD_LIVE_OPEN, METHOD_LIVESTREAM_DECODE_MANIFEST, METHOD_LYRICS_SEARCH,
    METHOD_NOW_PLAYING_SNAPSHOT, NOTIF_DANMAKU_MESSAGE, NowPlayingSnapshot,
    NowPlayingSnapshotParams,
    // music
    METHOD_MUSIC_ALBUM_TRACKS, METHOD_MUSIC_ARTIST_ALBUMS, METHOD_MUSIC_CONFIG_SET,
    METHOD_MUSIC_DOWNLOAD_CANCEL, METHOD_MUSIC_DOWNLOAD_START, METHOD_MUSIC_DOWNLOAD_STATUS,
    METHOD_MUSIC_KUGOU_LOGIN_QR_CREATE, METHOD_MUSIC_KUGOU_LOGIN_QR_POLL,
    METHOD_MUSIC_QQ_LOGIN_QR_CREATE, METHOD_MUSIC_QQ_LOGIN_QR_POLL, METHOD_MUSIC_QQ_REFRESH_COOKIE,
    METHOD_MUSIC_SEARCH_ALBUMS, METHOD_MUSIC_SEARCH_ARTISTS, METHOD_MUSIC_SEARCH_TRACKS,
    METHOD_MUSIC_TRACK_PLAY_URL,
    // bili
    METHOD_BILI_DOWNLOAD_CANCEL, METHOD_BILI_DOWNLOAD_START, METHOD_BILI_DOWNLOAD_STATUS,
    METHOD_BILI_LOGIN_QR_CREATE, METHOD_BILI_LOGIN_QR_POLL, METHOD_BILI_PARSE,
    METHOD_BILI_REFRESH_COOKIE,
    METHOD_BILI_CHECK_LOGIN, METHOD_BILI_LOGIN_QR_CREATE_V2, METHOD_BILI_LOGIN_QR_POLL_V2,
    METHOD_BILI_TASK_ADD, METHOD_BILI_TASK_CANCEL, METHOD_BILI_TASK_GET, METHOD_BILI_TASKS_GET,
    METHOD_BILI_TASKS_REMOVE_FINISHED,
    MusicAlbum, MusicAlbumTracksParams, MusicArtist, MusicArtistAlbumsParams, MusicDownloadCancelParams,
    MusicDownloadStartParams, MusicDownloadStartResult, MusicDownloadStatus, MusicDownloadStatusParams,
    MusicLoginQr, MusicLoginQrCreateParams, MusicLoginQrPollParams, MusicLoginQrPollResult,
    MusicProviderConfig, MusicRefreshCookieParams, MusicSearchParams, MusicTrack, OkReply, QqMusicCookie,
    MusicTrackPlayUrlParams, MusicTrackPlayUrlResult,
    BiliDownloadCancelParams, BiliDownloadStartParams, BiliDownloadStartResult, BiliDownloadStatus,
    BiliDownloadStatusParams, BiliLoginQr, BiliLoginQrCreateParams, BiliLoginQrPollParams,
    BiliLoginQrCreateV2Params, BiliLoginQrPollResult, BiliLoginQrPollResultV2, BiliParseParams,
    BiliParseResult, BiliRefreshCookieParams, BiliRefreshCookieResult,
    BiliCheckLoginParams, BiliCheckLoginResult,
    BiliTaskAddParams, BiliTaskAddResult, BiliTaskCancelParams, BiliTaskDetail,
    BiliTaskGetParams, BiliTasksGetParams, BiliTasksGetResult, BiliTasksRemoveFinishedParams,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::future::Future;
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::sync::mpsc;

pub trait ChaosService: Send + Sync + 'static {
    fn version(&self) -> String;

    fn livestream_decode_manifest(
        &self,
        params: LivestreamDecodeManifestParams,
    ) -> impl Future<Output = Result<LivestreamDecodeManifestResult, String>> + Send;

    fn live_dir_categories(
        &self,
        params: LiveDirCategoriesParams,
    ) -> impl Future<Output = Result<Vec<LiveDirCategory>, String>> + Send;

    fn live_dir_recommend_rooms(
        &self,
        params: LiveDirRecommendRoomsParams,
    ) -> impl Future<Output = Result<LiveDirRoomListResult, String>> + Send;

    fn live_dir_category_rooms(
        &self,
        params: LiveDirCategoryRoomsParams,
    ) -> impl Future<Output = Result<LiveDirRoomListResult, String>> + Send;

    fn live_dir_search_rooms(
        &self,
        params: LiveDirSearchRoomsParams,
    ) -> impl Future<Output = Result<LiveDirRoomListResult, String>> + Send;

    fn now_playing_snapshot(
        &self,
        params: NowPlayingSnapshotParams,
    ) -> impl Future<Output = Result<NowPlayingSnapshot, String>> + Send;

    fn lyrics_search(
        &self,
        params: LyricsSearchParams,
    ) -> impl Future<Output = Result<Vec<LyricsSearchResult>, String>> + Send;

    fn live_open(
        &self,
        params: LiveOpenParams,
    ) -> impl Future<
        Output = Result<
            (
                chaos_proto::LiveOpenResult,
                mpsc::UnboundedReceiver<chaos_proto::DanmakuMessage>,
            ),
            String,
        >,
    > + Send;

    fn live_close(
        &self,
        params: LiveCloseParams,
    ) -> impl Future<Output = Result<(), String>> + Send;

    fn danmaku_connect(
        &self,
        params: DanmakuConnectParams,
    ) -> impl Future<
        Output = Result<
            (
                DanmakuConnectResult,
                mpsc::UnboundedReceiver<chaos_proto::DanmakuMessage>,
            ),
            String,
        >,
    > + Send;

    fn danmaku_disconnect(
        &self,
        params: DanmakuDisconnectParams,
    ) -> impl Future<Output = Result<(), String>> + Send;

    fn danmaku_fetch_image(
        &self,
        params: DanmakuFetchImageParams,
    ) -> impl Future<Output = Result<chaos_proto::DanmakuFetchImageResult, String>> + Send;

    // ----- music -----
    fn music_config_set(
        &self,
        params: MusicProviderConfig,
    ) -> impl Future<Output = Result<OkReply, String>> + Send;

    fn music_search_tracks(
        &self,
        params: MusicSearchParams,
    ) -> impl Future<Output = Result<Vec<MusicTrack>, String>> + Send;

    fn music_search_albums(
        &self,
        params: MusicSearchParams,
    ) -> impl Future<Output = Result<Vec<MusicAlbum>, String>> + Send;

    fn music_search_artists(
        &self,
        params: MusicSearchParams,
    ) -> impl Future<Output = Result<Vec<MusicArtist>, String>> + Send;

    fn music_album_tracks(
        &self,
        params: MusicAlbumTracksParams,
    ) -> impl Future<Output = Result<Vec<MusicTrack>, String>> + Send;

    fn music_artist_albums(
        &self,
        params: MusicArtistAlbumsParams,
    ) -> impl Future<Output = Result<Vec<MusicAlbum>, String>> + Send;

    fn music_track_play_url(
        &self,
        params: MusicTrackPlayUrlParams,
    ) -> impl Future<Output = Result<MusicTrackPlayUrlResult, String>> + Send;

    fn music_qq_login_qr_create(
        &self,
        params: MusicLoginQrCreateParams,
    ) -> impl Future<Output = Result<MusicLoginQr, String>> + Send;

    fn music_qq_login_qr_poll(
        &self,
        params: MusicLoginQrPollParams,
    ) -> impl Future<Output = Result<MusicLoginQrPollResult, String>> + Send;

    fn music_qq_refresh_cookie(
        &self,
        params: MusicRefreshCookieParams,
    ) -> impl Future<Output = Result<QqMusicCookie, String>> + Send;

    fn music_kugou_login_qr_create(
        &self,
        params: MusicLoginQrCreateParams,
    ) -> impl Future<Output = Result<MusicLoginQr, String>> + Send;

    fn music_kugou_login_qr_poll(
        &self,
        params: MusicLoginQrPollParams,
    ) -> impl Future<Output = Result<MusicLoginQrPollResult, String>> + Send;

    fn music_download_start(
        &self,
        params: MusicDownloadStartParams,
    ) -> impl Future<Output = Result<MusicDownloadStartResult, String>> + Send;

    fn music_download_status(
        &self,
        params: MusicDownloadStatusParams,
    ) -> impl Future<Output = Result<MusicDownloadStatus, String>> + Send;

    fn music_download_cancel(
        &self,
        params: MusicDownloadCancelParams,
    ) -> impl Future<Output = Result<OkReply, String>> + Send;

    // ----- bilibili video (BV/AV) -----
    fn bili_login_qr_create(
        &self,
        params: BiliLoginQrCreateParams,
    ) -> impl Future<Output = Result<BiliLoginQr, String>> + Send;

    fn bili_login_qr_create_v2(
        &self,
        params: BiliLoginQrCreateV2Params,
    ) -> impl Future<Output = Result<BiliLoginQr, String>> + Send;

    fn bili_login_qr_poll(
        &self,
        params: BiliLoginQrPollParams,
    ) -> impl Future<Output = Result<BiliLoginQrPollResult, String>> + Send;

    fn bili_login_qr_poll_v2(
        &self,
        params: BiliLoginQrPollParams,
    ) -> impl Future<Output = Result<BiliLoginQrPollResultV2, String>> + Send;

    fn bili_check_login(
        &self,
        params: BiliCheckLoginParams,
    ) -> impl Future<Output = Result<BiliCheckLoginResult, String>> + Send;

    fn bili_refresh_cookie(
        &self,
        params: BiliRefreshCookieParams,
    ) -> impl Future<Output = Result<BiliRefreshCookieResult, String>> + Send;

    fn bili_parse(
        &self,
        params: BiliParseParams,
    ) -> impl Future<Output = Result<BiliParseResult, String>> + Send;

    fn bili_download_start(
        &self,
        params: BiliDownloadStartParams,
    ) -> impl Future<Output = Result<BiliDownloadStartResult, String>> + Send;

    fn bili_download_status(
        &self,
        params: BiliDownloadStatusParams,
    ) -> impl Future<Output = Result<BiliDownloadStatus, String>> + Send;

    fn bili_download_cancel(
        &self,
        params: BiliDownloadCancelParams,
    ) -> impl Future<Output = Result<OkReply, String>> + Send;

    // ----- bilibili tasks -----
    fn bili_task_add(
        &self,
        params: BiliTaskAddParams,
    ) -> impl Future<Output = Result<BiliTaskAddResult, String>> + Send;

    fn bili_tasks_get(
        &self,
        params: BiliTasksGetParams,
    ) -> impl Future<Output = Result<BiliTasksGetResult, String>> + Send;

    fn bili_task_get(
        &self,
        params: BiliTaskGetParams,
    ) -> impl Future<Output = Result<BiliTaskDetail, String>> + Send;

    fn bili_task_cancel(
        &self,
        params: BiliTaskCancelParams,
    ) -> impl Future<Output = Result<OkReply, String>> + Send;

    fn bili_tasks_remove_finished(
        &self,
        params: BiliTasksRemoveFinishedParams,
    ) -> impl Future<Output = Result<OkReply, String>> + Send;
}

#[derive(Debug, serde::Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

pub async fn run_jsonrpc_over_lsp<S: ChaosService, RW: AsyncRead + AsyncWrite + Unpin>(
    svc: &S,
    rw: RW,
    auth_token: &str,
) -> Result<(), crate::LspFrameError> {
    let (r, mut w) = tokio::io::split(rw);
    let mut br = BufReader::new(r);

    let mut authed = false;
    let mut active_live_session_id: Option<String> = None;
    let mut active_danmaku_session_id: Option<String> = None;

    let (notif_tx, mut notif_rx) = mpsc::unbounded_channel::<chaos_proto::DanmakuMessage>();
    let mut forwarders: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();

    fn abort_forwarder(
        forwarders: &mut HashMap<String, tokio::task::JoinHandle<()>>,
        session_id: &str,
    ) {
        if let Some(h) = forwarders.remove(session_id) {
            h.abort();
        }
    }

    loop {
        tokio::select! {
            biased;

            Some(msg) = notif_rx.recv() => {
                let payload = json!({
                    "jsonrpc": "2.0",
                    "method": NOTIF_DANMAKU_MESSAGE,
                    "params": msg,
                });
                let bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{}".to_vec());
                let _ = write_lsp_frame(&mut w, &bytes).await;
            }

            frame = read_lsp_frame(&mut br, 4 * 1024 * 1024) => {
                let frame = frame?;
                let req: JsonRpcRequest = match serde_json::from_slice(&frame) {
                    Ok(v) => v,
                    Err(_) => {
                        // Parse error: cannot reply without an id; drop.
                        continue;
                    }
                };

                if req.jsonrpc != "2.0" {
                    if let Some(id) = req.id {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InvalidRequest, "invalid jsonrpc version"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                    }
                    continue;
                }

                let Some(id) = req.id else {
                    // Notification from client: ignore for PoC.
                    continue;
                };

                if !authed {
                    if req.method != METHOD_DAEMON_PING {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::Unauthorized, "not authenticated"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                        continue;
                    }
                    let params: DaemonPingParams = match decode_params(req.params) {
                        Ok(v) => v,
                        Err(e) => {
                            let resp = JsonRpcResponse::err(id, e);
                            let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                            let _ = write_lsp_frame(&mut w, &bytes).await;
                            continue;
                        }
                    };
                    if params.auth_token != auth_token {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::Unauthorized, "invalid auth token"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                        break;
                    }
                    authed = true;
                    let result = DaemonPingResult { version: svc.version() };
                    let resp = JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap());
                    let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                    let _ = write_lsp_frame(&mut w, &bytes).await;
                    continue;
                }

                match req.method.as_str() {
                    METHOD_DAEMON_PING => {
                        let result = DaemonPingResult { version: svc.version() };
                        let resp = JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap());
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                    }
                    METHOD_NOW_PLAYING_SNAPSHOT => {
                        let params: NowPlayingSnapshotParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.now_playing_snapshot(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LYRICS_SEARCH => {
                        let params: LyricsSearchParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.lyrics_search(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_CONFIG_SET => {
                        let params: MusicProviderConfig = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_config_set(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_SEARCH_TRACKS => {
                        let params: MusicSearchParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_search_tracks(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_SEARCH_ALBUMS => {
                        let params: MusicSearchParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_search_albums(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_SEARCH_ARTISTS => {
                        let params: MusicSearchParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_search_artists(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_ALBUM_TRACKS => {
                        let params: MusicAlbumTracksParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_album_tracks(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_ARTIST_ALBUMS => {
                        let params: MusicArtistAlbumsParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_artist_albums(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_TRACK_PLAY_URL => {
                        let params: MusicTrackPlayUrlParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_track_play_url(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_QQ_LOGIN_QR_CREATE => {
                        let params: MusicLoginQrCreateParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_qq_login_qr_create(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_QQ_LOGIN_QR_POLL => {
                        let params: MusicLoginQrPollParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_qq_login_qr_poll(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_QQ_REFRESH_COOKIE => {
                        let params: MusicRefreshCookieParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_qq_refresh_cookie(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_KUGOU_LOGIN_QR_CREATE => {
                        let params: MusicLoginQrCreateParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_kugou_login_qr_create(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_KUGOU_LOGIN_QR_POLL => {
                        let params: MusicLoginQrPollParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_kugou_login_qr_poll(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_DOWNLOAD_START => {
                        let params: MusicDownloadStartParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_download_start(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_DOWNLOAD_STATUS => {
                        let params: MusicDownloadStatusParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_download_status(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_MUSIC_DOWNLOAD_CANCEL => {
                        let params: MusicDownloadCancelParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.music_download_cancel(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_LOGIN_QR_CREATE => {
                        let params: BiliLoginQrCreateParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_login_qr_create(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_LOGIN_QR_CREATE_V2 => {
                        let params: BiliLoginQrCreateV2Params = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_login_qr_create_v2(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_LOGIN_QR_POLL => {
                        let params: BiliLoginQrPollParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_login_qr_poll(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_LOGIN_QR_POLL_V2 => {
                        let params: BiliLoginQrPollParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_login_qr_poll_v2(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_CHECK_LOGIN => {
                        let params: BiliCheckLoginParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_check_login(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_REFRESH_COOKIE => {
                        let params: BiliRefreshCookieParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_refresh_cookie(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_PARSE => {
                        let params: BiliParseParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_parse(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_DOWNLOAD_START => {
                        let params: BiliDownloadStartParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_download_start(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_DOWNLOAD_STATUS => {
                        let params: BiliDownloadStatusParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_download_status(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_DOWNLOAD_CANCEL => {
                        let params: BiliDownloadCancelParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_download_cancel(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_TASK_ADD => {
                        let params: BiliTaskAddParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_task_add(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_TASKS_GET => {
                        let params: BiliTasksGetParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_tasks_get(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_TASK_GET => {
                        let params: BiliTaskGetParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_task_get(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_TASK_CANCEL => {
                        let params: BiliTaskCancelParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_task_cancel(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_BILI_TASKS_REMOVE_FINISHED => {
                        let params: BiliTasksRemoveFinishedParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.bili_tasks_remove_finished(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVESTREAM_DECODE_MANIFEST => {
                        let params: LivestreamDecodeManifestParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };

                        match svc.livestream_decode_manifest(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVE_DIR_CATEGORIES => {
                        let params: LiveDirCategoriesParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.live_dir_categories(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVE_DIR_RECOMMEND_ROOMS => {
                        let params: LiveDirRecommendRoomsParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.live_dir_recommend_rooms(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVE_DIR_CATEGORY_ROOMS => {
                        let params: LiveDirCategoryRoomsParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.live_dir_category_rooms(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVE_DIR_SEARCH_ROOMS => {
                        let params: LiveDirSearchRoomsParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.live_dir_search_rooms(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVE_OPEN => {
                        if let Some(prev) = active_live_session_id.take() {
                            abort_forwarder(&mut forwarders, &prev);
                            let _ = svc
                                .live_close(LiveCloseParams {
                                    session_id: prev.clone(),
                                })
                                .await;
                        }
                        let params: LiveOpenParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.live_open(params).await {
                            Ok((res, rx)) => {
                                let session_id = res.session_id.clone();
                                active_live_session_id = Some(session_id.clone());
                                let tx = notif_tx.clone();
                                let h = tokio::spawn(async move {
                                    let mut rx = rx;
                                    while let Some(msg) = rx.recv().await {
                                        let _ = tx.send(msg);
                                    }
                                });
                                forwarders.insert(session_id, h);
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVE_CLOSE => {
                        let params: LiveCloseParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        let sid = params.session_id.clone();
                        match svc.live_close(params).await {
                            Ok(()) => {
                                abort_forwarder(&mut forwarders, &sid);
                                if active_live_session_id.as_deref() == Some(&sid) {
                                    active_live_session_id = None;
                                }
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(chaos_proto::OkReply { ok: true }).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_DANMAKU_CONNECT => {
                        if let Some(prev) = active_danmaku_session_id.take() {
                            abort_forwarder(&mut forwarders, &prev);
                            let _ = svc
                                .danmaku_disconnect(DanmakuDisconnectParams {
                                    session_id: prev.clone(),
                                })
                                .await;
                        }
                        let params: DanmakuConnectParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.danmaku_connect(params).await {
                            Ok((res, rx)) => {
                                let session_id = res.session_id.clone();
                                active_danmaku_session_id = Some(session_id.clone());
                                let tx = notif_tx.clone();
                                let h = tokio::spawn(async move {
                                    let mut rx = rx;
                                    while let Some(msg) = rx.recv().await {
                                        let _ = tx.send(msg);
                                    }
                                });
                                forwarders.insert(session_id, h);
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_DANMAKU_DISCONNECT => {
                        let params: DanmakuDisconnectParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        let sid = params.session_id.clone();
                        match svc.danmaku_disconnect(params).await {
                            Ok(()) => {
                                abort_forwarder(&mut forwarders, &sid);
                                if active_danmaku_session_id.as_deref() == Some(&sid) {
                                    active_danmaku_session_id = None;
                                }
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(chaos_proto::OkReply { ok: true }).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_DANMAKU_FETCH_IMAGE => {
                        let params: DanmakuFetchImageParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.danmaku_fetch_image(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    _ => {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::MethodNotFound, "method not found"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                    }
                }
            }
        }
    }

    Ok(())
}

fn decode_params<T: DeserializeOwned>(params: Option<Value>) -> Result<T, JsonRpcError> {
    let Some(p) = params else {
        return Err(JsonRpcError::new(
            RpcErrorCode::InvalidParams,
            "missing params",
        ));
    };
    serde_json::from_value::<T>(p)
        .map_err(|_| JsonRpcError::new(RpcErrorCode::InvalidParams, "invalid params"))
}
