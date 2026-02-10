use serde::{Deserialize, Serialize};

pub const PIPE_NAME_PREFIX: &str = "chaos-seed-";

pub const METHOD_DAEMON_PING: &str = "daemon.ping";
pub const METHOD_LIVE_OPEN: &str = "live.open";
pub const METHOD_LIVE_CLOSE: &str = "live.close";
pub const METHOD_DANMAKU_FETCH_IMAGE: &str = "danmaku.fetchImage";

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
