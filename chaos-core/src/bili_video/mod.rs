use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use reqwest::Url;

pub mod auth;
pub mod download;
pub mod mux;
pub mod parse;
pub mod pgc;
pub mod playurl;
pub mod select_page;
pub mod subtitle;
pub mod template;

#[derive(Debug, thiserror::Error)]
pub enum BiliError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("api error: {0}")]
    Api(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("mux error: {0}")]
    Mux(String),
}

pub fn api_error_code(err: &BiliError) -> Option<i64> {
    let BiliError::Api(msg) = err else {
        return None;
    };
    let s = msg.trim();
    let rest = s.strip_prefix("code=")?;
    let num = rest.split(':').next()?.trim();
    num.parse::<i64>().ok()
}

impl From<reqwest::Error> for BiliError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.to_string())
    }
}

impl From<std::io::Error> for BiliError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct BiliEndpoints {
    pub api_base: String,      // https://api.bilibili.com
    pub passport_base: String, // https://passport.bilibili.com
    pub www_base: String,      // https://www.bilibili.com
}

impl Default for BiliEndpoints {
    fn default() -> Self {
        Self {
            api_base: "https://api.bilibili.com".to_string(),
            passport_base: "https://passport.bilibili.com".to_string(),
            www_base: "https://www.bilibili.com".to_string(),
        }
    }
}

pub fn default_user_agent() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
}

pub fn default_referer() -> &'static str {
    "https://www.bilibili.com/"
}

#[derive(Debug, Clone)]
struct WbiCache {
    mixin_key: String,
    fetched_at: Instant,
}

#[derive(Debug, Clone)]
struct BuvidCache {
    cookie: String,
    fetched_at: Instant,
}

#[derive(Debug, Clone)]
pub struct BiliClient {
    pub http: Client,
    pub endpoints: BiliEndpoints,
    pub timeout: Duration,
    wbi_cache: std::sync::Arc<std::sync::Mutex<Option<WbiCache>>>,
    buvid_cache: std::sync::Arc<std::sync::Mutex<Option<BuvidCache>>>,
    cookie_jar: std::sync::Arc<Jar>,
}

impl BiliClient {
    pub fn new() -> Result<Self, BiliError> {
        Self::with_endpoints(BiliEndpoints::default(), Duration::from_secs(18))
    }

    pub fn with_endpoints(
        endpoints: BiliEndpoints,
        timeout: Duration,
    ) -> Result<Self, BiliError> {
        let jar = std::sync::Arc::new(Jar::default());
        let http = Client::builder()
            .user_agent(default_user_agent())
            .timeout(timeout)
            .cookie_provider(jar.clone())
            .build()
            .map_err(|e| BiliError::Http(e.to_string()))?;
        Ok(Self {
            http,
            endpoints,
            timeout,
            wbi_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            buvid_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            cookie_jar: jar,
        })
    }

    pub fn buvid_cookie_cached(&self) -> Option<String> {
        const TTL: Duration = Duration::from_secs(24 * 3600);
        self.buvid_cache
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .and_then(|c| (c.fetched_at.elapsed() < TTL && !c.cookie.trim().is_empty()).then_some(c.cookie))
    }

    pub fn wbi_mixin_cached(&self) -> Option<String> {
        const TTL: Duration = Duration::from_secs(6 * 3600);
        self.wbi_cache
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .and_then(|c| (c.fetched_at.elapsed() < TTL && !c.mixin_key.trim().is_empty()).then_some(c.mixin_key))
    }

    pub(crate) fn set_buvid_cookie(&self, cookie: String) {
        if let Ok(mut g) = self.buvid_cache.lock() {
            *g = Some(BuvidCache {
                cookie,
                fetched_at: Instant::now(),
            });
        }
    }

    pub(crate) fn set_wbi_mixin(&self, mixin_key: String) {
        if let Ok(mut g) = self.wbi_cache.lock() {
            *g = Some(WbiCache {
                mixin_key,
                fetched_at: Instant::now(),
            });
        }
    }

    pub fn cookies_for_url(&self, url: &str) -> Option<String> {
        let u = Url::parse(url).ok()?;
        let hv = self.cookie_jar.cookies(&u)?;
        let s = hv.to_str().ok()?.trim();
        if s.is_empty() { None } else { Some(s.to_string()) }
    }

    pub fn cookies_for_www(&self) -> Option<String> {
        let base = self.endpoints.www_base.trim_end_matches('/');
        self.cookies_for_url(&format!("{base}/"))
    }
}

pub fn mask_cookie_for_log(cookie: &str) -> String {
    let c = cookie.trim();
    if c.is_empty() {
        return "<empty>".to_string();
    }
    let take = c.chars().take(6).collect::<String>();
    format!("{take}***")
}

pub fn cookie_get(cookie: &str, name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    for part in cookie.split(';') {
        let p = part.trim();
        if let Some((k, v)) = p.split_once('=') {
            if k.trim() == n {
                let vv = v.trim();
                if !vv.is_empty() {
                    return Some(vv.to_string());
                }
            }
        }
    }
    None
}

pub fn parse_cookie_kv(cookie: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for part in cookie.split(';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let Some((k, v)) = p.split_once('=') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim();
        if !k.is_empty() && !v.is_empty() {
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}

pub fn cookie_kv_to_string(kv: &BTreeMap<String, String>) -> String {
    kv.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn merge_cookie_strings(old_cookie: &str, new_cookie: &str) -> Option<String> {
    let mut kv = parse_cookie_kv(old_cookie);
    for (k, v) in parse_cookie_kv(new_cookie) {
        kv.insert(k, v);
    }
    let s = cookie_kv_to_string(&kv).trim().trim_end_matches(';').trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

pub fn extract_cookie_kv_from_url_query(url_s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let s = url_s.trim();
    if s.is_empty() {
        return out;
    }
    let Ok(url) = Url::parse(s) else {
        return out;
    };
    for (k, v) in url.query_pairs() {
        let k = k.trim();
        let v = v.trim();
        if k.is_empty() || v.is_empty() {
            continue;
        }
        if k.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}

pub fn merge_cookie_header(buvid_cookie: Option<&str>, cookie: Option<&str>) -> Option<String> {
    let mut out = String::new();
    if let Some(b) = buvid_cookie.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        out.push_str(b);
        if !out.ends_with(';') {
            out.push(';');
        }
        out.push(' ');
    }
    if let Some(c) = cookie.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        out.push_str(c);
    }
    let s = out.trim().trim_end_matches(';').trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

pub fn header_map_with_cookie(cookie: Option<&str>) -> HeaderMap {
    let mut h = HeaderMap::new();
    if let Some(c) = cookie.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if let Ok(v) = HeaderValue::from_str(c) {
            h.insert(reqwest::header::COOKIE, v);
        }
    }
    h.insert(reqwest::header::REFERER, HeaderValue::from_static(default_referer()));
    h
}

pub fn extract_set_cookie_kv(headers: &HeaderMap) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for v in headers.get_all(reqwest::header::SET_COOKIE).iter() {
        let Ok(s) = v.to_str() else { continue };
        let Some(first) = s.split(';').next() else { continue };
        let Some((k, val)) = first.split_once('=') else { continue };
        let k = k.trim();
        let val = val.trim();
        if !k.is_empty() && !val.is_empty() {
            out.insert(k.to_string(), val.to_string());
        }
    }
    out
}

pub fn build_cookie_string_from_set_cookie(headers: &HeaderMap) -> Option<String> {
    let kv = extract_set_cookie_kv(headers);
    if kv.is_empty() {
        return None;
    }
    Some(
        kv.into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

pub fn bili_check_code(json: &serde_json::Value) -> Result<(), BiliError> {
    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code == 0 {
        return Ok(());
    }
    let msg = json
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("bilibili api error")
        .trim()
        .to_string();
    Err(BiliError::Api(format!("code={code}: {msg}")))
}
