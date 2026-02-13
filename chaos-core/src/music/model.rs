use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};

fn de_opt_string_loose<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    // QQ login APIs sometimes return numeric IDs (e.g. musicid) as JSON numbers.
    // Internally we keep these fields as strings for cross-language (FFI/JSON-RPC) stability.
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    match v {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => {
            let s = s.trim().to_string();
            Ok(if s.is_empty() { None } else { Some(s) })
        }
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(serde_json::Value::Bool(b)) => Ok(Some(if b { "true" } else { "false" }.to_string())),
        Some(other) => Err(de::Error::custom(format!(
            "expected string/number, got {other}"
        ))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MusicService {
    Qq,
    Kugou,
    Netease,
    Kuwo,
}

impl MusicService {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Qq => "qq",
            Self::Kugou => "kugou",
            Self::Netease => "netease",
            Self::Kuwo => "kuwo",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicQuality {
    /// Stable id used by UI/daemon (e.g. "mp3_128", "mp3_320", "flac").
    pub id: String,
    pub label: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
    pub lossless: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// For "kugou" service. When empty, kugou provider is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kugou_base_url: Option<String>,

    /// For "netease" service: multiple base URLs separated by ';' in UI; daemon will normalize into Vec.
    #[serde(default)]
    pub netease_base_urls: Vec<String>,

    /// For "netease" anonymous login endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netease_anonymous_cookie_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqMusicCookie {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub openid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired_at: Option<i64>,

    /// `musicid` in refs
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub musicid: Option<String>,
    /// `musickey` in refs (`authst`)
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub musickey: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub musickey_create_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_login: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub refresh_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_type: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub str_musicid: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub nick: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub logo: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "de_opt_string_loose")]
    pub encrypt_uin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KugouUserInfo {
    pub token: String,
    pub userid: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qq: Option<QqMusicCookie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kugou: Option<KugouUserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netease_cookie: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MusicDownloadTarget {
    Track { track: MusicTrack },
    Album { service: MusicService, album_id: String },
    ArtistAll { service: MusicService, artist_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadOptions {
    pub quality_id: String,
    pub out_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_template: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_retries")]
    pub retries: u32,
}

const fn default_concurrency() -> u32 {
    3
}
const fn default_retries() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadTotals {
    pub total: u32,
    pub done: u32,
    pub failed: u32,
    pub skipped: u32,
    pub canceled: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MusicJobState {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicDownloadStatus {
    pub done: bool,
    pub totals: MusicDownloadTotals,
    pub jobs: Vec<MusicDownloadJobResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MusicLoginType {
    Qq,
    Wechat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicLoginQrState {
    Scan,
    Confirm,
    Done,
    Timeout,
    Refuse,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicLoginQr {
    pub session_id: String,
    pub login_type: MusicLoginType,
    pub mime: String,
    pub base64: String,
    /// Internal identifier (qrsig/uuid).
    pub identifier: String,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicLoginQrPollResult {
    pub session_id: String,
    pub state: MusicLoginQrState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<QqMusicCookie>,
}
