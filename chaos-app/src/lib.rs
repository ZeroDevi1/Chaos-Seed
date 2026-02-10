mod cache;
mod danmaku_map;
mod image_fetch;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chaos_core::danmaku::client::DanmakuClient;
use chaos_core::danmaku::model::{ConnectOptions, DanmakuSession, Site};
use chaos_core::livestream::client::LivestreamClient;
use chaos_core::livestream::model::{ResolveOptions, StreamVariant};
use chaos_proto::{DanmakuFetchImageParams, DanmakuFetchImageResult, DanmakuMessage, LiveOpenResult};
use tokio::sync::mpsc;

use crate::cache::ByteLruCache;

#[derive(Debug, thiserror::Error)]
pub enum ChaosAppError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("livestream error: {0}")]
    Livestream(String),
    #[error("danmaku error: {0}")]
    Danmaku(String),
    #[error("session not found")]
    SessionNotFound,
    #[error("unsupported url scheme: {0}")]
    UnsupportedUrlScheme(String),
    #[error("blocked host")]
    BlockedHost,
    #[error("http error: {0}")]
    Http(String),
    #[error("image too large: {0} bytes")]
    ImageTooLarge(usize),
    #[error("base64 error: {0}")]
    Base64(String),
}

#[derive(Debug, Clone)]
pub struct ChaosAppConfig {
    pub image_timeout: Duration,
    pub image_max_bytes: usize,
    pub image_cache_max_entries: usize,
    pub image_cache_max_bytes: usize,
}

impl Default for ChaosAppConfig {
    fn default() -> Self {
        Self {
            image_timeout: Duration::from_secs(12),
            image_max_bytes: 2_500_000,
            image_cache_max_entries: 256,
            image_cache_max_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
struct LiveSessionMeta {
    site: Site,
    room_id: String,
}

struct LiveSession {
    meta: LiveSessionMeta,
    danmaku_session: DanmakuSession,
    reader_task: tokio::task::JoinHandle<()>,
}

pub struct ChaosApp {
    cfg: ChaosAppConfig,
    livestream: LivestreamClient,
    image_http: reqwest::Client,
    sessions: Arc<Mutex<HashMap<String, LiveSession>>>,
    image_cache: Arc<Mutex<ByteLruCache<(String, String), Vec<u8>>>>,
    image_mime_cache: Arc<Mutex<ByteLruCache<(String, String), String>>>,
}

impl std::fmt::Debug for ChaosApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChaosApp")
            .field("cfg", &self.cfg)
            .field("sessions", &"<sessions>")
            .field("image_cache", &"<cache>")
            .finish()
    }
}

impl ChaosApp {
    pub fn new() -> Result<Self, ChaosAppError> {
        Self::with_config(ChaosAppConfig::default())
    }

    pub fn with_config(cfg: ChaosAppConfig) -> Result<Self, ChaosAppError> {
        let livestream = LivestreamClient::new().map_err(|e| ChaosAppError::Livestream(e.to_string()))?;
        let image_http = reqwest::Client::builder()
            .user_agent("chaos-seed/0.1 (daemon)")
            .timeout(cfg.image_timeout)
            .build()
            .map_err(|e| ChaosAppError::Http(e.to_string()))?;

        Ok(Self {
            cfg: cfg.clone(),
            livestream,
            image_http,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            image_cache: Arc::new(Mutex::new(ByteLruCache::new(
                cfg.image_cache_max_entries,
                cfg.image_cache_max_bytes,
            ))),
            image_mime_cache: Arc::new(Mutex::new(ByteLruCache::new(
                cfg.image_cache_max_entries,
                cfg.image_cache_max_bytes / 16,
            ))),
        })
    }

    pub async fn open_live(
        &self,
        input: &str,
        prefer_lowest: bool,
    ) -> Result<(LiveOpenResult, mpsc::UnboundedReceiver<DanmakuMessage>), ChaosAppError> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(ChaosAppError::InvalidInput("empty input".to_string()));
        }

        let man = self
            .livestream
            .decode_manifest(raw, ResolveOptions::default())
            .await
            .map_err(|e| ChaosAppError::Livestream(e.to_string()))?;

        let (variant, url, backup_urls) = self
            .select_and_resolve_variant(man.site, &man.room_id, man.variants, prefer_lowest)
            .await?;

        let title = man.info.title.clone();
        let referer = man.playback.referer.clone();
        let user_agent = man.playback.user_agent.clone();

        let session_id = uuid_string();

        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<DanmakuMessage>();

        let session_id2 = session_id.clone();
        let connect_input = raw.to_string();

        let danmaku = DanmakuClient::new().map_err(|e| ChaosAppError::Danmaku(e.to_string()))?;
        let target = danmaku
            .resolve(&connect_input)
            .await
            .map_err(|e| ChaosAppError::Danmaku(e.to_string()))?;
        let (danmaku_session, mut rx) = danmaku
            .connect_resolved(target, ConnectOptions::default())
            .await
            .map_err(|e| ChaosAppError::Danmaku(e.to_string()))?;

        let reader_task = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                for msg in crate::danmaku_map::map_event_to_proto(session_id2.clone(), ev) {
                    let _ = msg_tx.send(msg);
                }
            }
        });

        {
            let mut sessions = self.sessions.lock().expect("sessions mutex");
            sessions.insert(
                session_id.clone(),
                LiveSession {
                    meta: LiveSessionMeta {
                        site: man.site,
                        room_id: man.room_id.clone(),
                    },
                    danmaku_session,
                    reader_task,
                },
            );
        }

        Ok((
            LiveOpenResult {
                session_id,
                site: man.site.as_str().to_string(),
                room_id: man.room_id,
                title,
                variant_id: variant.id,
                variant_label: variant.label,
                url,
                backup_urls,
                referer,
                user_agent,
            },
            msg_rx,
        ))
    }

    async fn select_and_resolve_variant(
        &self,
        site: Site,
        room_id: &str,
        variants: Vec<StreamVariant>,
        prefer_lowest: bool,
    ) -> Result<(StreamVariant, String, Vec<String>), ChaosAppError> {
        let mut vars = variants;
        vars.retain(|v| !v.id.trim().is_empty());
        if vars.is_empty() {
            return Err(ChaosAppError::Livestream("no variants".to_string()));
        }

        // Prefer URLs that Windows MediaPlayerElement is more likely to handle (m3u8/mp4).
        // Many livestream sources may be FLV and won't play without a custom player.
        if prefer_lowest {
            vars.sort_by_key(|v| v.quality);
        } else {
            vars.sort_by(|a, b| b.quality.cmp(&a.quality));
        }

        let mut fallback: Option<StreamVariant> = None;

        // 1) Already-resolved playable URLs.
        for v in &vars {
            let Some(u) = v.url.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
                continue;
            };
            if is_media_player_playable_url(u) {
                return Ok((v.clone(), u.to_string(), v.backup_urls.clone()));
            }
            if fallback.is_none() {
                fallback = Some(v.clone());
            }
        }

        // 2) Try resolving candidates until we find a playable URL.
        for v in &vars {
            let need_resolve = v.url.as_deref().unwrap_or_default().trim().is_empty();
            if !need_resolve {
                continue;
            }

            let resolved = self
                .livestream
                .resolve_variant(site, room_id, &v.id)
                .await
                .map_err(|e| ChaosAppError::Livestream(e.to_string()))?;
            let Some(u) = resolved
                .url
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            else {
                continue;
            };

            if is_media_player_playable_url(u) {
                return Ok((resolved.clone(), u.to_string(), resolved.backup_urls.clone()));
            }

            if fallback.is_none() {
                fallback = Some(resolved);
            }
        }

        let resolved = fallback.ok_or_else(|| ChaosAppError::Livestream("missing url".to_string()))?;
        let url = resolved
            .url
            .clone()
            .ok_or_else(|| ChaosAppError::Livestream("missing url".to_string()))?;
        Ok((resolved.clone(), url, resolved.backup_urls.clone()))
    }

    pub async fn close_live(&self, session_id: &str) -> Result<(), ChaosAppError> {
        let sess = {
            let mut sessions = self.sessions.lock().expect("sessions mutex");
            sessions.remove(session_id)
        };
        let Some(sess) = sess else {
            return Err(ChaosAppError::SessionNotFound);
        };
        sess.danmaku_session.stop().await;
        sess.reader_task.abort();
        let _ = sess.reader_task.await;
        Ok(())
    }

    pub async fn fetch_image(
        &self,
        params: DanmakuFetchImageParams,
    ) -> Result<DanmakuFetchImageResult, ChaosAppError> {
        let url_str = params.url.trim().to_string();
        if url_str.is_empty() {
            return Err(ChaosAppError::InvalidInput("empty url".to_string()));
        }
        let u = url::Url::parse(&url_str).map_err(|e| ChaosAppError::InvalidInput(e.to_string()))?;
        match u.scheme() {
            "http" | "https" => {}
            other => return Err(ChaosAppError::UnsupportedUrlScheme(other.to_string())),
        }
        if crate::image_fetch::is_local_or_private_host(&u) {
            return Err(ChaosAppError::BlockedHost);
        }

        let meta = {
            let sessions = self.sessions.lock().expect("sessions mutex");
            sessions
                .get(&params.session_id)
                .map(|s| s.meta.clone())
                .ok_or(ChaosAppError::SessionNotFound)?
        };

        let cache_key = (params.session_id.clone(), url_str.clone());
        if let Some(bytes) = self
            .image_cache
            .lock()
            .expect("cache mutex")
            .get(&cache_key)
        {
            let mime = self
                .image_mime_cache
                .lock()
                .expect("cache mutex")
                .get(&cache_key)
                .unwrap_or_else(|| "image/png".to_string());
            return Ok(crate::image_fetch::encode_image_reply(
                &bytes,
                &mime,
                None,
            )?);
        }

        let mut req = self.image_http.get(u.clone());
        if let Some(r) = crate::image_fetch::image_referer(Some(meta.site), Some(meta.room_id.clone()), &u) {
            req = req.header(reqwest::header::REFERER, r);
        }

        let resp = req.send().await.map_err(|e| ChaosAppError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ChaosAppError::Http(format!(
                "http {} when fetching image",
                resp.status()
            )));
        }

        if let Some(len) = resp.content_length() {
            if len as usize > self.cfg.image_max_bytes {
                return Err(ChaosAppError::ImageTooLarge(len as usize));
            }
        }

        let mime = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(';').next())
            .unwrap_or("image/png")
            .to_string();

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ChaosAppError::Http(e.to_string()))?
            .to_vec();
        if bytes.len() > self.cfg.image_max_bytes {
            return Err(ChaosAppError::ImageTooLarge(bytes.len()));
        }

        self.image_cache
            .lock()
            .expect("cache mutex")
            .insert(cache_key.clone(), bytes.clone(), bytes.len());
        self.image_mime_cache
            .lock()
            .expect("cache mutex")
            .insert(cache_key, mime.clone(), mime.len());

        crate::image_fetch::encode_image_reply(&bytes, &mime, None)
    }
}

fn uuid_string() -> String {
    // Avoid adding a uuid crate dependency for the PoC. This is not cryptographically strong.
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:x}", t, fastrand::u64(..))
}

fn is_media_player_playable_url(url: &str) -> bool {
    let u = url.trim().to_lowercase();
    // Rough heuristics: allow m3u8/mp4 and common query forms.
    u.contains(".m3u8")
        || u.contains(".mp4")
        || u.contains(".ism/")
        || u.contains(".ism?")
        || u.contains("manifest(format=m3u8)")
}

#[cfg(test)]
mod media_playable_tests {
    use super::is_media_player_playable_url;

    #[test]
    fn accepts_m3u8_and_mp4() {
        assert!(is_media_player_playable_url("https://a/b.m3u8"));
        assert!(is_media_player_playable_url("https://a/b.m3u8?x=1"));
        assert!(is_media_player_playable_url("https://a/b.mp4"));
    }

    #[test]
    fn rejects_flv() {
        assert!(!is_media_player_playable_url("https://a/b.flv"));
    }
}
