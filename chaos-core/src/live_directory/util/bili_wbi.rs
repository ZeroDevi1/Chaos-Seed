use std::time::{Duration, Instant};

use crate::live_directory::client::{LiveDirectoryClient, LiveDirectoryError};

#[derive(Debug, Clone)]
pub struct WbiKeys {
    pub img_key: String,
    pub sub_key: String,
}

#[derive(Debug, Clone)]
pub struct Buvid {
    pub b_3: String,
    pub b_4: String,
}

#[derive(Debug)]
pub struct BiliWbi {
    keys: Option<(WbiKeys, Instant)>,
    access_id: Option<(String, Instant)>,
    buvid: Option<(Buvid, Instant)>,
}

impl BiliWbi {
    pub fn new() -> Self {
        Self {
            keys: None,
            access_id: None,
            buvid: None,
        }
    }

    pub fn mixin_key(origin_64: &str) -> Result<String, LiveDirectoryError> {
        // Ported from dart_simple_live.
        const TAB: [usize; 64] = [
            46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42,
            19, 29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60,
            51, 30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
        ];
        let s = origin_64.trim();
        if s.chars().count() < 64 {
            return Err(LiveDirectoryError::InvalidInput(
                "origin_64 must be >= 64 chars".to_string(),
            ));
        }
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::with_capacity(64);
        for i in TAB {
            if let Some(c) = chars.get(i) {
                out.push(*c);
            }
        }
        Ok(out.chars().take(32).collect())
    }

    pub fn filter_wbi_value(v: &str) -> String {
        v.chars()
            .filter(|c| !matches!(c, '!' | '\'' | '(' | ')' | '*'))
            .collect()
    }

    pub fn sign_query(
        params: &[(String, String)],
        mixin_key: &str,
        now_s: i64,
    ) -> Vec<(String, String)> {
        use std::collections::BTreeMap;

        let mut qp: BTreeMap<String, String> = BTreeMap::new();
        for (k, v) in params {
            qp.insert(k.clone(), v.clone());
        }
        qp.insert("wts".to_string(), now_s.to_string());

        // Sorted by key (BTreeMap).
        let mut query_pairs: Vec<(String, String)> = Vec::with_capacity(qp.len());
        for (k, v) in qp.iter() {
            query_pairs.push((k.clone(), Self::filter_wbi_value(v)));
        }

        // Build query string with percent-encoding.
        let query = query_pairs
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let sign_src = format!("{query}{mixin_key}");
        let w_rid = format!("{:x}", md5::compute(sign_src));

        let mut out: Vec<(String, String)> = qp.into_iter().collect();
        out.push(("w_rid".to_string(), w_rid));
        out
    }

    pub fn cached_keys(&self) -> Option<WbiKeys> {
        const TTL: Duration = Duration::from_secs(6 * 3600);
        self.keys.as_ref().and_then(|(k, at)| {
            (at.elapsed() < TTL && !k.img_key.is_empty() && !k.sub_key.is_empty())
                .then(|| k.clone())
        })
    }

    pub fn set_keys(&mut self, keys: WbiKeys) {
        self.keys = Some((keys, Instant::now()));
    }

    pub fn clear_keys(&mut self) {
        self.keys = None;
    }

    pub fn cached_access_id(&self) -> Option<String> {
        const TTL: Duration = Duration::from_secs(24 * 3600);
        self.access_id
            .as_ref()
            .and_then(|(id, at)| (at.elapsed() < TTL && !id.is_empty()).then(|| id.clone()))
    }

    pub fn set_access_id(&mut self, id: String) {
        self.access_id = Some((id, Instant::now()));
    }

    pub fn clear_access_id(&mut self) {
        self.access_id = None;
    }

    pub fn cached_buvid(&self) -> Option<Buvid> {
        const TTL: Duration = Duration::from_secs(24 * 3600);
        self.buvid.as_ref().and_then(|(b, at)| {
            (at.elapsed() < TTL && !b.b_3.is_empty() && !b.b_4.is_empty()).then(|| b.clone())
        })
    }

    pub fn set_buvid(&mut self, b: Buvid) {
        self.buvid = Some((b, Instant::now()));
    }

    pub fn clear_buvid(&mut self) {
        self.buvid = None;
    }

    pub fn cookie_from_buvid(b: &Buvid) -> String {
        format!("buvid3={};buvid4={};", b.b_3, b.b_4)
    }

    pub async fn ensure_buvid_cookie(client: &LiveDirectoryClient) -> Option<String> {
        let cached = client
            .bili_wbi
            .lock()
            .expect("bili_wbi mutex")
            .cached_buvid();
        if let Some(b) = cached {
            return Some(Self::cookie_from_buvid(&b));
        }

        let fetched = Self::fetch_buvid(client).await.ok()?;
        client
            .bili_wbi
            .lock()
            .expect("bili_wbi mutex")
            .set_buvid(fetched.clone());
        Some(Self::cookie_from_buvid(&fetched))
    }

    pub async fn fetch_keys(client: &LiveDirectoryClient) -> Result<WbiKeys, LiveDirectoryError> {
        let base = client.cfg.endpoints.bili_api_base.trim_end_matches('/');
        let url = format!("{base}/x/web-interface/nav");
        let mut req = client
            .http
            .get(&url)
            .header(reqwest::header::REFERER, "https://live.bilibili.com/")
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            );
        if let Some(cookie) = Self::ensure_buvid_cookie(client).await {
            req = req.header(reqwest::header::COOKIE, cookie);
        }
        let json = req
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        let img_url = json
            .pointer("/data/wbi_img/img_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sub_url = json
            .pointer("/data/wbi_img/sub_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let img_key = img_url
            .split('/')
            .last()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .to_string();
        let sub_key = sub_url
            .split('/')
            .last()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .to_string();
        if img_key.is_empty() || sub_key.is_empty() {
            return Err(LiveDirectoryError::Parse(
                "bili wbi keys missing".to_string(),
            ));
        }
        Ok(WbiKeys { img_key, sub_key })
    }

    pub async fn fetch_buvid(client: &LiveDirectoryClient) -> Result<Buvid, LiveDirectoryError> {
        let base = client.cfg.endpoints.bili_api_base.trim_end_matches('/');
        let url = format!("{base}/x/frontend/finger/spi");
        let json = client
            .http
            .get(&url)
            .header(reqwest::header::REFERER, "https://live.bilibili.com/")
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            )
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let b_3 = json
            .pointer("/data/b_3")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let b_4 = json
            .pointer("/data/b_4")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if b_3.is_empty() || b_4.is_empty() {
            return Err(LiveDirectoryError::Parse("bili buvid missing".to_string()));
        }
        Ok(Buvid { b_3, b_4 })
    }

    pub async fn fetch_access_id(
        client: &LiveDirectoryClient,
    ) -> Result<String, LiveDirectoryError> {
        let base = client.cfg.endpoints.bili_live_base.trim_end_matches('/');
        let url = format!("{base}/lol");
        let mut req = client
            .http
            .get(&url)
            .header(reqwest::header::REFERER, "https://live.bilibili.com/")
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            );
        if let Some(cookie) = Self::ensure_buvid_cookie(client).await {
            req = req.header(reqwest::header::COOKIE, cookie);
        }
        let text = req.send().await?.error_for_status()?.text().await?;
        let re = regex::Regex::new(r#""access_id":"(.*?)""#).expect("access_id regex");
        let id = re
            .captures(&text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().replace("\\", ""))
            .unwrap_or_default();
        Ok(id)
    }
}
