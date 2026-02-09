use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::danmaku::model::Site;
use crate::danmaku::sites::parse_target_hint;

use super::model::{LiveManifest, ResolveOptions, StreamVariant};
use super::platforms;

#[derive(Debug, Error)]
pub enum LivestreamError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("unsupported site")]
    UnsupportedSite,
    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("need password")]
    NeedPassword,
    #[error("need login")]
    NeedLogin,
}

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub bili_api_base: String,
    pub bili_live_base: String,
    pub douyu_base: String,
    pub huya_base: String,
    pub huya_mp_base: String,
    pub douyu_cdn_scheme: String,
    pub douyu_p2p_scheme: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            bili_api_base: "https://api.live.bilibili.com".to_string(),
            bili_live_base: "https://live.bilibili.com".to_string(),
            douyu_base: "https://www.douyu.com".to_string(),
            huya_base: "https://www.huya.com".to_string(),
            huya_mp_base: "https://mp.huya.com".to_string(),
            douyu_cdn_scheme: "https".to_string(),
            douyu_p2p_scheme: "https".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct EnvConfig {
    pub now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub now_s: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub rng: Arc<Mutex<fastrand::Rng>>,
}

impl std::fmt::Debug for EnvConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvConfig")
            .field("now_ms", &"<fn>")
            .field("now_s", &"<fn>")
            .field("rng", &"<rng>")
            .finish()
    }
}

impl EnvConfig {
    pub fn real() -> Self {
        fn now_ms() -> i64 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0)
        }
        fn now_s() -> i64 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        }
        Self {
            now_ms: Arc::new(now_ms),
            now_s: Arc::new(now_s),
            rng: Arc::new(Mutex::new(fastrand::Rng::new())),
        }
    }

    pub fn huya_uid(&self) -> u32 {
        // Ported from IINA+:
        // (time_ms % 1e10 * 1e3 + rand(1e2..1e3)) % 4294967295
        let t = (self.now_ms)();
        let base = ((t.rem_euclid(10_000_000_000)) as i64) * 1000;
        let r = self.rng.lock().expect("rng").i64(100..1000); // [100, 999]
        let v = (base + r).rem_euclid(4_294_967_295i64);
        v as u32
    }

    pub fn douyu_did(&self) -> String {
        // IINA+ uses drand48 + md5. Here we only need a stable pseudo-random DID.
        let n = self.rng.lock().expect("rng").u64(..);
        format!("{:x}", md5::compute(n.to_string()))
    }
}

#[derive(Clone)]
pub struct LivestreamConfig {
    pub endpoints: Endpoints,
    pub env: EnvConfig,
}

impl Default for LivestreamConfig {
    fn default() -> Self {
        Self {
            endpoints: Endpoints::default(),
            env: EnvConfig::real(),
        }
    }
}

pub struct LivestreamClient {
    pub(crate) http: reqwest::Client,
    pub(crate) cfg: LivestreamConfig,
}

impl LivestreamClient {
    pub fn new() -> Result<Self, LivestreamError> {
        Self::with_config(LivestreamConfig::default())
    }

    pub fn with_config(cfg: LivestreamConfig) -> Result<Self, LivestreamError> {
        let http = reqwest::Client::builder()
            .user_agent("chaos-seed/0.1")
            .build()?;
        Ok(Self { http, cfg })
    }

    pub async fn decode_manifest(
        &self,
        input: &str,
        opt: ResolveOptions,
    ) -> Result<LiveManifest, LivestreamError> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(LivestreamError::InvalidInput("empty input".to_string()));
        }
        let (site, room_id) =
            parse_target_hint(raw).map_err(|e| LivestreamError::InvalidInput(format!("{e}")))?;
        platforms::decode_manifest(&self.http, &self.cfg, site, &room_id, raw, opt).await
    }

    pub async fn resolve_variant(
        &self,
        site: Site,
        room_id: &str,
        variant_id: &str,
    ) -> Result<StreamVariant, LivestreamError> {
        let rid = room_id.trim();
        if rid.is_empty() {
            return Err(LivestreamError::InvalidInput("empty room_id".to_string()));
        }
        let vid = variant_id.trim();
        if vid.is_empty() {
            return Err(LivestreamError::InvalidInput(
                "empty variant_id".to_string(),
            ));
        }
        platforms::resolve_variant(&self.http, &self.cfg, site, rid, vid).await
    }
}
