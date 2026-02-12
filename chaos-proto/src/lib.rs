use serde::{Deserialize, Serialize};

pub const PIPE_NAME_PREFIX: &str = "chaos-seed-";

pub const METHOD_DAEMON_PING: &str = "daemon.ping";
pub const METHOD_LIVE_OPEN: &str = "live.open";
pub const METHOD_LIVE_CLOSE: &str = "live.close";
pub const METHOD_LIVESTREAM_DECODE_MANIFEST: &str = "livestream.decodeManifest";
pub const METHOD_DANMAKU_FETCH_IMAGE: &str = "danmaku.fetchImage";
pub const METHOD_NOW_PLAYING_SNAPSHOT: &str = "nowPlaying.snapshot";
pub const METHOD_LYRICS_SEARCH: &str = "lyrics.search";

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
