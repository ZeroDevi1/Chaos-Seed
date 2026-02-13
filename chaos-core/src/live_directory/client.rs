use std::sync::{Arc, Mutex};
use std::time::Duration;

use thiserror::Error;

use crate::danmaku::model::Site;
use crate::livestream::client::EnvConfig;

use super::model::{LiveCategory, LiveRoomList};
use super::platforms;
use super::util::bili_wbi::BiliWbi;

#[derive(Debug, Error)]
pub enum LiveDirectoryError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("unsupported site")]
    UnsupportedSite,
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct LiveDirectoryEndpoints {
    pub bili_live_api_base: String, // api.live.bilibili.com
    pub bili_api_base: String,      // api.bilibili.com
    pub bili_live_base: String,     // live.bilibili.com
    pub huya_base: String,          // www.huya.com
    pub huya_live_cdn_base: String, // live.cdn.huya.com
    pub huya_search_base: String,   // search.cdn.huya.com
    pub douyu_base: String,         // www.douyu.com
    pub douyu_m_base: String,       // m.douyu.com
}

impl Default for LiveDirectoryEndpoints {
    fn default() -> Self {
        Self {
            bili_live_api_base: "https://api.live.bilibili.com".to_string(),
            bili_api_base: "https://api.bilibili.com".to_string(),
            bili_live_base: "https://live.bilibili.com".to_string(),
            huya_base: "https://www.huya.com".to_string(),
            huya_live_cdn_base: "https://live.cdn.huya.com".to_string(),
            huya_search_base: "https://search.cdn.huya.com".to_string(),
            douyu_base: "https://www.douyu.com".to_string(),
            douyu_m_base: "https://m.douyu.com".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct LiveDirectoryConfig {
    pub endpoints: LiveDirectoryEndpoints,
    pub env: EnvConfig,
    pub timeout: Duration,
}

impl std::fmt::Debug for LiveDirectoryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveDirectoryConfig")
            .field("endpoints", &self.endpoints)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Default for LiveDirectoryConfig {
    fn default() -> Self {
        Self {
            endpoints: LiveDirectoryEndpoints::default(),
            env: EnvConfig::real(),
            timeout: Duration::from_secs(10),
        }
    }
}

pub struct LiveDirectoryClient {
    pub(crate) http: reqwest::Client,
    pub(crate) cfg: LiveDirectoryConfig,
    pub(crate) bili_wbi: Arc<Mutex<BiliWbi>>,
}

impl LiveDirectoryClient {
    pub fn new() -> Result<Self, LiveDirectoryError> {
        Self::with_config(LiveDirectoryConfig::default())
    }

    pub fn with_config(cfg: LiveDirectoryConfig) -> Result<Self, LiveDirectoryError> {
        crate::tls::ensure_rustls_provider();
        let http = reqwest::Client::builder()
            .user_agent("chaos-seed/0.1")
            .timeout(cfg.timeout)
            .build()?;
        Ok(Self {
            http,
            cfg,
            bili_wbi: Arc::new(Mutex::new(BiliWbi::new())),
        })
    }

    pub async fn get_categories(
        &self,
        site: Site,
    ) -> Result<Vec<LiveCategory>, LiveDirectoryError> {
        platforms::get_categories(self, site).await
    }

    pub async fn get_recommend_rooms(
        &self,
        site: Site,
        page: u32,
    ) -> Result<LiveRoomList, LiveDirectoryError> {
        platforms::get_recommend_rooms(self, site, page).await
    }

    pub async fn get_category_rooms(
        &self,
        site: Site,
        parent_id: Option<&str>,
        category_id: &str,
        page: u32,
    ) -> Result<LiveRoomList, LiveDirectoryError> {
        platforms::get_category_rooms(self, site, parent_id, category_id, page).await
    }

    pub async fn search_rooms(
        &self,
        site: Site,
        keyword: &str,
        page: u32,
    ) -> Result<LiveRoomList, LiveDirectoryError> {
        platforms::search_rooms(self, site, keyword, page).await
    }
}
