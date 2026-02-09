use std::collections::HashMap;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Site {
    BiliLive,
    Douyu,
    Huya,
}

impl Site {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BiliLive => "bili_live",
            Self::Douyu => "douyu",
            Self::Huya => "huya",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DanmakuMethod {
    SendDM,
    LiveDMServer,
}

#[derive(Debug, Clone, Serialize)]
pub struct DanmakuComment {
    pub text: String,
    pub image_url: Option<String>,
    pub image_width: Option<u32>,
}

impl DanmakuComment {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            image_url: None,
            image_width: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DanmakuEvent {
    pub site: Site,
    pub room_id: String,
    /// Unix epoch milliseconds (best-effort; `0` if clock is before epoch).
    pub received_at_ms: i64,
    pub method: DanmakuMethod,
    /// Best-effort display name (may be empty if the platform payload doesn't include it).
    pub user: String,
    pub text: String,
    pub dms: Option<Vec<DanmakuComment>>,
}

impl DanmakuEvent {
    pub fn new(
        site: Site,
        room_id: impl Into<String>,
        method: DanmakuMethod,
        text: impl Into<String>,
        dms: Option<Vec<DanmakuComment>>,
    ) -> Self {
        Self {
            site,
            room_id: room_id.into(),
            received_at_ms: std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            method,
            user: String::new(),
            text: text.into(),
            dms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmoticonMeta {
    pub unique: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum ConnectInfo {
    BiliLive {
        rid: u64,
        token: String,
        uid: u64,
        emoticons: HashMap<String, EmoticonMeta>,
    },
    Douyu {
        room_id: String,
    },
    Huya {
        room_id: String,
        yyuid: i64,
        uid: i64,
    },
}

#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub site: Site,
    /// User-facing room id, typically from the URL.
    pub room_id: String,
    pub connect: ConnectInfo,
}

#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// Default blocklist includes IINA+'s Douyu + Huya phrases.
    ///
    /// We only apply it to Douyu/Huya in this port.
    pub blocklist: Vec<String>,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            blocklist: vec![
                // Douyu
                "#挑战666#",
                "#签到",
                "#超管来了#",
                "#让火箭飞#",
                "#消消乐#",
                // Huya
                "分享了直播间，房间号",
                "录制并分享了小视频",
                "进入直播间",
                "刚刚在打赏君活动中",
                "竟然抽出了",
                "车队召集令在此",
                "微信公众号“虎牙志愿者”",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

pub type DanmakuEventTx = mpsc::UnboundedSender<DanmakuEvent>;
pub type DanmakuEventRx = mpsc::UnboundedReceiver<DanmakuEvent>;

pub struct DanmakuSession {
    pub(crate) cancel: CancellationToken,
    pub(crate) tasks: Vec<JoinHandle<()>>,
}

impl DanmakuSession {
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    pub async fn stop(self) {
        self.cancel.cancel();
        for t in self.tasks {
            let _ = t.await;
        }
    }
}

#[derive(Debug, Error)]
pub enum DanmakuError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error(
        "ambiguous input: {input}. Please provide a full URL or use a platform prefix like `bilibili:`, `douyu:`, `huya:`"
    )]
    AmbiguousInput { input: String },
    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("websocket error: {0}")]
    Ws(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("protobuf decode error: {0}")]
    Protobuf(#[from] prost::DecodeError),
    #[error("codec error: {0}")]
    Codec(String),
    #[error("parse error: {0}")]
    Parse(String),
}
