use std::time::Duration;

use reqwest::Client;
use serde_json::Value;

use crate::music::error::MusicError;
use crate::music::model::{
    AuthState, KugouUserInfo, MusicAlbum, MusicArtist, MusicQuality, MusicService, MusicTrack,
    ProviderConfig,
};

fn base_url(cfg: &ProviderConfig) -> Result<String, MusicError> {
    let Some(b) = cfg.kugou_base_url.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Err(MusicError::NotConfigured(
            "kugouBaseUrl is not set".to_string(),
        ));
    };
    Ok(b.trim_end_matches('/').to_string())
}

fn cookie_from_auth(auth: &AuthState) -> Option<String> {
    let u = auth.kugou.as_ref()?;
    let token = u.token.trim();
    let userid = u.userid.trim();
    if token.is_empty() || userid.is_empty() {
        return None;
    }
    Some(format!("token={token};userid={userid};KUGOU_API_PLATFORM=lite"))
}

async fn get_json(
    http: &Client,
    url: &str,
    params: &[(&str, String)],
    timeout: Duration,
) -> Result<Value, MusicError> {
    let resp = http
        .get(url)
        .query(params)
        .timeout(timeout)
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

fn qualities_from_item(item: &Value) -> Vec<MusicQuality> {
    let mut out = Vec::new();
    let file_128 = item.get("FileSize").or_else(|| item.get("fileSize")).and_then(|v| v.as_i64()).unwrap_or(0);
    if file_128 > 0 {
        out.push(MusicQuality {
            id: "mp3_128".to_string(),
            label: "MP3 128".to_string(),
            format: "mp3".to_string(),
            bitrate_kbps: Some(128),
            lossless: false,
        });
    }
    if let Some(hq) = item.get("HQ").or_else(|| item.get("hq")) {
        let sz = hq.get("FileSize").or_else(|| hq.get("fileSize")).and_then(|v| v.as_i64()).unwrap_or(0);
        if sz > 0 {
            out.push(MusicQuality {
                id: "mp3_320".to_string(),
                label: "MP3 320".to_string(),
                format: "mp3".to_string(),
                bitrate_kbps: Some(320),
                lossless: false,
            });
        }
    }
    if let Some(sq) = item.get("SQ").or_else(|| item.get("sq")) {
        let sz = sq.get("FileSize").or_else(|| sq.get("fileSize")).and_then(|v| v.as_i64()).unwrap_or(0);
        if sz > 0 {
            out.push(MusicQuality {
                id: "flac".to_string(),
                label: "FLAC".to_string(),
                format: "flac".to_string(),
                bitrate_kbps: Some(2000),
                lossless: true,
            });
        }
    }
    out
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

    let base = base_url(cfg)?;
    let url = format!("{base}/search");
    let params: Vec<(&str, String)> = vec![
        ("page", page.max(1).to_string()),
        ("pagesize", page_size.clamp(1, 50).to_string()),
        ("type", "song".to_string()),
        ("keywords", q.to_string()),
    ];
    let json = get_json(http, &url, &params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(vec![]);
    }
    let list = json
        .pointer("/data/lists")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it.get("FileHash").or_else(|| it.get("fileHash")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = it
            .get("OriSongName")
            .or_else(|| it.get("oriSongName"))
            .or_else(|| it.get("SongName"))
            .or_else(|| it.get("songName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let album = it.get("AlbumName").or_else(|| it.get("albumName")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let album_id = it.get("AlbumID").or_else(|| it.get("albumID")).or_else(|| it.get("albumId")).and_then(|v| v.as_i64()).map(|n| n.to_string());
        let duration_ms = it.get("Duration").or_else(|| it.get("duration")).and_then(|v| v.as_i64()).map(|s| (s.max(0) as u64) * 1000);
        let mut artists = Vec::new();
        let mut artist_ids = Vec::new();
        if let Some(arr) = it.get("Singers").or_else(|| it.get("singers")).and_then(|v| v.as_array()) {
            for s in arr {
                if let Some(n) = s.get("Name").or_else(|| s.get("name")).and_then(|v| v.as_str()) {
                    if !n.trim().is_empty() {
                        artists.push(n.to_string());
                    }
                }
                if let Some(i) = s.get("Id").or_else(|| s.get("id")).and_then(|v| v.as_i64()) {
                    artist_ids.push(i.to_string());
                }
            }
        } else if let Some(singer) = it.get("SingerName").or_else(|| it.get("singerName")).and_then(|v| v.as_str()) {
            if !singer.trim().is_empty() {
                artists.push(singer.to_string());
            }
        }

        let image = it
            .get("Image")
            .or_else(|| it.get("image"))
            .and_then(|v| v.as_str())
            .map(|s| s.replace("{size}", "480"));

        let qualities = qualities_from_item(&it);

        out.push(MusicTrack {
            service: MusicService::Kugou,
            id,
            title,
            artists,
            artist_ids,
            album,
            album_id,
            duration_ms,
            cover_url: image,
            qualities,
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
    let base = base_url(cfg)?;
    let url = format!("{base}/search");
    let params: Vec<(&str, String)> = vec![
        ("page", page.max(1).to_string()),
        ("pagesize", page_size.clamp(1, 50).to_string()),
        ("type", "singer".to_string()),
        ("keywords", q.to_string()),
    ];
    let json = get_json(http, &url, &params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(vec![]);
    }
    let list = json
        .pointer("/data/lists")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it.get("AuthorId").or_else(|| it.get("authorId")).and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let name = it.get("AuthorName").or_else(|| it.get("authorName")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let cover = it
            .get("Avatar")
            .or_else(|| it.get("avatar"))
            .and_then(|v| v.as_str())
            .map(|s| s.replace("{size}", "480"));
        let album_count = it
            .get("AlbumCount")
            .or_else(|| it.get("albumCount"))
            .and_then(|v| v.as_i64())
            .and_then(|n| u32::try_from(n).ok());
        out.push(MusicArtist {
            service: MusicService::Kugou,
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
    let base = base_url(cfg)?;
    let url = format!("{base}/search");
    let params: Vec<(&str, String)> = vec![
        ("page", page.max(1).to_string()),
        ("pagesize", page_size.clamp(1, 50).to_string()),
        ("type", "album".to_string()),
        ("keywords", q.to_string()),
    ];
    let json = get_json(http, &url, &params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(vec![]);
    }
    let list = json
        .pointer("/data/lists")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it
            .get("AlbumID")
            .or_else(|| it.get("albumID"))
            .or_else(|| it.get("albumId"))
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let title = it.get("AlbumName").or_else(|| it.get("albumName")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.get("AuthorName").or_else(|| it.get("authorName")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let artist_id = it.get("AuthorId").or_else(|| it.get("authorId")).and_then(|v| v.as_i64()).map(|n| n.to_string());
        let cover = it
            .get("SizableCover")
            .or_else(|| it.get("sizableCover"))
            .or_else(|| it.get("Cover"))
            .or_else(|| it.get("cover"))
            .and_then(|v| v.as_str())
            .map(|s| s.replace("{size}", "480"));
        out.push(MusicAlbum {
            service: MusicService::Kugou,
            id,
            title,
            artist,
            artist_id,
            cover_url: cover,
            publish_time: it.get("PublishDate").or_else(|| it.get("publishDate")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            track_count: it.get("SongCount").or_else(|| it.get("songCount")).and_then(|v| v.as_i64()).and_then(|n| u32::try_from(n).ok()),
        });
    }
    Ok(out)
}

pub async fn artist_albums(
    http: &Client,
    cfg: &ProviderConfig,
    artist_id: &str,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let base = base_url(cfg)?;
    let id = artist_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty artist_id".to_string()));
    }
    let url = format!("{base}/artist/albums");
    let params: Vec<(&str, String)> = vec![
        ("id", id.to_string()),
        ("page", "1".to_string()),
        ("pagesize", "10000".to_string()),
        ("sort", "new".to_string()),
    ];
    let json = get_json(http, &url, &params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(vec![]);
    }
    let list = json
        .pointer("/data/info")
        .or_else(|| json.pointer("/data/list"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for it in list {
        let album_id = it.get("album_id").or_else(|| it.get("albumId")).and_then(|v| v.as_str()).map(|s| s.to_string())
            .or_else(|| it.get("album_id").and_then(|v| v.as_i64()).map(|n| n.to_string()))
            .unwrap_or_default();
        if album_id.is_empty() {
            continue;
        }
        let title = it.get("album_name").or_else(|| it.get("albumName")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let cover = it.get("sizable_cover").or_else(|| it.get("sizableCover")).and_then(|v| v.as_str()).map(|s| s.replace("{size}", "480"));
        out.push(MusicAlbum {
            service: MusicService::Kugou,
            id: album_id,
            title,
            artist: None,
            artist_id: Some(id.to_string()),
            cover_url: cover,
            publish_time: it.get("publish_date").or_else(|| it.get("publishDate")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            track_count: None,
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
    let base = base_url(cfg)?;
    let id = album_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty album_id".to_string()));
    }

    // Paginate /album/songs
    let url = format!("{base}/album/songs");
    let mut page = 1u32;
    let page_size = 100u32;
    let mut out: Vec<MusicTrack> = Vec::new();
    loop {
        let params: Vec<(&str, String)> = vec![
            ("id", id.to_string()),
            ("page", page.to_string()),
            ("page_size", page_size.to_string()),
        ];
        let json = get_json(http, &url, &params, timeout).await?;
        let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
        if status != 1 {
            break;
        }
        let total = json
            .get("total")
            .or_else(|| json.pointer("/data/total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let songs = json
            .pointer("/data/songs")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if songs.is_empty() {
            break;
        }

        for s in songs {
            let audio = s.get("audio_info").or_else(|| s.get("audioInfo")).unwrap_or(&Value::Null);
            let base_node = s.get("base").unwrap_or(&Value::Null);
            let album_info = s.get("album_info").or_else(|| s.get("albumInfo")).unwrap_or(&Value::Null);
            let trans = s.get("trans_param").or_else(|| s.get("transParam")).unwrap_or(&Value::Null);

            let hash = audio.get("hash").or_else(|| audio.get("Hash")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            if hash.is_empty() {
                continue;
            }
            let title = base_node.get("audio_name").or_else(|| base_node.get("audioName")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let duration_ms = audio.get("duration").and_then(|v| v.as_i64()).map(|d| (d.max(0) as u64) * 1000);
            let album_name = album_info.get("album_name").or_else(|| album_info.get("albumName")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let album_id = base_node.get("album_id").or_else(|| base_node.get("albumId")).and_then(|v| v.as_i64()).map(|n| n.to_string());
            let cover = trans.get("union_cover").or_else(|| trans.get("unionCover")).and_then(|v| v.as_str()).map(|s| s.replace("{size}", "400"));

            let mut artists = Vec::new();
            let mut artist_ids = Vec::new();
            if let Some(arr) = s.get("authors").and_then(|v| v.as_array()) {
                for a in arr {
                    if let Some(n) = a.get("author_name").or_else(|| a.get("authorName")).and_then(|v| v.as_str()) {
                        if !n.trim().is_empty() {
                            artists.push(n.to_string());
                        }
                    }
                    if let Some(i) = a.get("author_id").or_else(|| a.get("authorId")).and_then(|v| v.as_i64()) {
                        artist_ids.push(i.to_string());
                    }
                }
            }

            let mut qualities = Vec::new();
            let hash_320 = audio.get("hash320").or_else(|| audio.get("hash_320")).and_then(|v| v.as_str()).unwrap_or("");
            let hash_flac = audio.get("hash_flac").or_else(|| audio.get("hashFlac")).and_then(|v| v.as_str()).unwrap_or("");
            if !hash.is_empty() {
                qualities.push(MusicQuality { id: "mp3_128".to_string(), label: "MP3 128".to_string(), format: "mp3".to_string(), bitrate_kbps: Some(128), lossless: false });
            }
            if !hash_320.trim().is_empty() {
                qualities.push(MusicQuality { id: "mp3_320".to_string(), label: "MP3 320".to_string(), format: "mp3".to_string(), bitrate_kbps: Some(320), lossless: false });
            }
            if !hash_flac.trim().is_empty() {
                qualities.push(MusicQuality { id: "flac".to_string(), label: "FLAC".to_string(), format: "flac".to_string(), bitrate_kbps: Some(2000), lossless: true });
            }

            out.push(MusicTrack {
                service: MusicService::Kugou,
                id: hash,
                title,
                artists,
                artist_ids,
                album: album_name,
                album_id,
                duration_ms,
                cover_url: cover,
                qualities,
            });
        }

        let fetched = out.len() as i64;
        if total > 0 && fetched >= total {
            break;
        }
        page += 1;
        if page > 200 {
            break;
        }
    }

    Ok(out)
}

pub async fn track_download_url(
    http: &Client,
    cfg: &ProviderConfig,
    track_hash: &str,
    quality_id: &str,
    auth: &AuthState,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    let base = base_url(cfg)?;
    let hash = track_hash.trim();
    if hash.is_empty() {
        return Err(MusicError::InvalidInput("empty track_id".to_string()));
    }

    let quality = match quality_id.trim() {
        "mp3_128" => "128kmp3",
        "mp3_320" => "320kmp3",
        "flac" => "2000kflac",
        other => other,
    };

    let url = format!("{base}/song/url");
    let mut params: Vec<(&str, String)> = vec![("hash", hash.to_string()), ("quality", quality.to_string())];
    let Some(cookie) = cookie_from_auth(auth) else {
        return Err(MusicError::Unauthorized(
            "kugou: missing token/userid".to_string(),
        ));
    };
    params.push(("cookie", cookie));

    let json = get_json(http, &url, &params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Err(MusicError::Other("kugou: download url status!=1".to_string()));
    }

    // download result: url: [ ... ], extName, bitRate
    let mut chosen = String::new();
    if let Some(arr) = json.get("url").and_then(|v| v.as_array()) {
        for u in arr {
            let s = u.as_str().unwrap_or("").trim();
            if !s.is_empty() {
                chosen = s.to_string();
                break;
            }
        }
    }
    if chosen.is_empty() {
        return Err(MusicError::Other("kugou: empty url list".to_string()));
    }

    let ext = json.get("extName").and_then(|v| v.as_str()).unwrap_or("mp3").trim().to_string();
    Ok((chosen, ext))
}

// Daemon-facing QR login helpers (kugou API baseUrl).

#[derive(Debug, Clone)]
pub struct KugouQr {
    pub key: String,
    pub image_base64: String,
}

pub async fn kugou_qr_create(
    http: &Client,
    cfg: &ProviderConfig,
    timeout: Duration,
) -> Result<KugouQr, MusicError> {
    let base = base_url(cfg)?;
    let url = format!("{base}/login/qr/key");
    let json = get_json(http, &url, &[], timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Err(MusicError::Other("kugou: qr/key status!=1".to_string()));
    }
    let key = json.pointer("/data/qrcode").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let img = json.pointer("/data/qrcode_img").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if key.is_empty() || img.is_empty() {
        return Err(MusicError::Parse("kugou: missing qr key/img".to_string()));
    }
    Ok(KugouQr { key, image_base64: img })
}

pub async fn kugou_qr_poll(
    http: &Client,
    cfg: &ProviderConfig,
    key: &str,
    timeout: Duration,
) -> Result<Option<KugouUserInfo>, MusicError> {
    let base = base_url(cfg)?;
    let url = format!("{base}/login/qr/check");
    let json = get_json(http, &url, &[("key", key.trim().to_string())], timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(None);
    }
    let qrstatus = json.pointer("/data/status").and_then(|v| v.as_i64()).unwrap_or(0);
    if qrstatus != 4 {
        return Ok(None);
    }
    let token = json.pointer("/data/token").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let userid = json.pointer("/data/userid").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if token.is_empty() || userid.is_empty() {
        return Ok(None);
    }
    Ok(Some(KugouUserInfo { token, userid }))
}

pub async fn kugou_wx_qr_create(
    http: &Client,
    cfg: &ProviderConfig,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    // returns (uuid, qrcode_base64_data_uri)
    let base = base_url(cfg)?;
    let url = format!("{base}/login/wx/create");
    let json = get_json(http, &url, &[], timeout).await?;
    let err = json.get("errcode").and_then(|v| v.as_i64()).unwrap_or(-1);
    if err != 0 {
        return Err(MusicError::Other(format!("kugou: wx/create errcode={err}")));
    }
    let uuid = json.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let img = json.pointer("/qrcode/qrcodebase64").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if uuid.is_empty() || img.is_empty() {
        return Err(MusicError::Parse("kugou: missing wx uuid/img".to_string()));
    }
    Ok((uuid, format!("data:image/jpeg;base64,{img}")))
}

pub async fn kugou_wx_qr_poll(
    http: &Client,
    cfg: &ProviderConfig,
    uuid: &str,
    timeout: Duration,
) -> Result<Option<KugouUserInfo>, MusicError> {
    let base = base_url(cfg)?;
    let url = format!("{base}/login/wx/check");
    let ts = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs())
    .to_string();
    let json = get_json(
        http,
        &url,
        &[("uuid", uuid.to_string()), ("timestamp", ts)],
        timeout,
    )
    .await?;
    let wx_code = json.get("wx_code").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if wx_code.is_empty() {
        return Ok(None);
    }
    // exchange openplat
    let open = format!("{base}/login/openplat");
    let res = get_json(http, &open, &[("code", wx_code)], timeout).await?;
    let status = res.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(None);
    }
    // `data` is a JSON string in refs; accept both string/object.
    if let Some(s) = res.get("data").and_then(|v| v.as_str()) {
        let v: Value = serde_json::from_str(s).unwrap_or(Value::Null);
        let token = v.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let userid = v.get("userid").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default();
        if !token.is_empty() && !userid.is_empty() {
            return Ok(Some(KugouUserInfo { token, userid }));
        }
    }
    let token = res.pointer("/data/token").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let userid = res.pointer("/data/userid").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default();
    if token.is_empty() || userid.is_empty() {
        return Ok(None);
    }
    Ok(Some(KugouUserInfo { token, userid }))
}
