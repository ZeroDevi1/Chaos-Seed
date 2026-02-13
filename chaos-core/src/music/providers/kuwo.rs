use std::time::Duration;

use reqwest::Client;
use serde_json::Value;

use crate::music::error::MusicError;
use crate::music::model::{MusicAlbum, MusicArtist, MusicQuality, MusicService, MusicTrack};

const SEARCH_URL: &str = "http://search.kuwo.cn/r.s?client=kt&encoding=utf8&rformat=json&mobi=1&vipver=1&pn={pn}&rn={rn}&correct=1&all={q}&ft={ft}";
const DOWNLOAD_URL2: &str = "https://mobi.kuwo.cn/mobi.s?f=web&user=0&source=kwplayer_ar_5.0.0.0_B_jiakong_vh.apk&type=convert_url_with_sign&rid={rid}&br={br}";
const ALBUM_INFO_URL: &str = "https://search.kuwo.cn/r.s?pn={pn}&rn={rn}&albumid={albumid}&stype=albuminfo&show_copyright_off=1&alflac=1&pcmp4=1&encoding=utf8&plat=pc&vipver=MUSIC_9.1.1.2_BCS2&devid=38668888&newver=1&pcjson=1";
const ARTIST_ALBUM_LIST_URL: &str = "https://search.kuwo.cn/r.s?pn=0&rn=10000&artistid={artistid}&stype=albumlist&sortby=1&alflac=1&show_copyright_off=1&pcmp4=1&encoding=utf8&plat=pc&thost=search.kuwo.cn&vipver=MUSIC_9.1.1.2_BCS2&devid=38668888&pcjson=1";

const COVER_PREFIX: &str = "https://img3.kuwo.cn/star/albumcover/";
const ARTIST_COVER_PREFIX: &str = "https://star.kuwo.cn/star/starheads/";

fn map_br(quality_id: &str) -> &'static str {
    match quality_id {
        "mp3_128" => "128kmp3",
        "mp3_320" => "320kmp3",
        "flac" => "2000kflac",
        _ => "320kmp3",
    }
}

fn parse_nm_info_to_qualities(nm_info: &str) -> Vec<MusicQuality> {
    let mut out = Vec::new();
    for part in nm_info.split(';') {
        for kv in part.split(',') {
            let mut it = kv.split(':');
            let k = it.next().unwrap_or("").trim();
            let v = it.next().unwrap_or("").trim();
            if k != "bitrate" {
                continue;
            }
            match v {
                "128" => out.push(MusicQuality {
                    id: "mp3_128".to_string(),
                    label: "MP3 128".to_string(),
                    format: "mp3".to_string(),
                    bitrate_kbps: Some(128),
                    lossless: false,
                }),
                "320" => out.push(MusicQuality {
                    id: "mp3_320".to_string(),
                    label: "MP3 320".to_string(),
                    format: "mp3".to_string(),
                    bitrate_kbps: Some(320),
                    lossless: false,
                }),
                "2000" => out.push(MusicQuality {
                    id: "flac".to_string(),
                    label: "FLAC".to_string(),
                    format: "flac".to_string(),
                    bitrate_kbps: Some(2000),
                    lossless: true,
                }),
                _ => {}
            }
        }
    }
    out.sort_by_key(|q| q.id.clone());
    out.dedup_by(|a, b| a.id == b.id);
    out
}

async fn get_json(http: &Client, url: &str, timeout: Duration) -> Result<Value, MusicError> {
    let resp = http
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .timeout(timeout)
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json::<Value>().await?)
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
    let pn = page.saturating_sub(1);
    let rn = page_size.clamp(1, 100);
    let q_enc = urlencoding::encode(q);
    let url = SEARCH_URL
        .replace("{pn}", &pn.to_string())
        .replace("{rn}", &rn.to_string())
        .replace("{q}", q_enc.as_ref())
        .replace("{ft}", "music");
    let json = get_json(http, &url, timeout).await?;
    let list = json
        .get("abslist")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let rid = it.get("MUSICRID").or_else(|| it.get("musicrid")).and_then(|v| v.as_str()).unwrap_or("");
        let id = rid.trim().trim_start_matches("MUSIC_").to_string();
        if id.is_empty() {
            continue;
        }
        let title = it.get("NAME").or_else(|| it.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.get("ARTIST").or_else(|| it.get("artist")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artists: Vec<String> = artist
            .split('&')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let artist_ids: Vec<String> = it
            .get("ALLARTISTID")
            .or_else(|| it.get("allartistid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .split('&')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let album = it.get("ALBUM").or_else(|| it.get("album")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let album_id = it.get("ALBUMID").or_else(|| it.get("albumid")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let duration_ms = it
            .get("DURATION")
            .or_else(|| it.get("duration"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .map(|s| s * 1000);
        let nm = it.get("NMINFO").or_else(|| it.get("N_MINFO")).or_else(|| it.get("nMinfo")).and_then(|v| v.as_str()).unwrap_or("");
        let qualities = parse_nm_info_to_qualities(nm);

        let cover = it
            .get("web_albumpic_short")
            .or_else(|| it.get("webAlbumpicShort"))
            .and_then(|v| v.as_str())
            .map(|s| format!("{}{}", COVER_PREFIX, s).replace("/120", "/500"));

        out.push(MusicTrack {
            service: MusicService::Kuwo,
            id,
            title,
            artists,
            artist_ids,
            album,
            album_id,
            duration_ms,
            cover_url: cover,
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
    let pn = page.saturating_sub(1);
    let rn = page_size.clamp(1, 100);
    let q_enc = urlencoding::encode(q);
    let url = SEARCH_URL
        .replace("{pn}", &pn.to_string())
        .replace("{rn}", &rn.to_string())
        .replace("{q}", q_enc.as_ref())
        .replace("{ft}", "artist");
    let json = get_json(http, &url, timeout).await?;
    let list = json
        .get("abslist")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it.get("ARTISTID").or_else(|| it.get("artistid")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let name = it.get("ARTIST").or_else(|| it.get("artist")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let cover = it
            .get("hts_picpath")
            .or_else(|| it.get("htsPicpath"))
            .and_then(|v| v.as_str())
            .map(|s| s.replace("/120", "/500"))
            .or_else(|| Some(format!("{ARTIST_COVER_PREFIX}{id}").to_string()));
        out.push(MusicArtist {
            service: MusicService::Kuwo,
            id,
            name,
            cover_url: cover,
            album_count: None,
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
    let pn = page.saturating_sub(1);
    let rn = page_size.clamp(1, 100);
    let q_enc = urlencoding::encode(q);
    let url = SEARCH_URL
        .replace("{pn}", &pn.to_string())
        .replace("{rn}", &rn.to_string())
        .replace("{q}", q_enc.as_ref())
        .replace("{ft}", "album");
    let json = get_json(http, &url, timeout).await?;
    let list = json
        .get("albumlist")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let id = it.get("albumid").or_else(|| it.get("ALBUMID")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = it.get("name").or_else(|| it.get("NAME")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.get("artist").or_else(|| it.get("ARTIST")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let artist_id = it.get("artistid").or_else(|| it.get("ARTISTID")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let cover = it
            .get("pic")
            .and_then(|v| v.as_str())
            .map(|s| format!("{}{}", COVER_PREFIX, s).replace("/120", "/500"));
        out.push(MusicAlbum {
            service: MusicService::Kuwo,
            id,
            title,
            artist,
            artist_id,
            cover_url: cover,
            publish_time: it.get("pub").and_then(|v| v.as_str()).map(|s| s.to_string()),
            track_count: it
                .get("musiccnt")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok()),
        });
    }
    Ok(out)
}

pub async fn album_tracks(
    http: &Client,
    album_id: &str,
    timeout: Duration,
) -> Result<Vec<MusicTrack>, MusicError> {
    let id = album_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty album_id".to_string()));
    }
    let url = ALBUM_INFO_URL
        .replace("{pn}", "0")
        .replace("{rn}", "10000")
        .replace("{albumid}", id);
    let json = get_json(http, &url, timeout).await?;
    let album_title = json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let album_id = json.get("albumid").or_else(|| json.get("id")).and_then(|v| v.as_str()).map(|s| s.to_string());
    let list = json
        .get("musiclist")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let track_id = it.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if track_id.is_empty() {
            continue;
        }
        let title = it.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.get("artist").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artists: Vec<String> = artist
            .split('&')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let artist_ids: Vec<String> = it
            .get("allartistid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .split('&')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let duration_ms = it
            .get("duration")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .map(|s| s * 1000);
        let nm = it
            .get("N_MINFO")
            .or_else(|| it.get("nMinfo"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let qualities = parse_nm_info_to_qualities(nm);
        let cover = it
            .get("web_albumpic_short")
            .and_then(|v| v.as_str())
            .map(|s| format!("{}{}", COVER_PREFIX, s).replace("/120", "/500"));

        out.push(MusicTrack {
            service: MusicService::Kuwo,
            id: track_id,
            title,
            artists,
            artist_ids,
            album: album_title.clone(),
            album_id: album_id.clone(),
            duration_ms,
            cover_url: cover,
            qualities,
        });
    }
    Ok(out)
}

pub async fn artist_albums(
    http: &Client,
    artist_id: &str,
    timeout: Duration,
) -> Result<Vec<MusicAlbum>, MusicError> {
    let id = artist_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty artist_id".to_string()));
    }
    let url = ARTIST_ALBUM_LIST_URL.replace("{artistid}", id);
    let json = get_json(http, &url, timeout).await?;
    let list = json
        .get("albumlist")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let album_id = it.get("albumid").or_else(|| it.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if album_id.is_empty() {
            continue;
        }
        let title = it.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artist = it.get("artist").and_then(|v| v.as_str()).map(|s| s.to_string());
        let artist_id = it.get("artistid").and_then(|v| v.as_str()).map(|s| s.to_string());
        let cover = it
            .get("pic")
            .and_then(|v| v.as_str())
            .map(|s| format!("{}{}", COVER_PREFIX, s).replace("/120", "/500"));
        out.push(MusicAlbum {
            service: MusicService::Kuwo,
            id: album_id,
            title,
            artist,
            artist_id,
            cover_url: cover,
            publish_time: it.get("pub").and_then(|v| v.as_str()).map(|s| s.to_string()),
            track_count: it
                .get("musiccnt")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok()),
        });
    }
    Ok(out)
}

pub async fn track_download_url(
    http: &Client,
    track_id: &str,
    quality_id: &str,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    track_download_url_with_template(http, DOWNLOAD_URL2, track_id, quality_id, timeout).await
}

#[doc(hidden)]
pub async fn track_download_url_with_template(
    http: &Client,
    template: &str,
    track_id: &str,
    quality_id: &str,
    timeout: Duration,
) -> Result<(String, String), MusicError> {
    let id = track_id.trim();
    if id.is_empty() {
        return Err(MusicError::InvalidInput("empty track_id".to_string()));
    }
    let br = map_br(quality_id.trim());
    let url = template
        .replace("{rid}", id)
        .replace("{br}", br);
    let json = get_json(http, &url, timeout).await?;
    let url = json
        .pointer("/data/url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(MusicError::Other("kuwo: empty download url".to_string()));
    }
    let fmt = json
        .pointer("/data/format")
        .and_then(|v| v.as_str())
        .unwrap_or("mp3")
        .trim()
        .to_string();
    Ok((url, fmt))
}
