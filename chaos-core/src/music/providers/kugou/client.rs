use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use serde_json::Value;

use crate::music::error::MusicError;
use crate::music::model::AuthState;

use super::signatures;

const DEFAULT_APPID: &str = "1005";
const DEFAULT_CLIENTVER: &str = "20489";
const DEFAULT_SRCAPPID: &str = "2919";

const DEFAULT_GATEWAY_BASE: &str = "https://gateway.kugou.com";
const DEFAULT_LOGIN_BASE: &str = "https://login-user.kugou.com";
const DEFAULT_TRACKER_BASE: &str = "http://tracker.kugou.com";

const DEFAULT_UA: &str = "Android15-1070-11083-46-0-DiscoveryDRADProtocol-wifi";

#[derive(Debug, Clone)]
pub struct KugouEndpoints {
    pub gateway_base: String,
    pub login_base: String,
    pub tracker_base: String,
}

impl Default for KugouEndpoints {
    fn default() -> Self {
        Self {
            gateway_base: DEFAULT_GATEWAY_BASE.to_string(),
            login_base: DEFAULT_LOGIN_BASE.to_string(),
            tracker_base: DEFAULT_TRACKER_BASE.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EncryptType {
    Android,
    Web,
    Register,
}

#[derive(Debug, Clone)]
struct KugouDevice {
    dfid: String,
    mid: String,
    uuid: String,
}

fn now_unix_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn random_string(len: usize) -> String {
    const CHARS: &[u8] = b"1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    (0..len)
        .map(|_| {
            let idx = fastrand::usize(..CHARS.len());
            CHARS[idx] as char
        })
        .collect()
}

fn random_hex(len: usize) -> String {
    const CHARS: &[u8] = b"0123456789abcdef";
    (0..len)
        .map(|_| {
            let idx = fastrand::usize(..CHARS.len());
            CHARS[idx] as char
        })
        .collect()
}

fn calculate_mid(input: &str) -> String {
    // JS impl: md5(hex) interpreted as BigInt base16, then to decimal string.
    let hex = signatures::md5_hex_lower(input);
    let v = u128::from_str_radix(&hex, 16).unwrap_or(0);
    v.to_string()
}

fn device() -> &'static KugouDevice {
    static DEV: OnceLock<KugouDevice> = OnceLock::new();
    DEV.get_or_init(|| {
        let guid = random_hex(32);
        KugouDevice {
            dfid: random_string(24),
            mid: calculate_mid(&guid),
            uuid: "-".to_string(),
        }
    })
}

#[derive(Debug, Clone)]
pub struct KugouClient<'a> {
    http: &'a Client,
    endpoints: KugouEndpoints,
}

impl<'a> KugouClient<'a> {
    pub fn new(http: &'a Client) -> Self {
        Self {
            http,
            endpoints: KugouEndpoints::default(),
        }
    }

    pub fn with_endpoints(http: &'a Client, endpoints: KugouEndpoints) -> Self {
        Self { http, endpoints }
    }

    pub fn srcappid(&self) -> &'static str {
        DEFAULT_SRCAPPID
    }

    fn auth_token_userid(auth: Option<&AuthState>) -> (Option<String>, Option<String>) {
        let Some(a) = auth else {
            return (None, None);
        };
        let Some(u) = a.kugou.as_ref() else {
            return (None, None);
        };
        let token = u.token.trim();
        let userid = u.userid.trim();
        let token = if token.is_empty() { None } else { Some(token.to_string()) };
        let userid = if userid.is_empty() { None } else { Some(userid.to_string()) };
        (token, userid)
    }

    fn build_default_params(&self, auth: Option<&AuthState>) -> BTreeMap<String, String> {
        let dev = device();
        let mut params = BTreeMap::new();
        params.insert("dfid".to_string(), dev.dfid.clone());
        params.insert("mid".to_string(), dev.mid.clone());
        params.insert("uuid".to_string(), dev.uuid.clone());
        params.insert("appid".to_string(), DEFAULT_APPID.to_string());
        params.insert("clientver".to_string(), DEFAULT_CLIENTVER.to_string());
        params.insert("clienttime".to_string(), now_unix_s().to_string());

        let (token, userid) = Self::auth_token_userid(auth);
        if let Some(t) = token {
            params.insert("token".to_string(), t);
        }
        if let Some(u) = userid {
            if u != "0" {
                params.insert("userid".to_string(), u);
            }
        }

        params
    }

    fn build_headers(router: Option<&str>, kg_tid: Option<&str>, clienttime: &str) -> Result<HeaderMap, MusicError> {
        let dev = device();
        let mut h = HeaderMap::new();
        h.insert(reqwest::header::USER_AGENT, HeaderValue::from_static(DEFAULT_UA));
        h.insert("dfid", HeaderValue::from_str(&dev.dfid).map_err(|e| MusicError::Other(e.to_string()))?);
        h.insert("mid", HeaderValue::from_str(&dev.mid).map_err(|e| MusicError::Other(e.to_string()))?);
        h.insert("clienttime", HeaderValue::from_str(clienttime).map_err(|e| MusicError::Other(e.to_string()))?);
        h.insert("kg-rc", HeaderValue::from_static("1"));
        h.insert("kg-thash", HeaderValue::from_static("5d816a0"));
        h.insert("kg-rec", HeaderValue::from_static("1"));
        h.insert("kg-rf", HeaderValue::from_static("B9EDA08A64250DEFFBCADDEE00F8F25F"));
        if let Some(r) = router.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            h.insert("x-router", HeaderValue::from_str(r).map_err(|e| MusicError::Other(e.to_string()))?);
        }
        if let Some(t) = kg_tid.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            h.insert("kg-tid", HeaderValue::from_str(t).map_err(|e| MusicError::Other(e.to_string()))?);
        }
        Ok(h)
    }

    fn sign_params(params: &BTreeMap<String, String>, data: Option<&str>, encrypt: EncryptType) -> String {
        match encrypt {
            EncryptType::Android => signatures::signature_android_params(params, data),
            EncryptType::Web => signatures::signature_web_params(params),
            EncryptType::Register => signatures::signature_register_params(params),
        }
    }

    async fn request_get(
        &self,
        base: &str,
        path: &str,
        router: Option<&str>,
        kg_tid: Option<&str>,
        mut extra_params: BTreeMap<String, String>,
        encrypt: EncryptType,
        auth: Option<&AuthState>,
        timeout: Duration,
    ) -> Result<Value, MusicError> {
        let mut params = self.build_default_params(auth);
        params.append(&mut extra_params);
        let sig = Self::sign_params(&params, None, encrypt);
        params.insert("signature".to_string(), sig);

        let url = format!("{}{}", base.trim_end_matches('/'), path);
        let ct = params.get("clienttime").map(|s| s.as_str()).unwrap_or("0");
        let headers = Self::build_headers(router, kg_tid, ct)?;
        let resp = self
            .http
            .get(url)
            .headers(headers)
            .query(&params)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json::<Value>().await?)
    }

    async fn request_post(
        &self,
        base: &str,
        path: &str,
        router: Option<&str>,
        kg_tid: Option<&str>,
        data: &Value,
        extra_params: BTreeMap<String, String>,
        encrypt: EncryptType,
        auth: Option<&AuthState>,
        timeout: Duration,
    ) -> Result<Value, MusicError> {
        let mut params = self.build_default_params(auth);
        for (k, v) in extra_params {
            params.insert(k, v);
        }
        let data_str = serde_json::to_string(data).unwrap_or_default();
        let sig = Self::sign_params(&params, Some(&data_str), encrypt);
        params.insert("signature".to_string(), sig);

        let url = format!("{}{}", base.trim_end_matches('/'), path);
        let ct = params.get("clienttime").map(|s| s.as_str()).unwrap_or("0");
        let headers = Self::build_headers(router, kg_tid, ct)?;
        let resp = self
            .http
            .post(url)
            .headers(headers)
            .query(&params)
            .json(data)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json::<Value>().await?)
    }

    pub async fn gateway_get(
        &self,
        path: &str,
        router: &str,
        params: BTreeMap<String, String>,
        auth: Option<&AuthState>,
        timeout: Duration,
    ) -> Result<Value, MusicError> {
        self.request_get(&self.endpoints.gateway_base, path, Some(router), None, params, EncryptType::Android, auth, timeout)
            .await
    }

    pub async fn gateway_post(
        &self,
        path: &str,
        router: &str,
        kg_tid: Option<&str>,
        data: &Value,
        auth: Option<&AuthState>,
        timeout: Duration,
    ) -> Result<Value, MusicError> {
        self.request_post(
            &self.endpoints.gateway_base,
            path,
            Some(router),
            kg_tid,
            data,
            BTreeMap::new(),
            EncryptType::Android,
            auth,
            timeout,
        )
        .await
    }

    pub async fn login_user_get(
        &self,
        path: &str,
        params: BTreeMap<String, String>,
        timeout: Duration,
    ) -> Result<Value, MusicError> {
        self.request_get(&self.endpoints.login_base, path, None, None, params, EncryptType::Web, None, timeout)
            .await
    }

    pub async fn tracker_post(
        &self,
        path: &str,
        data: &Value,
        auth: Option<&AuthState>,
        timeout: Duration,
    ) -> Result<Value, MusicError> {
        self.request_post(
            &self.endpoints.tracker_base,
            path,
            None,
            None,
            data,
            BTreeMap::new(),
            EncryptType::Android,
            auth,
            timeout,
        )
        .await
    }

    pub fn now_ms(&self) -> i64 {
        now_unix_ms()
    }

    pub fn device_mid(&self) -> String {
        device().mid.clone()
    }

    pub fn appid(&self) -> &'static str {
        DEFAULT_APPID
    }
}
