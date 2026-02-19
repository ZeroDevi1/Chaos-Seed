use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::Client;
use serde_json::{Value, json};

use crate::music::error::MusicError;
use crate::music::model::{
    AuthState, KugouUserInfo, MusicAlbum, MusicArtist, MusicQuality, MusicService, MusicTrack,
    ProviderConfig,
};

mod client;
pub mod signatures;

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
    _cfg: &ProviderConfig,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicTrack>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let kg = client::KugouClient::new(http);
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("albumhide".to_string(), "0".to_string());
    params.insert("iscorrection".to_string(), "1".to_string());
    params.insert("keyword".to_string(), q.to_string());
    params.insert("nocollect".to_string(), "0".to_string());
    params.insert("page".to_string(), page.max(1).to_string());
    params.insert("pagesize".to_string(), page_size.clamp(1, 50).to_string());
    params.insert("platform".to_string(), "AndroidFilter".to_string());
    let json = kg
        .gateway_get("/v3/search/song", "complexsearch.kugou.com", params, None, timeout)
        .await?;
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
    _cfg: &ProviderConfig,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicArtist>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let kg = client::KugouClient::new(http);
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("albumhide".to_string(), "0".to_string());
    params.insert("iscorrection".to_string(), "1".to_string());
    params.insert("keyword".to_string(), q.to_string());
    params.insert("nocollect".to_string(), "0".to_string());
    params.insert("page".to_string(), page.max(1).to_string());
    params.insert("pagesize".to_string(), page_size.clamp(1, 50).to_string());
    params.insert("platform".to_string(), "AndroidFilter".to_string());
    let json = kg
        .gateway_get("/v1/search/author", "complexsearch.kugou.com", params, None, timeout)
        .await?;
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
    _cfg: &ProviderConfig,
    keyword: &str,
    page: u32,
    page_size: u32,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let kg = client::KugouClient::new(http);
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("albumhide".to_string(), "0".to_string());
    params.insert("iscorrection".to_string(), "1".to_string());
    params.insert("keyword".to_string(), q.to_string());
    params.insert("nocollect".to_string(), "0".to_string());
    params.insert("page".to_string(), page.max(1).to_string());
    params.insert("pagesize".to_string(), page_size.clamp(1, 50).to_string());
    params.insert("platform".to_string(), "AndroidFilter".to_string());
    let json = kg
        .gateway_get("/v1/search/album", "complexsearch.kugou.com", params, None, timeout)
        .await?;
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
    _cfg: &ProviderConfig,
    artist_id: &str,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let id = artist_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty artist_id".to_string()));
    }
    let kg = client::KugouClient::new(http);
    let payload = json!({
        "author_id": id,
        "pagesize": 10000,
        "page": 1,
        "sort": 1,
        "category": 1,
        "area_code": "all"
    });
    let json = kg
        .gateway_post("/kmr/v1/author/albums", "openapi.kugou.com", Some("36"), &payload, None, timeout)
        .await?;
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
    _cfg: &ProviderConfig,
    album_id: &str,
    timeout: Duration,
) -> Result<Vec<MusicTrack>, MusicError> {
    let id = album_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty album_id".to_string()));
    }

    let kg = client::KugouClient::new(http);
    let mut page = 1u32;
    let page_size = 100u32;
    let mut out: Vec<MusicTrack> = Vec::new();
    loop {
        let payload = json!({
            "album_id": id,
            "is_buy": "",
            "page": page,
            "pagesize": page_size
        });
        let json = kg
            .gateway_post("/v1/album_audio/lite", "openapi.kugou.com", Some("255"), &payload, None, timeout)
            .await?;
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
    _cfg: &ProviderConfig,
    track_hash: &str,
    quality_id: &str,
    auth: &AuthState,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    let hash = track_hash.trim();
    if hash.is_empty() {
        return Err(MusicError::InvalidInput("empty track_id".to_string()));
    }

    let Some(user) = auth.kugou.as_ref() else {
        return Err(MusicError::Unauthorized("kugou: missing token/userid".to_string()));
    };
    let token = user.token.trim();
    let userid = user.userid.trim();
    if token.is_empty() || userid.is_empty() {
        return Err(MusicError::Unauthorized("kugou: missing token/userid".to_string()));
    }

    let kg = client::KugouClient::new(http);
    let desired_ext = if quality_id.trim() == "flac" { "flac" } else { "mp3" };
    let qualities = match quality_id.trim() {
        "flac" => vec!["flac", "320", "128"],
        "mp3_320" => vec!["320", "128"],
        "mp3_128" => vec!["128"],
        _ => vec!["320", "128"],
    };

    let tracker_key = signatures::sign_key_lite(hash, &kg.device_mid(), userid, kg.appid());
    let collect_time = kg.now_ms();

    let payload = json!({
        "area_code": "1",
        "behavior": "play",
        "qualities": qualities,
        "resource": {
            "album_audio_id": 0,
            "collect_list_id": "3",
            "collect_time": collect_time,
            "hash": hash,
            "id": 0,
            "page_id": 1,
            "type": "audio"
        },
        "token": token,
        "tracker_param": {
            "all_m": 1,
            "auth": "",
            "is_free_part": 0,
            "key": tracker_key,
            "module_id": 0,
            "need_climax": 1,
            "need_xcdn": 1,
            "open_time": "",
            "pid": "411",
            "pidversion": "3001",
            "priv_vip_type": "6",
            "viptoken": ""
        },
        "userid": userid,
        "vip": 0
    });

    let json = kg.tracker_post("/v6/priv_url", &payload, Some(auth), timeout).await?;

    fn collect_urls(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => {
                let s = s.trim();
                if s.starts_with("http") {
                    out.push(s.to_string());
                }
            }
            Value::Array(arr) => {
                for x in arr {
                    collect_urls(x, out);
                }
            }
            Value::Object(map) => {
                for (_k, x) in map {
                    collect_urls(x, out);
                }
            }
            _ => {}
        }
    }

    let mut urls: Vec<String> = Vec::new();
    collect_urls(&json, &mut urls);
    let chosen = urls
        .iter()
        .find(|u| u.to_lowercase().contains(&format!(".{desired_ext}")))
        .cloned()
        .or_else(|| urls.first().cloned())
        .unwrap_or_default();
    if chosen.is_empty() {
        return Err(MusicError::Other("kugou: empty download url".to_string()));
    }

    let ext = if chosen.to_lowercase().contains(".flac") {
        "flac".to_string()
    } else if chosen.to_lowercase().contains(".mp3") {
        "mp3".to_string()
    } else {
        desired_ext.to_string()
    };
    Ok((chosen, ext))
}

// Daemon-facing QR login helpers (direct login-user.kugou.com, no secrets).

#[derive(Debug, Clone)]
pub struct KugouQr {
    pub key: String,
    pub image_base64: String,
}

pub async fn kugou_qr_create(
    http: &Client,
    _cfg: &ProviderConfig,
    timeout: Duration,
) -> Result<KugouQr, MusicError> {
    let kg = client::KugouClient::new(http);
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("appid".to_string(), "1001".to_string()); // QQ
    params.insert("type".to_string(), "1".to_string());
    params.insert("plat".to_string(), "4".to_string());
    params.insert(
        "qrcode_txt".to_string(),
        format!(
            "https://h5.kugou.com/apps/loginQRCode/html/index.html?appid={}&",
            kg.appid()
        ),
    );
    params.insert("srcappid".to_string(), kg.srcappid().to_string());

    let json = kg.login_user_get("/v2/qrcode", params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Err(MusicError::Other(format!("kugou: qrcode status={status}")));
    }
    let key = json.pointer("/data/qrcode").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let img = json.pointer("/data/qrcode_img").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if key.is_empty() || img.is_empty() {
        return Err(MusicError::Parse("kugou: missing qrcode/qrcode_img".to_string()));
    }
    Ok(KugouQr { key, image_base64: img })
}

pub async fn kugou_qr_poll(
    http: &Client,
    _cfg: &ProviderConfig,
    key: &str,
    timeout: Duration,
) -> Result<Option<KugouUserInfo>, MusicError> {
    let kg = client::KugouClient::new(http);
    let q = key.trim();
    if q.is_empty() {
        return Err(MusicError::InvalidInput("empty key".to_string()));
    }
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("plat".to_string(), "4".to_string());
    params.insert("appid".to_string(), "1001".to_string()); // QQ
    params.insert("srcappid".to_string(), kg.srcappid().to_string());
    params.insert("qrcode".to_string(), q.to_string());

    let json = kg.login_user_get("/v2/get_userinfo_qrcode", params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(None);
    }
    let qrstatus = json.pointer("/data/status").and_then(|v| v.as_i64()).unwrap_or(0);
    if qrstatus != 4 {
        return Ok(None);
    }
    let token = json.pointer("/data/token").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let userid = json.pointer("/data/userid").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if token.is_empty() || userid.is_empty() {
        return Ok(None);
    }
    Ok(Some(KugouUserInfo { token, userid }))
}

pub async fn kugou_wx_qr_create(
    http: &Client,
    _cfg: &ProviderConfig,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    // returns (key, qrcode_base64_data_uri)
    let kg = client::KugouClient::new(http);
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("appid".to_string(), "1014".to_string()); // Wechat button mapped to kugou appid=1014
    params.insert("type".to_string(), "1".to_string());
    params.insert("plat".to_string(), "4".to_string());
    params.insert(
        "qrcode_txt".to_string(),
        format!(
            "https://h5.kugou.com/apps/loginQRCode/html/index.html?appid={}&",
            kg.appid()
        ),
    );
    params.insert("srcappid".to_string(), kg.srcappid().to_string());

    let json = kg.login_user_get("/v2/qrcode", params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Err(MusicError::Other(format!("kugou: qrcode status={status}")));
    }
    let key = json.pointer("/data/qrcode").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let img = json.pointer("/data/qrcode_img").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if key.is_empty() || img.is_empty() {
        return Err(MusicError::Parse("kugou: missing qrcode/qrcode_img".to_string()));
    }
    Ok((key, format!("data:image/png;base64,{img}")))
}

pub async fn kugou_wx_qr_poll(
    http: &Client,
    _cfg: &ProviderConfig,
    uuid: &str,
    timeout: Duration,
) -> Result<Option<KugouUserInfo>, MusicError> {
    let kg = client::KugouClient::new(http);
    let q = uuid.trim();
    if q.is_empty() {
        return Err(MusicError::InvalidInput("empty key".to_string()));
    }
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("plat".to_string(), "4".to_string());
    params.insert("appid".to_string(), "1014".to_string()); // mapped wechat
    params.insert("srcappid".to_string(), kg.srcappid().to_string());
    params.insert("qrcode".to_string(), q.to_string());

    let json = kg.login_user_get("/v2/get_userinfo_qrcode", params, timeout).await?;
    let status = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    if status != 1 {
        return Ok(None);
    }
    let qrstatus = json.pointer("/data/status").and_then(|v| v.as_i64()).unwrap_or(0);
    if qrstatus != 4 {
        return Ok(None);
    }
    let token = json.pointer("/data/token").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let userid = json.pointer("/data/userid").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if token.is_empty() || userid.is_empty() {
        return Ok(None);
    }
    Ok(Some(KugouUserInfo { token, userid }))
}
