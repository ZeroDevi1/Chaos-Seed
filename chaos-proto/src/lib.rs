use serde::{Deserialize, Serialize};

pub const PIPE_NAME_PREFIX: &str = "chaos-seed-";

pub const METHOD_DAEMON_PING: &str = "daemon.ping";
pub const METHOD_LIVE_OPEN: &str = "live.open";
pub const METHOD_LIVE_CLOSE: &str = "live.close";
pub const METHOD_LIVESTREAM_DECODE_MANIFEST: &str = "livestream.decodeManifest";
pub const METHOD_DANMAKU_FETCH_IMAGE: &str = "danmaku.fetchImage";
pub const METHOD_DANMAKU_CONNECT: &str = "danmaku.connect";
pub const METHOD_DANMAKU_DISCONNECT: &str = "danmaku.disconnect";
pub const METHOD_NOW_PLAYING_SNAPSHOT: &str = "nowPlaying.snapshot";
pub const METHOD_LYRICS_SEARCH: &str = "lyrics.search";

// Music (search + login + download)
pub const METHOD_MUSIC_CONFIG_SET: &str = "music.config.set";
pub const METHOD_MUSIC_SEARCH_TRACKS: &str = "music.searchTracks";
pub const METHOD_MUSIC_SEARCH_ALBUMS: &str = "music.searchAlbums";
pub const METHOD_MUSIC_SEARCH_ARTISTS: &str = "music.searchArtists";
pub const METHOD_MUSIC_ALBUM_TRACKS: &str = "music.albumTracks";
pub const METHOD_MUSIC_ARTIST_ALBUMS: &str = "music.artistAlbums";
pub const METHOD_MUSIC_TRACK_PLAY_URL: &str = "music.trackPlayUrl";

pub const METHOD_MUSIC_QQ_LOGIN_QR_CREATE: &str = "music.qq.loginQrCreate";
pub const METHOD_MUSIC_QQ_LOGIN_QR_POLL: &str = "music.qq.loginQrPoll";
pub const METHOD_MUSIC_QQ_REFRESH_COOKIE: &str = "music.qq.refreshCookie";

pub const METHOD_MUSIC_KUGOU_LOGIN_QR_CREATE: &str = "music.kugou.loginQrCreate";
pub const METHOD_MUSIC_KUGOU_LOGIN_QR_POLL: &str = "music.kugou.loginQrPoll";

pub const METHOD_MUSIC_DOWNLOAD_START: &str = "music.download.start";
pub const METHOD_MUSIC_DOWNLOAD_STATUS: &str = "music.download.status";
pub const METHOD_MUSIC_DOWNLOAD_CANCEL: &str = "music.download.cancel";

// Bilibili video download (BV/AV) - MVP
pub const METHOD_BILI_LOGIN_QR_CREATE: &str = "bili.loginQrCreate";
pub const METHOD_BILI_LOGIN_QR_POLL: &str = "bili.loginQrPoll";
pub const METHOD_BILI_REFRESH_COOKIE: &str = "bili.refreshCookie";
pub const METHOD_BILI_PARSE: &str = "bili.parse";
pub const METHOD_BILI_DOWNLOAD_START: &str = "bili.download.start";
pub const METHOD_BILI_DOWNLOAD_STATUS: &str = "bili.download.status";
pub const METHOD_BILI_DOWNLOAD_CANCEL: &str = "bili.download.cancel";

pub const METHOD_LIVE_DIR_CATEGORIES: &str = "liveDir.categories";
pub const METHOD_LIVE_DIR_RECOMMEND_ROOMS: &str = "liveDir.recommendRooms";
pub const METHOD_LIVE_DIR_CATEGORY_ROOMS: &str = "liveDir.categoryRooms";
pub const METHOD_LIVE_DIR_SEARCH_ROOMS: &str = "liveDir.searchRooms";

pub const NOTIF_DANMAKU_MESSAGE: &str = "danmaku.message";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonPingParams {
    pub auth_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonPingResult {
    pub version: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PreferredQuality {
    Highest,
    Lowest,
}

impl Default for PreferredQuality {
    fn default() -> Self {
        Self::Highest
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveOpenParams {
    pub input: String,
    pub preferred_quality: Option<PreferredQuality>,
    pub variant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveOpenResult {
    pub session_id: String,
    pub site: String,
    pub room_id: String,
    pub title: String,
    pub variant_id: String,
    pub variant_label: String,
    pub url: String,
    pub backup_urls: Vec<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LivestreamDecodeManifestParams {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LivestreamInfo {
    pub title: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub cover: Option<String>,
    pub is_living: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LivestreamPlaybackHints {
    pub referer: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LivestreamVariant {
    pub id: String,
    pub label: String,
    pub quality: i32,
    pub rate: Option<i32>,
    pub url: Option<String>,
    pub backup_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LivestreamDecodeManifestResult {
    pub site: String,
    pub room_id: String,
    pub raw_input: String,
    pub info: LivestreamInfo,
    pub playback: LivestreamPlaybackHints,
    pub variants: Vec<LivestreamVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirCategoriesParams {
    pub site: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirRecommendRoomsParams {
    pub site: String,
    pub page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirCategoryRoomsParams {
    pub site: String,
    pub parent_id: Option<String>,
    pub category_id: String,
    pub page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirSearchRoomsParams {
    pub site: String,
    pub keyword: String,
    pub page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirSubCategory {
    pub id: String,
    pub parent_id: String,
    pub name: String,
    pub pic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirCategory {
    pub id: String,
    pub name: String,
    pub children: Vec<LiveDirSubCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirRoomCard {
    pub site: String,
    pub room_id: String,
    pub input: String,
    pub title: String,
    pub cover: Option<String>,
    pub user_name: Option<String>,
    pub online: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveDirRoomListResult {
    pub has_more: bool,
    pub items: Vec<LiveDirRoomCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveCloseParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OkReply {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DanmakuMessage {
    pub session_id: String,
    pub received_at_ms: i64,
    pub user: String,
    pub text: String,
    pub image_url: Option<String>,
    pub image_width: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DanmakuFetchImageParams {
    pub session_id: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DanmakuFetchImageResult {
    pub mime: String,
    pub base64: String,
    pub width: Option<u32>,
}

// -----------------------------
// Music DTOs
// -----------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MusicService {
    Qq,
    Kugou,
    Netease,
    Kuwo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicQuality {
    pub id: String,
    pub label: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
    pub lossless: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicTrack {
    pub service: MusicService,
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub artist_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    pub qualities: Vec<MusicQuality>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicAlbum {
    pub service: MusicService,
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicArtist {
    pub service: MusicService,
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_count: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kugou_base_url: Option<String>,
    #[serde(default)]
    pub netease_base_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netease_anonymous_cookie_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicSearchParams {
    pub service: MusicService,
    pub keyword: String,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicAlbumTracksParams {
    pub service: MusicService,
    pub album_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicArtistAlbumsParams {
    pub service: MusicService,
    pub artist_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicTrackPlayUrlParams {
    pub service: MusicService,
    pub track_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_id: Option<String>,
    #[serde(default)]
    pub auth: MusicAuthState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicTrackPlayUrlResult {
    pub url: String,
    pub ext: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QqMusicCookie {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired_at: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musickey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musickey_create_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_login: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_type: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub str_musicid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypt_uin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KugouUserInfo {
    pub token: String,
    pub userid: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicAuthState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qq: Option<QqMusicCookie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kugou: Option<KugouUserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netease_cookie: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MusicDownloadTarget {
    Track { track: MusicTrack },
    Album { service: MusicService, album_id: String },
    ArtistAll { service: MusicService, artist_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadOptions {
    pub quality_id: String,
    pub out_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_template: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default = "default_music_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_music_retries")]
    pub retries: u32,
}

const fn default_music_concurrency() -> u32 {
    3
}

const fn default_music_retries() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadStartParams {
    pub config: MusicProviderConfig,
    #[serde(default)]
    pub auth: MusicAuthState,
    pub target: MusicDownloadTarget,
    pub options: MusicDownloadOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadStartResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MusicJobState {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadTotals {
    pub total: u32,
    pub done: u32,
    pub failed: u32,
    pub skipped: u32,
    pub canceled: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadJobResult {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<String>,
    pub state: MusicJobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadStatus {
    pub done: bool,
    pub totals: MusicDownloadTotals,
    pub jobs: Vec<MusicDownloadJobResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadStatusParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadCancelParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MusicLoginType {
    Qq,
    Wechat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MusicLoginQrState {
    Scan,
    Confirm,
    Done,
    Timeout,
    Refuse,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicLoginQrCreateParams {
    pub login_type: MusicLoginType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicLoginQr {
    pub session_id: String,
    pub login_type: MusicLoginType,
    pub mime: String,
    pub base64: String,
    pub identifier: String,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MusicLoginQrPollParams {
    pub session_id: String,
}

// -----------------------------
// Bilibili video (BV/AV) download
// -----------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliAuthState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BiliLoginQrState {
    Scan,
    Confirm,
    Done,
    Timeout,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliLoginQr {
    pub session_id: String,
    pub mime: String,
    pub base64: String,
    pub url: String,
    pub qrcode_key: String,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BiliLoginQrCreateParams {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliLoginQrPollParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliLoginQrPollResult {
    pub session_id: String,
    pub state: BiliLoginQrState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<BiliAuthState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliRefreshCookieParams {
    pub auth: BiliAuthState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliRefreshCookieResult {
    pub auth: BiliAuthState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliParseParams {
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<BiliAuthState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliPage {
    pub page_number: u32,
    pub cid: String,
    pub page_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliParsedVideo {
    pub aid: String,
    pub bvid: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_mid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pub_time_unix_s: Option<i64>,
    pub pages: Vec<BiliPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliParseResult {
    pub videos: Vec<BiliParsedVideo>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BiliApiType {
    Auto,
    Web,
    Tv,
    App,
    Intl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadOptions {
    pub out_dir: String,
    pub select_page: String,
    pub dfn_priority: String,
    pub encoding_priority: String,
    pub file_pattern: String,
    pub multi_file_pattern: String,
    pub download_subtitle: bool,
    pub skip_mux: bool,
    pub concurrency: u32,
    pub retries: u32,
    pub ffmpeg_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadStartParams {
    pub api: BiliApiType,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<BiliAuthState>,
    pub options: BiliDownloadOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadStartResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BiliJobState {
    Pending,
    Running,
    Muxing,
    Done,
    Failed,
    Skipped,
    Canceled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BiliJobPhase {
    Parse,
    Video,
    Audio,
    Subtitle,
    Mux,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadJobStatus {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    pub title: String,
    pub state: BiliJobState,
    pub phase: BiliJobPhase,
    pub bytes_downloaded: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_bps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadTotals {
    pub total: u32,
    pub done: u32,
    pub failed: u32,
    pub skipped: u32,
    pub canceled: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadStatus {
    pub done: bool,
    pub totals: BiliDownloadTotals,
    pub jobs: Vec<BiliDownloadJobStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadStatusParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BiliDownloadCancelParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicLoginQrPollResult {
    pub session_id: String,
    pub state: MusicLoginQrState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<QqMusicCookie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kugou_user: Option<KugouUserInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicRefreshCookieParams {
    pub cookie: QqMusicCookie,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DanmakuConnectParams {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DanmakuConnectResult {
    pub session_id: String,
    pub site: String,
    pub room_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DanmakuDisconnectParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingSnapshotParams {
    #[serde(default)]
    pub include_thumbnail: Option<bool>,
    #[serde(default)]
    pub max_thumbnail_bytes: Option<u32>,
    #[serde(default)]
    pub max_sessions: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingThumbnail {
    pub mime: String,
    pub base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingSession {
    pub app_id: String,
    pub is_current: bool,
    pub playback_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<NowPlayingThumbnail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingSnapshot {
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub now_playing: Option<NowPlayingSession>,
    pub sessions: Vec<NowPlayingSession>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picked_app_id: Option<String>,
    pub retrieved_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchParams {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchResult {
    pub service: String,
    pub service_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub match_percentage: u8,
    pub quality: f64,
    pub matched: bool,
    pub has_translation: bool,
    pub has_inline_timetags: bool,
    pub lyrics_original: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics_translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<serde_json::Value>,
}
