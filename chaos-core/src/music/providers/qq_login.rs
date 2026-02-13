use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regex::Regex;
use reqwest::redirect::Policy;
use reqwest::header::{HeaderMap, LOCATION, REFERER};
use reqwest::Client;
use serde_json::{Value, json};

use crate::music::error::MusicError;
use crate::music::model::{MusicLoginQrState, MusicLoginType, QqMusicCookie};

pub fn new_login_client() -> Result<Client, MusicError> {
    crate::tls::ensure_rustls_provider();
    Ok(Client::builder()
        .user_agent("Mozilla/5.0")
        .cookie_store(true)
        .redirect(Policy::none())
        .timeout(Duration::from_secs(20))
        .build()?)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub async fn create_login_qr(
    http: &Client,
    login_type: MusicLoginType,
) -> Result<(String, String, Vec<u8>), MusicError> {
    // returns (identifier, mime, bytes)
    match login_type {
        MusicLoginType::Qq => {
            let t = fastrand::f64();
            let url = format!("https://ssl.ptlogin2.qq.com/ptqrshow?appid=716027609&e=2&l=M&s=3&d=72&v=4&t={t}&daid=383&pt_3rd_aid=100497308");
            let resp = http
                .get(url)
                .header(REFERER, "https://xui.ptlogin2.qq.com/")
                .send()
                .await?
                .error_for_status()?;
            let headers = resp.headers().clone();
            let bytes = resp.bytes().await?.to_vec();
            let qrsig = extract_cookie(&headers, "qrsig")
                .ok_or_else(|| MusicError::Parse("missing qrsig cookie".to_string()))?;
            Ok((qrsig, "image/png".to_string(), bytes))
        }
        MusicLoginType::Wechat => {
            // Step1: fetch uuid from html.
            let url = "https://open.weixin.qq.com/connect/qrconnect?appid=wx48db31d50e334801&redirect_uri=https%3A%2F%2Fy.qq.com%2Fportal%2Fwx_redirect.html%3Flogin_type%3D2%26surl%3Dhttps%3A%2F%2Fy.qq.com%2F&response_type=code&scope=snsapi_login&state=STATE&href=https%3A%2F%2Fy.qq.com%2Fmediastyle%2Fmusic_v17%2Fsrc%2Fcss%2Fpopup_wechat.css%23wechat_redirect";
            let html = http.get(url).send().await?.error_for_status()?.text().await?;
            let re = Regex::new(r#"uuid=([^"]+)""#).expect("uuid regex");
            let uuid = re
                .captures(&html)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| MusicError::Parse("missing wechat uuid".to_string()))?;
            let qr_url = format!("https://open.weixin.qq.com/connect/qrcode/{uuid}");
            let resp = http
                .get(qr_url)
                .header(REFERER, "https://open.weixin.qq.com/connect/qrconnect")
                .send()
                .await?
                .error_for_status()?;
            let bytes = resp.bytes().await?.to_vec();
            Ok((uuid, "image/jpeg".to_string(), bytes))
        }
    }
}

pub async fn poll_login_qr(
    http: &Client,
    login_type: MusicLoginType,
    identifier: &str,
) -> Result<(MusicLoginQrState, Option<String>, Option<String>, Option<String>), MusicError> {
    // returns (state, message, sigx_or_wx_code, uin)
    match login_type {
        MusicLoginType::Qq => poll_qq(http, identifier).await,
        MusicLoginType::Wechat => poll_wechat(http, identifier).await,
    }
}

async fn poll_qq(
    http: &Client,
    qrsig: &str,
) -> Result<(MusicLoginQrState, Option<String>, Option<String>, Option<String>), MusicError> {
    let token = sig_hash(qrsig, 0);
    let url = format!("https://ssl.ptlogin2.qq.com/ptqrlogin?u1=https%3A%2F%2Fgraph.qq.com%2Foauth2.0%2Flogin_jump&ptqrtoken={token}&ptredirect=0&h=1&t=1&g=1&from_ui=1&ptlang=2052&action=0-0-{ms}&js_ver=20102616&js_type=1&pt_uistyle=40&aid=716027609&daid=383&pt_3rd_aid=100497308&has_onekey=1", ms = now_ms());
    let resp = http
        .get(url)
        .header(REFERER, "https://xui.ptlogin2.qq.com/")
        .header("Cookie", format!("qrsig={qrsig}"))
        .send()
        .await?
        .error_for_status()?;
    let text = resp.text().await?;
    let re = Regex::new(r#"ptuiCB\((.*?)\)"#).expect("ptuiCB regex");
    let caps = match re.captures(&text) {
        Some(c) => c,
        None => {
            return Ok((
                MusicLoginQrState::Other,
                Some("invalid response".to_string()),
                None,
                None,
            ));
        }
    };
    let inner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let cleaned = inner.replace('\'', "").replace('"', "");
    let parts: Vec<&str> = cleaned.split(',').map(|s| s.trim()).collect();
    let code = parts.first().and_then(|s| s.parse::<i32>().ok()).unwrap_or(99);
    let state = match code {
        66 => MusicLoginQrState::Scan,
        67 => MusicLoginQrState::Confirm,
        65 => MusicLoginQrState::Timeout,
        68 => MusicLoginQrState::Refuse,
        0 => MusicLoginQrState::Done,
        _ => MusicLoginQrState::Other,
    };
    if state != MusicLoginQrState::Done {
        return Ok((state, None, None, None));
    }

    // Extract sigx & uin.
    let sigx = extract_regex(&text, r#"&ptsigx=([^&]+)&s_url"#);
    let uin = extract_regex(&text, r#"&uin=([^&]+)&service"#);
    Ok((state, None, sigx, uin))
}

async fn poll_wechat(
    http: &Client,
    uuid: &str,
) -> Result<(MusicLoginQrState, Option<String>, Option<String>, Option<String>), MusicError> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let url = format!("https://lp.open.weixin.qq.com/connect/l/qrconnect?uuid={uuid}&_={ts}");
    let text = http
        .get(url)
        .header(REFERER, "https://open.weixin.qq.com/")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let re = Regex::new(r#"window\.wx_errcode=(\d+);window\.wx_code='([^']*)'"#).expect("wx regex");
    let caps = match re.captures(&text) {
        Some(c) => c,
        None => {
            return Ok((
                MusicLoginQrState::Other,
                Some("invalid wechat response".to_string()),
                None,
                None,
            ));
        }
    };
    let err = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(0);
    let code = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
    let state = match err {
        408 => MusicLoginQrState::Scan,
        404 => MusicLoginQrState::Confirm,
        405 => MusicLoginQrState::Done,
        403 => MusicLoginQrState::Refuse,
        _ => MusicLoginQrState::Other,
    };
    let sig = if state == MusicLoginQrState::Done && !code.trim().is_empty() {
        Some(code)
    } else {
        None
    };
    Ok((state, None, sig, None))
}

pub async fn exchange_code_for_cookie(
    http: &Client,
    code: &str,
    login_type: MusicLoginType,
) -> Result<QqMusicCookie, MusicError> {
    let c = code.trim();
    if c.is_empty() {
        return Err(MusicError::InvalidInput("empty code".to_string()));
    }
    let payload = match login_type {
        MusicLoginType::Qq => json!({
            "comm": { "g_tk": 5381, "platform": "yqq", "ct": 24, "cv": 0 },
            "req": { "module": "QQConnectLogin.LoginServer", "method": "QQLogin", "param": { "code": c } }
        }),
        MusicLoginType::Wechat => json!({
            "comm": { "tmeLoginType": "1", "tmeAppID": "qqmusic", "g_tk": 5381, "platform": "yqq", "ct": 24, "cv": 0 },
            "req": { "module": "music.login.LoginServer", "method": "Login", "param": { "strAppid": "wx48db31d50e334801", "code": c } }
        }),
    };
    let resp = http
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Content-Type", "application/json;charset=utf-8")
        .header("Referer", "https://y.qq.com/")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    let v: Value = resp.json().await?;
    // Both QQLogin and Login return { code:0, req: { code:0, data:{...} } }
    let outer = v.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if outer != 0 {
        return Err(MusicError::Other(format!("qq login code={outer}")));
    }
    let req_node = v.get("req").or_else(|| v.get("req1")).ok_or_else(|| MusicError::Parse("missing req".to_string()))?;
    let inner = req_node.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if inner != 0 {
        let msg = req_node.get("msg").and_then(|v| v.as_str()).unwrap_or("login failed").to_string();
        return Err(MusicError::Other(msg));
    }
    let data = req_node.get("data").ok_or_else(|| MusicError::Parse("missing cookie data".to_string()))?;
    Ok(serde_json::from_value::<QqMusicCookie>(data.clone())?)
}

pub async fn refresh_cookie(http: &Client, cookie: &QqMusicCookie) -> Result<QqMusicCookie, MusicError> {
    let login_type = cookie.login_type.ok_or_else(|| MusicError::InvalidInput("cookie.loginType missing".to_string()))?;
    let str_musicid = cookie
        .str_musicid
        .as_deref()
        .or(cookie.musicid.as_deref())
        .unwrap_or("")
        .trim()
        .to_string();
    let musickey = cookie.musickey.as_deref().unwrap_or("").trim().to_string();
    let refresh_key = cookie.refresh_key.as_deref().unwrap_or("").trim().to_string();
    if str_musicid.is_empty() || musickey.is_empty() || refresh_key.is_empty() {
        return Err(MusicError::InvalidInput(
            "cookie missing strMusicid/musickey/refreshKey".to_string(),
        ));
    }

    let payload = json!({
        "comm": {
            "fPersonality": "0",
            "tmeLoginType": login_type.to_string(),
            "qq": str_musicid,
            "authst": musickey,
            "ct": "11",
            "cv": "12080008",
            "v": "12080008",
            "tmeAppID": "qqmusic"
        },
        "req1": {
            "module": "music.login.LoginServer",
            "method": "Login",
            "param": {
                "str_musicid": str_musicid,
                "musickey": musickey,
                "refresh_key": refresh_key
            }
        }
    });

    let resp = http
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Content-Type", "application/json;charset=utf-8")
        .header("Referer", "https://y.qq.com/")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    let v: Value = resp.json().await?;
    let outer = v.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if outer != 0 {
        return Err(MusicError::Other(format!("refresh code={outer}")));
    }
    let req_node = v.get("req1").ok_or_else(|| MusicError::Parse("missing req1".to_string()))?;
    let inner = req_node.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if inner != 0 {
        let msg = req_node.get("msg").and_then(|v| v.as_str()).unwrap_or("refresh failed").to_string();
        return Err(MusicError::Other(msg));
    }
    let data = req_node.get("data").ok_or_else(|| MusicError::Parse("missing cookie data".to_string()))?;
    Ok(serde_json::from_value::<QqMusicCookie>(data.clone())?)
}

pub async fn authorize_qq_and_get_code(
    http: &Client,
    sigx: &str,
    uin: &str,
) -> Result<String, MusicError> {
    let sigx = sigx.trim();
    let uin = uin.trim();
    if sigx.is_empty() || uin.is_empty() {
        return Err(MusicError::InvalidInput("missing sigx/uin".to_string()));
    }

    // Step1: check_sig to obtain p_skey in cookie store.
    let url = format!("https://ssl.ptlogin2.graph.qq.com/check_sig?uin={uin}&pttype=1&service=ptqrlogin&nodirect=0&ptsigx={sigx}&s_url=https%3A%2F%2Fgraph.qq.com%2Foauth2.0%2Flogin_jump&ptlang=2052&ptredirect=100&aid=716027609&daid=383&j_later=0&low_login_hour=0&regmaster=0&pt_login_type=3&pt_aid=0&pt_aaid=16&pt_light=0&pt_3rd_aid=100497308");
    let resp = http.get(url).header(REFERER, "https://xui.ptlogin2.qq.com/").send().await?;
    let headers = resp.headers().clone();
    // We don't need the body; location not followed due to Policy::none().
    let _ = resp.bytes().await;

    let p_skey = extract_cookie(&headers, "p_skey")
        .ok_or_else(|| MusicError::Other("missing p_skey".to_string()))?;
    let gtk = sig_hash(&p_skey, 5381).to_string();

    // Step2: authorize to obtain code in Location header.
    let auth_time = now_ms().to_string();
    let ui = format!("{:x}", md5::compute(auth_time.as_bytes()));
    let form: Vec<(String, String)> = vec![
        ("response_type".to_string(), "code".to_string()),
        ("client_id".to_string(), "100497308".to_string()),
        (
            "redirect_uri".to_string(),
            "https://y.qq.com/portal/wx_redirect.html?login_type=1&surl=https://y.qq.com"
                .to_string(),
        ),
        ("state".to_string(), "state".to_string()),
        ("switch".to_string(), "".to_string()),
        ("from_ptlogin".to_string(), "1".to_string()),
        ("src".to_string(), "1".to_string()),
        ("update_auth".to_string(), "1".to_string()),
        ("openapi".to_string(), "1010_1030".to_string()),
        ("g_tk".to_string(), gtk),
        ("auth_time".to_string(), auth_time),
        ("ui".to_string(), ui),
    ];

    let resp = http
        .post("https://graph.qq.com/oauth2.0/authorize")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header(
            REFERER,
            "https://graph.qq.com/oauth2.0/show?which=Login&display=pc&response_type=code&client_id=100497308&redirect_uri=https://y.qq.com/portal/wx_redirect.html?login_type=1&surl=https://y.qq.com/portal/profile.html",
        )
        .form(&form)
        .send()
        .await?;

    let loc = resp
        .headers()
        .get(LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if loc.is_empty() {
        return Err(MusicError::Other("missing Location".to_string()));
    }
    let re = Regex::new(r#"code=([^&]+)"#).expect("code regex");
    let code = re
        .captures(&loc)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| MusicError::Other("missing code in redirect".to_string()))?;
    Ok(code)
}

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    for v in headers.get_all("Set-Cookie").iter() {
        let s = v.to_str().ok()?;
        // "name=value; Path=/; ..."
        if let Some(rest) = s.strip_prefix(&format!("{name}=")) {
            let value = rest.split(';').next().unwrap_or("").to_string();
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn extract_regex(text: &str, pat: &str) -> Option<String> {
    let re = Regex::new(pat).ok()?;
    re.captures(text).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

fn sig_hash(input: &str, seed: i64) -> i64 {
    let mut hash = seed;
    for c in input.chars() {
        hash = (hash << 5) + hash + (c as i64);
    }
    hash & 0x7fffffff
}

// Note: we intentionally parse QQ cookie responses with `serde_json::Value` for flexibility across
// login endpoints and variants.
