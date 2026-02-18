use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use image::ImageFormat;
use qrcode::QrCode;
use regex::Regex;
use reqwest::header::HeaderMap;
use rsa::pkcs8::DecodePublicKey;
use rsa::{Oaep, RsaPublicKey};
use rsa::rand_core::OsRng;
use serde_json::Value;
use sha2::Sha256;

use super::{
    BiliClient, BiliError, bili_check_code, build_cookie_string_from_set_cookie, cookie_get,
    header_map_with_cookie, merge_cookie_header,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthState {
    pub cookie: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginQrState {
    Scan,
    Confirm,
    Done,
    Timeout,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginQr {
    pub url: String,
    pub qrcode_key: String,
    pub mime: String,
    pub base64: String,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginQrPollResult {
    pub state: LoginQrState,
    pub message: Option<String>,
    pub auth: Option<AuthState>,
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub async fn login_qr_create(client: &BiliClient) -> Result<LoginQr, BiliError> {
    let url = format!(
        "{}/x/passport-login/web/qrcode/generate?source=main-fe-header",
        client.endpoints.passport_base.trim_end_matches('/')
    );

    let buvid = super::playurl::ensure_buvid_cookie(client).await.ok();
    let headers = header_map_with_cookie(super::merge_cookie_header(buvid.as_deref(), None).as_deref());

    let json: Value = client.http.get(url).headers(headers).send().await?.json().await?;
    bili_check_code(&json)?;

    let data = json.get("data").ok_or_else(|| BiliError::Parse("missing data".to_string()))?;
    let login_url = data
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let qrcode_key = data
        .get("qrcode_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if login_url.is_empty() || qrcode_key.is_empty() {
        return Err(BiliError::Parse("missing url/qrcode_key".to_string()));
    }

    let code = QrCode::new(login_url.as_bytes()).map_err(|e| BiliError::Parse(e.to_string()))?;
    let img = code.render::<image::Luma<u8>>().max_dimensions(320, 320).build();
    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let mut buf: Vec<u8> = Vec::new();
    dyn_img
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| BiliError::Parse(e.to_string()))?;
    let base64 = base64::engine::general_purpose::STANDARD.encode(buf);

    Ok(LoginQr {
        url: login_url,
        qrcode_key,
        mime: "image/png".to_string(),
        base64,
        created_at_unix_ms: now_unix_ms(),
    })
}

pub async fn login_qr_poll(client: &BiliClient, qrcode_key: &str) -> Result<LoginQrPollResult, BiliError> {
    let key = qrcode_key.trim();
    if key.is_empty() {
        return Err(BiliError::InvalidInput("empty qrcode_key".to_string()));
    }

    let url = format!(
        "{}/x/passport-login/web/qrcode/poll?qrcode_key={}&source=main-fe-header",
        client.endpoints.passport_base.trim_end_matches('/'),
        urlencoding::encode(key)
    );

    let buvid = super::playurl::ensure_buvid_cookie(client).await.ok();
    let headers = header_map_with_cookie(super::merge_cookie_header(buvid.as_deref(), None).as_deref());

    let resp = client.http.get(url).headers(headers).send().await?;
    let headers_resp = resp.headers().clone();
    let json: Value = resp.json().await?;
    bili_check_code(&json)?;

    let data = json.get("data").ok_or_else(|| BiliError::Parse("missing data".to_string()))?;
    let code = data.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let refresh_token = data.get("refresh_token").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();

    match code {
        86101 => Ok(LoginQrPollResult { state: LoginQrState::Scan, message: None, auth: None }),
        86090 => Ok(LoginQrPollResult { state: LoginQrState::Confirm, message: Some("scan ok, waiting confirm".to_string()), auth: None }),
        86038 => Ok(LoginQrPollResult { state: LoginQrState::Timeout, message: Some("qrcode timeout".to_string()), auth: None }),
        0 => {
            let cookie = build_cookie_string_from_set_cookie(&headers_resp);
            if cookie.as_deref().unwrap_or("").trim().is_empty() || refresh_token.is_empty() {
                return Ok(LoginQrPollResult {
                    state: LoginQrState::Other,
                    message: Some("login ok but missing cookie/refresh_token".to_string()),
                    auth: None,
                });
            }
            Ok(LoginQrPollResult {
                state: LoginQrState::Done,
                message: None,
                auth: Some(AuthState {
                    cookie,
                    refresh_token: Some(refresh_token),
                }),
            })
        }
        _ => Ok(LoginQrPollResult { state: LoginQrState::Other, message: Some(format!("unknown code={code}")), auth: None }),
    }
}

fn public_key_pem() -> &'static str {
    // Source: BAC Document (bilibili-API-collect) cookie refresh correspondPath public key.
    "-----BEGIN PUBLIC KEY-----\n\
MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDLgd2OAkcGVtoE3ThUREbio0Eg\n\
Uc/prcajMKXvkCKFCWhJYJcLkcM2DKKcSeFpD/j6Boy538YXnR6VhcuUJOhH2x71\n\
nzPjfdTcqMz7djHum0qSZA0AyCBDABUqCrfNgCiJ00Ra7GmRj+YCK1NJEuewlb40\n\
JNrRuoEUXpabUzGB8QIDAQAB\n\
-----END PUBLIC KEY-----"
}

fn correspond_path(timestamp_ms: i64) -> Result<String, BiliError> {
    let pub_key = RsaPublicKey::from_public_key_pem(public_key_pem())
        .map_err(|e| BiliError::Crypto(e.to_string()))?;
    let msg = format!("refresh_{timestamp_ms}");
    let padding = Oaep::new::<Sha256>();
    let enc = pub_key
        .encrypt(&mut OsRng, padding, msg.as_bytes())
        .map_err(|e| BiliError::Crypto(e.to_string()))?;
    Ok(hex::encode(enc))
}

fn extract_refresh_csrf(html: &str) -> Option<String> {
    // <div id="1-name">refresh_csrf</div>
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"<div\s+id=["']1-name["']\s*>\s*([0-9a-fA-F]+)\s*</div>"#).unwrap()
    });
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

async fn cookie_info_needs_refresh(
    client: &BiliClient,
    cookie_hdr: &str,
    csrf: &str,
) -> Result<Option<i64>, BiliError> {
    let url = format!(
        "{}/x/passport-login/web/cookie/info?csrf={}",
        client.endpoints.passport_base.trim_end_matches('/'),
        urlencoding::encode(csrf)
    );
    let headers = header_map_with_cookie(Some(cookie_hdr));
    let json: Value = client.http.get(url).headers(headers).send().await?.json().await?;
    bili_check_code(&json)?;
    let refresh = json.pointer("/data/refresh").and_then(|v| v.as_bool()).unwrap_or(false);
    if !refresh {
        return Ok(None);
    }
    let ts = json.pointer("/data/timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
    if ts <= 0 {
        return Ok(Some(now_unix_ms()));
    }
    Ok(Some(ts))
}

async fn fetch_refresh_csrf(
    client: &BiliClient,
    cookie_hdr: &str,
    correspond_path: &str,
) -> Result<String, BiliError> {
    let url = format!(
        "{}/correspond/1/{}",
        client.endpoints.www_base.trim_end_matches('/'),
        correspond_path.trim()
    );
    let headers = header_map_with_cookie(Some(cookie_hdr));
    let html = client.http.get(url).headers(headers).send().await?.text().await?;
    extract_refresh_csrf(&html).ok_or_else(|| BiliError::Parse("refresh_csrf not found".to_string()))
}

async fn refresh_cookie_post(
    client: &BiliClient,
    cookie_hdr: &str,
    csrf: &str,
    refresh_csrf: &str,
    refresh_token: &str,
) -> Result<(AuthState, HeaderMap), BiliError> {
    let url = format!(
        "{}/x/passport-login/web/cookie/refresh",
        client.endpoints.passport_base.trim_end_matches('/')
    );
    let headers = header_map_with_cookie(Some(cookie_hdr));
    let resp = client
        .http
        .post(url)
        .headers(headers)
        .form(&[
            ("csrf", csrf),
            ("refresh_csrf", refresh_csrf),
            ("source", "main_web"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?;
    let hdr = resp.headers().clone();
    let json: Value = resp.json().await?;
    bili_check_code(&json)?;

    let new_refresh_token = json
        .pointer("/data/refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let cookie = build_cookie_string_from_set_cookie(&hdr);
    if cookie.as_deref().unwrap_or("").trim().is_empty() || new_refresh_token.is_empty() {
        return Err(BiliError::Parse("refresh response missing cookie/refresh_token".to_string()));
    }
    Ok((
        AuthState {
            cookie,
            refresh_token: Some(new_refresh_token),
        },
        hdr,
    ))
}

async fn confirm_refresh(
    client: &BiliClient,
    cookie_hdr: &str,
    csrf: &str,
    old_refresh_token: &str,
) -> Result<(), BiliError> {
    let url = format!(
        "{}/x/passport-login/web/confirm/refresh",
        client.endpoints.passport_base.trim_end_matches('/')
    );
    let headers = header_map_with_cookie(Some(cookie_hdr));
    let json: Value = client
        .http
        .post(url)
        .headers(headers)
        .form(&[("csrf", csrf), ("refresh_token", old_refresh_token)])
        .send()
        .await?
        .json()
        .await?;
    bili_check_code(&json)?;
    Ok(())
}

pub async fn refresh_cookie_if_needed(client: &BiliClient, auth: &AuthState) -> Result<AuthState, BiliError> {
    refresh_cookie_if_needed_with(client, auth, correspond_path).await
}

pub async fn refresh_cookie_if_needed_with(
    client: &BiliClient,
    auth: &AuthState,
    correspond_path_fn: fn(i64) -> Result<String, BiliError>,
) -> Result<AuthState, BiliError> {
    let cookie = auth.cookie.as_deref().unwrap_or("").trim();
    let refresh_token = auth.refresh_token.as_deref().unwrap_or("").trim();
    if cookie.is_empty() || refresh_token.is_empty() {
        return Err(BiliError::InvalidInput("missing cookie/refresh_token".to_string()));
    }

    let csrf = cookie_get(cookie, "bili_jct").ok_or_else(|| BiliError::InvalidInput("cookie missing bili_jct".to_string()))?;

    let buvid = super::playurl::ensure_buvid_cookie(client).await.ok();
    let cookie_hdr = merge_cookie_header(buvid.as_deref(), Some(cookie))
        .ok_or_else(|| BiliError::InvalidInput("empty cookie".to_string()))?;

    let ts = match cookie_info_needs_refresh(client, &cookie_hdr, &csrf).await? {
        None => return Ok(auth.clone()),
        Some(ts) => ts,
    };

    let path = correspond_path_fn(ts)?;
    let refresh_csrf = fetch_refresh_csrf(client, &cookie_hdr, &path).await?;

    let old_refresh = refresh_token.to_string();
    let (new_auth, _hdr) = refresh_cookie_post(client, &cookie_hdr, &csrf, &refresh_csrf, &old_refresh).await?;

    // Confirm refresh using new cookie's bili_jct and old refresh_token.
    let new_cookie = new_auth.cookie.as_deref().unwrap_or("").trim();
    let new_csrf = cookie_get(new_cookie, "bili_jct").unwrap_or_else(|| csrf.clone());
    let buvid2 = super::playurl::ensure_buvid_cookie(client).await.ok();
    let new_cookie_hdr = merge_cookie_header(buvid2.as_deref(), Some(new_cookie)).unwrap_or_else(|| new_cookie.to_string());
    let _ = confirm_refresh(client, &new_cookie_hdr, &new_csrf, &old_refresh).await;

    Ok(new_auth)
}
