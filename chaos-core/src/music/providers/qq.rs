use std::time::Duration;

use reqwest::Client;
use serde_json::{Value, json};

use crate::music::error::MusicError;
use crate::music::model::{AuthState, MusicAlbum, MusicArtist, MusicQuality, MusicService, MusicTrack};
use crate::music::util::quality_fallback_order;

const BASE_URL: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";

fn ua() -> &'static str {
    // Keep aligned with refs for higher compatibility.
    "QQ%E9%9F%B3%E4%B9%90/73222 CFNetwork/1406.0.3 Darwin/22.4.0"
}

async fn post_musicu_at(
    http: &Client,
    base_url: &str,
    body: &Value,
    timeout: Duration,
    sign: Option<&str>,
) -> Result<Value, MusicError> {
    let mut url = base_url.to_string();
    if let Some(sign) = sign {
        url = format!("{url}?sign={sign}&signature={sign}");
    }

    let resp = http
        .post(url)
        .header("Content-Type", "application/json;charset=utf-8")
        .header("Referer", "https://y.qq.com")
        .header("User-Agent", ua())
        .timeout(timeout)
        .json(body)
        .send()
        .await?
        .error_for_status()?;
    let bytes = resp.bytes().await?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn post_musicu(http: &Client, body: &Value, timeout: Duration, sign: Option<&str>) -> Result<Value, MusicError> {
    post_musicu_at(http, BASE_URL, body, timeout, sign).await
}

fn qualities_from_file(v: &Value) -> Vec<MusicQuality> {
    let mut out = Vec::new();
    let size_flac = v.get("size_flac").and_then(|v| v.as_i64()).unwrap_or(0);
    let size_320 = v.get("size_320mp3").and_then(|v| v.as_i64()).unwrap_or(0);
    let size_128 = v.get("size_128mp3").and_then(|v| v.as_i64()).unwrap_or(0);

    if size_flac > 0 {
        out.push(MusicQuality {
            id: "flac".to_string(),
            label: "FLAC".to_string(),
            format: "flac".to_string(),
            bitrate_kbps: Some(2000),
            lossless: true,
        });
    }
    if size_320 > 0 {
        out.push(MusicQuality {
            id: "mp3_320".to_string(),
            label: "MP3 320".to_string(),
            format: "mp3".to_string(),
            bitrate_kbps: Some(320),
            lossless: false,
        });
    }
    if size_128 > 0 {
        out.push(MusicQuality {
            id: "mp3_128".to_string(),
            label: "MP3 128".to_string(),
            format: "mp3".to_string(),
            bitrate_kbps: Some(128),
            lossless: false,
        });
    }
    out
}

pub async fn search_tracks(
    http: &Client,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicTrack>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let body = json!({
        "comm": { "ct": "19", "cv": "1859", "uin": "0" },
        "req": {
            "method": "DoSearchForQQMusicDesktop",
            "module": "music.search.SearchCgiService",
            "param": {
                "search_type": 0,
                "query": q,
                "page_num": page.max(1),
                "num_per_page": page_size.clamp(1, 50),
                "grp": 1
            }
        }
    });

    let json = post_musicu(http, &body, timeout, None).await?;
    let list = json
        .pointer("/req/data/body/song/list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let mid = it.get("mid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if mid.is_empty() {
            continue;
        }
        let title = it.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut artists = Vec::new();
        let mut artist_ids = Vec::new();
        if let Some(arr) = it.get("singer").and_then(|v| v.as_array()) {
            for s in arr {
                if let Some(name) = s.get("name").and_then(|v| v.as_str()) {
                    if !name.trim().is_empty() {
                        artists.push(name.to_string());
                    }
                }
                if let Some(id) = s.get("mid").and_then(|v| v.as_str()) {
                    if !id.trim().is_empty() {
                        artist_ids.push(id.to_string());
                    }
                }
            }
        }
        let album = it.pointer("/album/name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let album_id = it.pointer("/album/mid").and_then(|v| v.as_str()).map(|s| s.to_string());
        let pmid = it.pointer("/album/pmid").and_then(|v| v.as_str()).unwrap_or("");
        let cover_url = (!pmid.trim().is_empty()).then_some(format!(
            "https://y.qq.com/music/photo_new/T002R800x800M000{pmid}.jpg"
        ));
        let duration_ms = it.get("interval").and_then(|v| v.as_u64()).map(|s| s * 1000);
        let qualities = it
            .get("file")
            .map(qualities_from_file)
            .unwrap_or_default();

        out.push(MusicTrack {
            service: MusicService::Qq,
            id: mid,
            title,
            artists,
            artist_ids,
            album,
            album_id,
            duration_ms,
            cover_url,
            qualities,
        });
    }
    Ok(out)
}

pub async fn search_artists(
    http: &Client,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicArtist>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let body = json!({
        "comm": { "ct": "19", "cv": "1859", "uin": "0" },
        "req": {
            "method": "DoSearchForQQMusicDesktop",
            "module": "music.search.SearchCgiService",
            "param": {
                "search_type": 1,
                "query": q,
                "page_num": page.max(1),
                "num_per_page": page_size.clamp(1, 50),
                "grp": 1
            }
        }
    });

    let json = post_musicu(http, &body, timeout, None).await?;
    let list = json
        .pointer("/req/data/body/singer/list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it.get("singerMID").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let name = it.get("singerName").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let cover = it.get("singerPic").and_then(|v| v.as_str()).map(|s| s.to_string());
        let album_count = it
            .get("albumNum")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok());
        out.push(MusicArtist {
            service: MusicService::Qq,
            id,
            name,
            cover_url: cover,
            album_count,
        });
    }
    Ok(out)
}

pub async fn search_albums(
    http: &Client,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let body = json!({
        "comm": { "ct": "19", "cv": "1859", "uin": "0" },
        "req": {
            "method": "DoSearchForQQMusicDesktop",
            "module": "music.search.SearchCgiService",
            "param": {
                "search_type": 2,
                "query": q,
                "page_num": page.max(1),
                "num_per_page": page_size.clamp(1, 50),
                "grp": 1
            }
        }
    });

    let json = post_musicu(http, &body, timeout, None).await?;
    let list = json
        .pointer("/req/data/body/album/list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it.get("albumMID").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = it.get("albumName").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.get("singerName").and_then(|v| v.as_str()).map(|s| s.to_string());
        let artist_id = it.get("singerID").and_then(|v| v.as_str()).map(|s| s.to_string());
        let cover = it.get("albumPic").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(MusicAlbum {
            service: MusicService::Qq,
            id,
            title,
            artist,
            artist_id,
            cover_url: cover,
            publish_time: None,
            track_count: None,
        });
    }
    Ok(out)
}

pub async fn album_tracks(
    http: &Client,
    album_mid: &str,
    timeout: Duration,
) -> Result<Vec<MusicTrack>, MusicError> {
    let mid = album_mid.trim();
    if mid.is_empty() {
        return Err(MusicError::InvalidInput("empty album_id".to_string()));
    }
    let body = json!({
        "AlbumSongList": {
            "module": "music.musichallAlbum.AlbumSongList",
            "method": "GetAlbumSongList",
            "param": { "albumMid": mid, "begin": 0, "num": 100, "order": 2 }
        },
        "comm": {
            "g_tk": 0,
            "uin": "",
            "format": "json",
            "ct": 6,
            "cv": 80600,
            "platform": "wk_v17",
            "uid": ""
        }
    });

    let json = post_musicu(http, &body, timeout, None).await?;
    let list = json
        .pointer("/AlbumSongList/data/songList")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let song = it.get("songInfo").unwrap_or(&Value::Null);
        let id = song.get("mid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = song.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut artists = Vec::new();
        let mut artist_ids = Vec::new();
        if let Some(arr) = song.get("singer").and_then(|v| v.as_array()) {
            for s in arr {
                if let Some(name) = s.get("name").and_then(|v| v.as_str()) {
                    if !name.trim().is_empty() {
                        artists.push(name.to_string());
                    }
                }
                if let Some(id) = s.get("mid").and_then(|v| v.as_str()) {
                    if !id.trim().is_empty() {
                        artist_ids.push(id.to_string());
                    }
                }
            }
        }
        let album = song.pointer("/album/name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let album_id = song.pointer("/album/mid").and_then(|v| v.as_str()).map(|s| s.to_string());
        let pmid = song.pointer("/album/pmid").and_then(|v| v.as_str()).unwrap_or("");
        let cover_url = (!pmid.trim().is_empty()).then_some(format!(
            "https://y.qq.com/music/photo_new/T002R800x800M000{pmid}.jpg"
        ));
        let duration_ms = song.get("interval").and_then(|v| v.as_u64()).map(|s| s * 1000);
        let qualities = song.get("file").map(qualities_from_file).unwrap_or_default();

        out.push(MusicTrack {
            service: MusicService::Qq,
            id,
            title,
            artists,
            artist_ids,
            album,
            album_id,
            duration_ms,
            cover_url,
            qualities,
        });
    }
    Ok(out)
}

pub async fn artist_albums(
    http: &Client,
    artist_mid: &str,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let mid = artist_mid.trim();
    if mid.is_empty() {
        return Err(MusicError::InvalidInput("empty artist_id".to_string()));
    }
    let body = json!({
        "comm": { "ct": 24, "cv": 0 },
        "singerAlbum": {
            "method": "get_singer_album",
            "param": { "singermid": mid, "order": "time", "begin": 0, "num": 1000, "exstatus": 1 },
            "module": "music.web_singer_info_svr"
        }
    });

    let json = post_musicu(http, &body, timeout, None).await?;
    let list = json
        .pointer("/singerAlbum/data/list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it.get("album_mid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = it.get("album_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.get("singer_name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let artist_id = it.get("singer_mid").and_then(|v| v.as_str()).map(|s| s.to_string());
        let publish_time = it.get("pub_time").and_then(|v| v.as_str()).map(|s| s.to_string());
        let cover_url = Some(format!(
            "https://y.qq.com/music/photo_new/T002R800x800M000{id}.jpg"
        ));
        out.push(MusicAlbum {
            service: MusicService::Qq,
            id,
            title,
            artist,
            artist_id,
            cover_url,
            publish_time,
            track_count: None,
        });
    }
    Ok(out)
}

fn map_quality_to_songtype(quality_id: &str) -> (&'static str, &'static str) {
    match quality_id {
        "mp3_128" => ("M500", "mp3"),
        "mp3_320" => ("M800", "mp3"),
        "flac" => ("F000", "flac"),
        // best-effort
        _ => ("M800", "mp3"),
    }
}

fn qq_guid() -> String {
    const ALLOWED: &[u8] = b"abcdef1234567890";
    let mut s = String::with_capacity(32);
    for _ in 0..32 {
        let i = fastrand::usize(..ALLOWED.len());
        s.push(ALLOWED[i] as char);
    }
    s
}

pub fn sign_request_payload(json_text: &str) -> Result<String, MusicError> {
    use base64::Engine as _;

    let md5 = md5::compute(json_text.as_bytes());
    let md5_hex = format!("{:x}", md5).to_uppercase();
    let b = md5_hex.as_bytes();
    if b.len() < 32 {
        return Err(MusicError::Other("md5 hex too short".to_string()));
    }

    fn extract(b: &[u8], pos: &[usize]) -> Vec<u8> {
        pos.iter().map(|&i| b[i]).collect()
    }

    let head = extract(b, &[21, 4, 9, 26, 16, 20, 27, 30]);
    let tail = extract(b, &[18, 11, 3, 2, 1, 7, 6, 25]);

    // middle: convert hex-pairs to bytes then xor with table.
    let ol: [u8; 16] = [212, 45, 80, 68, 195, 163, 163, 203, 157, 220, 254, 91, 204, 79, 104, 6];
    let mut mid = Vec::with_capacity(16);
    for j in 0..16 {
        let i = j * 2;
        let hi = (b[i] as char).to_digit(16).ok_or_else(|| MusicError::Other("bad md5 hex".to_string()))?;
        let lo = (b[i + 1] as char).to_digit(16).ok_or_else(|| MusicError::Other("bad md5 hex".to_string()))?;
        let r = ((hi as u8) << 4) ^ (lo as u8);
        mid.push(r ^ ol[j]);
    }
    let m = base64::engine::general_purpose::STANDARD.encode(mid);

    let mut res = String::from("zzb");
    res.push_str(&String::from_utf8_lossy(&head));
    res.push_str(&m);
    res.push_str(&String::from_utf8_lossy(&tail));

    Ok(res
        .to_lowercase()
        .replace('/', "")
        .replace('+', "")
        .replace('=', ""))
}

pub async fn track_download_url(
    http: &Client,
    track_mid: &str,
    quality_id: &str,
    auth: &AuthState,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    track_download_url_with_base(http, BASE_URL, track_mid, quality_id, auth, timeout).await
}

#[doc(hidden)]
pub async fn track_download_url_with_base(
    http: &Client,
    base_url: &str,
    track_mid: &str,
    quality_id: &str,
    auth: &AuthState,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    let mid = track_mid.trim();
    if mid.is_empty() {
        return Err(MusicError::InvalidInput("empty track_id".to_string()));
    }
    let cookie = auth
        .qq
        .as_ref()
        .ok_or_else(|| MusicError::Unauthorized("missing qq cookie".to_string()))?;

    let musickey = cookie
        .musickey
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let qq = cookie.musicid.as_deref().unwrap_or("").trim().to_string();
    let login_type = cookie
        .login_type
        .map(|v| v.to_string())
        .unwrap_or_default();

    if musickey.is_empty() || qq.is_empty() || login_type.is_empty() {
        return Err(MusicError::Unauthorized(
            "qq cookie missing musickey/musicid/loginType".to_string(),
        ));
    }

    let desired = quality_id.trim();
    let (prefix, ext) = map_quality_to_songtype(desired);

    let file_name = format!("{prefix}{mid}{mid}.{ext}");
    let payload = json!({
        "comm": {
            "cv": 13020508,
            "v": 13020508,
            "ct": "24",
            "tmeAppID": "qqmusic",
            "format": "json",
            "inCharset": "utf-8",
            "outCharset": "utf-8",
            "uid": "3931641530",
            "qq": qq,
            "authst": musickey,
            "tmeLoginType": login_type
        },
        "music.vkey.GetVkey.UrlGetVkey": {
            "module": "music.vkey.GetVkey",
            "method": "UrlGetVkey",
            "param": {
                "filename": [ file_name ],
                "guid": qq_guid(),
                "songmid": [ mid ],
                "songtype": [ 0 ],
                "uin": qq,
                "loginflag": 1,
                "platform": "20"
            }
        }
    });
    let payload_text = serde_json::to_string(&payload)?;
    let sign = sign_request_payload(&payload_text)?;
    let json = post_musicu_at(http, base_url, &payload, timeout, Some(&sign)).await?;

    let node = json
        .get("music.vkey.GetVkey.UrlGetVkey")
        .ok_or_else(|| MusicError::Parse("missing vkey response".to_string()))?;
    let code = node.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 0 {
        return Err(MusicError::Other(format!("qq vkey code={code}")));
    }
    let midurlinfo = node
        .pointer("/data/midurlinfo")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| MusicError::Parse("missing midurlinfo".to_string()))?;

    let mut purl = midurlinfo
        .get("wifiurl")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if purl.trim().is_empty() {
        purl = midurlinfo
            .get("purl")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
    }
    if purl.trim().is_empty() {
        return Err(MusicError::Other("qq: empty purl".to_string()));
    }

    // sip base urls
    let mut base = "https://isure.stream.qqmusic.qq.com/".to_string();
    if let Some(sip) = node.pointer("/data/sip").and_then(|v| v.as_array()) {
        let candidates: Vec<String> = sip
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !candidates.is_empty() {
            base = candidates[fastrand::usize(..candidates.len())].clone();
        }
    }

    Ok((format!("{base}{purl}"), ext.to_string()))
}

pub fn pick_quality_id(available: &[MusicQuality], desired: &str) -> Option<String> {
    let d = desired.trim();
    if !d.is_empty() && available.iter().any(|q| q.id == d) {
        return Some(d.to_string());
    }
    for id in quality_fallback_order() {
        if available.iter().any(|q| q.id == id) {
            return Some(id.to_string());
        }
    }
    available.first().map(|q| q.id.clone())
}
