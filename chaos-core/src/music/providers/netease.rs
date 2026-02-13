use std::time::Duration;

use reqwest::Client;
use serde_json::{Value, json};

use crate::music::error::MusicError;
use crate::music::model::{AuthState, MusicAlbum, MusicArtist, MusicQuality, MusicService, MusicTrack, ProviderConfig};

fn bases(cfg: &ProviderConfig) -> Result<Vec<String>, MusicError> {
    let mut out: Vec<String> = Vec::new();
    for raw in &cfg.netease_base_urls {
        let b = raw.trim().trim_end_matches('/').to_string();
        if b.is_empty() {
            continue;
        }
        if !out.contains(&b) {
            out.push(b);
        }
    }
    if out.is_empty() {
        return Err(MusicError::NotConfigured("neteaseBaseUrls is empty".to_string()));
    }
    Ok(out)
}

fn append_timestamp(url: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    if url.contains('?') {
        format!("{url}&timestamp={ts}")
    } else {
        format!("{url}?timestamp={ts}")
    }
}

pub fn anonymous_cookie_path(cfg: &ProviderConfig) -> String {
    cfg.netease_anonymous_cookie_url
        .as_deref()
        .unwrap_or("/register/anonimous")
        .trim()
        .to_string()
}

pub async fn fetch_anonymous_cookie(
    http: &Client,
    cfg: &ProviderConfig,
    timeout: Duration,
) -> Result<String, MusicError> {
    let p = anonymous_cookie_path(cfg);
    let mut last: Option<MusicError> = None;
    for base in bases(cfg)? {
        let url = format!(
            "{base}{}",
            if p.starts_with('/') { p.clone() } else { format!("/{p}") }
        );
        let resp = match http.get(append_timestamp(&url)).timeout(timeout).send().await {
            Ok(v) => v,
            Err(e) => {
                last = Some(MusicError::Http(e));
                continue;
            }
        };
        let resp = match resp.error_for_status() {
            Ok(v) => v,
            Err(e) => {
                last = Some(MusicError::Http(e));
                continue;
            }
        };
        let json: Value = resp.json().await?;
        let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        if code != 200 {
            last = Some(MusicError::Other(format!("netease anon code={code}")));
            continue;
        }
        let cookie = json
            .get("cookie")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if cookie.is_empty() {
            last = Some(MusicError::Parse("netease anon: missing cookie".to_string()));
            continue;
        }
        return Ok(cookie);
    }

    Err(last.unwrap_or_else(|| MusicError::Other("netease anon: all baseUrls failed".to_string())))
}

async fn post_json(
    http: &Client,
    url: &str,
    payload: &Value,
    cookie: Option<&str>,
    timeout: Duration,
) -> Result<Value, MusicError> {
    let mut req = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "Mozilla/5.0")
        .timeout(timeout);
    if let Some(c) = cookie {
        if !c.trim().is_empty() {
            req = req.header("Cookie", c.trim());
        }
    }
    let resp = req.json(payload).send().await?.error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

async fn post_json_try_bases(
    http: &Client,
    cfg: &ProviderConfig,
    path: &str,
    payload: &Value,
    cookie: Option<&str>,
    timeout: Duration,
) -> Result<Value, MusicError> {
    let mut last: Option<MusicError> = None;
    for base in bases(cfg)? {
        let url = if path.starts_with('/') {
            format!("{base}{path}")
        } else {
            format!("{base}/{path}")
        };
        match post_json(http, &append_timestamp(&url), payload, cookie, timeout).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = Some(e);
                continue;
            }
        }
    }
    Err(last.unwrap_or_else(|| MusicError::Other("netease: all baseUrls failed".to_string())))
}

fn qualities_from_song(song: &Value) -> Vec<MusicQuality> {
    let mut out = Vec::new();
    // These keys are common in Netease song detail/search responses.
    let has = |k: &str| song.get(k).is_some();
    // Keep `id` stable across providers ("flac", "mp3_320"...). Some Netease responses include both
    // "hr" (Hi-Res) and "sq" (standard lossless). Expose the best available lossless tier as "flac".
    if has("hr") || has("sq") {
        let hi_res = has("hr");
        out.push(MusicQuality {
            id: "flac".to_string(),
            label: if hi_res { "Hi-Res" } else { "FLAC" }.to_string(),
            format: "flac".to_string(),
            bitrate_kbps: Some(if hi_res { 3000 } else { 2000 }),
            lossless: true,
        });
    }
    if has("h") {
        out.push(MusicQuality {
            id: "mp3_320".to_string(),
            label: "MP3 320".to_string(),
            format: "mp3".to_string(),
            bitrate_kbps: Some(320),
            lossless: false,
        });
    }
    if has("m") {
        out.push(MusicQuality {
            id: "mp3_192".to_string(),
            label: "MP3 192".to_string(),
            format: "mp3".to_string(),
            bitrate_kbps: Some(192),
            lossless: false,
        });
    }
    if has("l") {
        out.push(MusicQuality {
            id: "mp3_128".to_string(),
            label: "MP3 128".to_string(),
            format: "mp3".to_string(),
            bitrate_kbps: Some(128),
            lossless: false,
        });
    }
    out.sort_by_key(|q| q.bitrate_kbps.unwrap_or(0));
    out.dedup_by(|a, b| a.id == b.id && a.bitrate_kbps == b.bitrate_kbps);
    out
}

fn map_song_to_track(song: &Value) -> Option<MusicTrack> {
    let id = song.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default();
    if id.is_empty() {
        return None;
    }
    let title = song.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let duration_ms = song.get("dt").and_then(|v| v.as_i64()).map(|n| n.max(0) as u64);
    let album = song.pointer("/al/name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let album_id = song.pointer("/al/id").and_then(|v| v.as_i64()).map(|n| n.to_string());
    let cover = song.pointer("/al/picUrl").and_then(|v| v.as_str()).map(|s| s.to_string());
    let mut artists = Vec::new();
    let mut artist_ids = Vec::new();
    if let Some(arr) = song.get("ar").and_then(|v| v.as_array()) {
        for a in arr {
            if let Some(n) = a.get("name").and_then(|v| v.as_str()) {
                if !n.trim().is_empty() {
                    artists.push(n.to_string());
                }
            }
            if let Some(i) = a.get("id").and_then(|v| v.as_i64()) {
                artist_ids.push(i.to_string());
            }
        }
    }
    let qualities = qualities_from_song(song);
    Some(MusicTrack {
        service: MusicService::Netease,
        id,
        title,
        artists,
        artist_ids,
        album,
        album_id,
        duration_ms,
        cover_url: cover,
        qualities,
    })
}

pub async fn search_tracks(
    http: &Client,
    cfg: &ProviderConfig,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicTrack>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let offset = (page.saturating_sub(1) * page_size).to_string();
    let payload = json!({
        "keywords": q,
        "limit": page_size.clamp(1, 50),
        "type": 1,
        "offset": offset.parse::<u32>().unwrap_or(0),
    });
    let json = post_json_try_bases(http, cfg, "/cloudsearch", &payload, None, timeout).await?;
    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code != 200 {
        return Ok(vec![]);
    }
    let songs = json.pointer("/result/songs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(songs.into_iter().filter_map(|s| map_song_to_track(&s)).collect())
}

pub async fn search_albums(
    http: &Client,
    cfg: &ProviderConfig,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let payload = json!({
        "keywords": q,
        "limit": page_size.clamp(1, 50),
        "type": 10,
        "offset": (page.saturating_sub(1) * page_size),
    });
    let json = post_json_try_bases(http, cfg, "/cloudsearch", &payload, None, timeout).await?;
    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code != 200 {
        return Ok(vec![]);
    }
    let list = json.pointer("/result/albums").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for it in list {
        let id = it.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let title = it.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.pointer("/artist/name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let artist_id = it.pointer("/artist/id").and_then(|v| v.as_i64()).map(|n| n.to_string());
        let cover = it.get("picUrl").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(MusicAlbum {
            service: MusicService::Netease,
            id,
            title,
            artist,
            artist_id,
            cover_url: cover,
            publish_time: it.get("publishTime").and_then(|v| v.as_i64()).map(|n| n.to_string()),
            track_count: None,
        });
    }
    Ok(out)
}

pub async fn search_artists(
    http: &Client,
    cfg: &ProviderConfig,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicArtist>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let payload = json!({
        "keywords": q,
        "limit": page_size.clamp(1, 50),
        "type": 100,
        "offset": (page.saturating_sub(1) * page_size),
    });
    let json = post_json_try_bases(http, cfg, "/cloudsearch", &payload, None, timeout).await?;
    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code != 200 {
        return Ok(vec![]);
    }
    let list = json.pointer("/result/artists").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for it in list {
        let id = it.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let name = it.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let cover = it.get("picUrl").and_then(|v| v.as_str()).map(|s| s.to_string());
        let album_count = it.get("albumSize").and_then(|v| v.as_i64()).and_then(|n| u32::try_from(n).ok());
        out.push(MusicArtist {
            service: MusicService::Netease,
            id,
            name,
            cover_url: cover,
            album_count,
        });
    }
    Ok(out)
}

pub async fn album_tracks(
    http: &Client,
    cfg: &ProviderConfig,
    album_id: &str,
    timeout: Duration,
) -> Result<Vec<MusicTrack>, MusicError> {
    let id = album_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty album_id".to_string()));
    }
    let payload = json!({ "id": id });
    let json = post_json_try_bases(http, cfg, "/album", &payload, None, timeout).await?;
    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code != 200 {
        return Ok(vec![]);
    }
    let songs = json.get("songs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(songs.into_iter().filter_map(|s| map_song_to_track(&s)).collect())
}

pub async fn artist_albums(
    http: &Client,
    cfg: &ProviderConfig,
    artist_id: &str,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let id = artist_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty artist_id".to_string()));
    }
    let payload = json!({ "id": id, "limit": 1000, "offset": 0 });
    let json = post_json_try_bases(http, cfg, "/artist/album", &payload, None, timeout).await?;
    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code != 200 {
        return Ok(vec![]);
    }
    let list = json.pointer("/hotAlbums").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for it in list {
        let album_id = it.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default();
        if album_id.is_empty() {
            continue;
        }
        let title = it.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let cover = it.get("picUrl").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(MusicAlbum {
            service: MusicService::Netease,
            id: album_id,
            title,
            artist: None,
            artist_id: Some(id.to_string()),
            cover_url: cover,
            publish_time: it.get("publishTime").and_then(|v| v.as_i64()).map(|n| n.to_string()),
            track_count: None,
        });
    }
    Ok(out)
}

pub async fn track_download_url(
    http: &Client,
    cfg: &ProviderConfig,
    track_id: &str,
    quality_id: &str,
    auth: &AuthState,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    let id = track_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty track_id".to_string()));
    }

    let cookie = auth.netease_cookie.as_deref();
    let q = quality_id.trim();
    let payload = match q {
        "mp3_128" => json!({ "id": id, "br": 128000 }),
        "mp3_192" => json!({ "id": id, "br": 192000 }),
        "mp3_320" => json!({ "id": id, "br": 320000 }),
        "flac" => json!({ "id": id, "type": "flac", "br": 999000 }),
        _ => json!({ "id": id, "br": 320000 }),
    };

    let json = post_json_try_bases(http, cfg, "/song/download/url", &payload, cookie, timeout).await?;
    // Some services return { data: { url: ... } }, others { url: ... }.
    let url = json
        .pointer("/data/url")
        .or_else(|| json.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(MusicError::Other(
            "netease: empty download url (may require login or different API)".to_string(),
        ));
    }
    let ext = if q == "flac" { "flac" } else { "mp3" }.to_string();
    Ok((url, ext))
}
