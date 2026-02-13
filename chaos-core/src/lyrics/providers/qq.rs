use std::time::Duration;

use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, REFERER};
use serde::Deserialize;

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsService};
use crate::lyrics::util;

#[derive(Debug, Clone)]
pub struct QqMusicProvider {
    search_url: String,
    lyric_url: String,
}

impl Default for QqMusicProvider {
    fn default() -> Self {
        Self {
            search_url: "https://c.y.qq.com/soso/fcgi-bin/client_search_cp".to_string(),
            lyric_url: "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg".to_string(),
        }
    }
}

impl QqMusicProvider {
    pub fn with_base_url(search_url: &str, lyric_url: &str) -> Self {
        Self {
            search_url: search_url.to_string(),
            lyric_url: lyric_url.to_string(),
        }
    }

    pub async fn search(
        &self,
        http: &Client,
        req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<Vec<QqToken>, LyricsError> {
        let term = req.term.description();
        if term.trim().is_empty() {
            return Ok(vec![]);
        }

        let resp = http
            .get(&self.search_url)
            .query(&[("w", term.as_str())])
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        let bytes = resp.bytes().await?;
        let json = util::extract_json_from_jsonp(&bytes)
            .ok_or_else(|| LyricsError::Parse("qq: failed to extract jsonp".to_string()))?;

        let data: QqSearchResp = serde_json::from_str(&json)?;
        Ok(data
            .data
            .song
            .list
            .into_iter()
            .map(|it| QqToken {
                songmid: it.songmid,
                title: it.songname,
                album: it.albumname,
                artist: it
                    .singer
                    .first()
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
                duration_ms: (it.interval.max(0) as u64) * 1000,
            })
            .collect())
    }

    pub async fn fetch(
        &self,
        http: &Client,
        token: QqToken,
        _req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<LyricsSearchResult, LyricsError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            REFERER,
            HeaderValue::from_static("y.qq.com/portal/player.html"),
        );

        let resp = http
            .get(&self.lyric_url)
            .headers(headers)
            .query(&[("songmid", token.songmid.as_str()), ("g_tk", "5381")])
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;
        let json = util::extract_json_from_jsonp(&bytes)
            .ok_or_else(|| LyricsError::Parse("qq: failed to extract jsonp".to_string()))?;

        let data: QqLyricResp = serde_json::from_str(&json)?;

        let lyric_raw = decode_b64(&data.lyric)
            .map_err(|e| LyricsError::Parse(format!("qq: base64 lyric decode: {e}")))?;
        let lyric_str = String::from_utf8_lossy(&lyric_raw);
        let lyric_str = util::decode_xml_entities(&lyric_str).into_owned();
        if lyric_str.trim().is_empty() {
            return Err(LyricsError::Parse("qq: empty lyric content".to_string()));
        }

        let trans_str = match &data.trans {
            Some(s) if !s.trim().is_empty() => {
                let raw = decode_b64(s)
                    .map_err(|e| LyricsError::Parse(format!("qq: base64 trans decode: {e}")))?;
                let t = String::from_utf8_lossy(&raw);
                let t = util::decode_xml_entities(&t).into_owned();
                (!t.trim().is_empty()).then_some(t)
            }
            _ => None,
        };

        Ok(LyricsSearchResult {
            service: LyricsService::QQMusic,
            service_token: token.songmid.clone(),
            title: Some(token.title),
            artist: Some(token.artist),
            album: Some(token.album),
            duration_ms: Some(token.duration_ms),
            match_percentage: 0,
            quality: 0.0,
            matched: false,
            has_translation: trans_str.is_some(),
            has_inline_timetags: false,
            lyrics_original: lyric_str,
            lyrics_translation: trans_str,
            debug: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct QqToken {
    pub(crate) songmid: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) duration_ms: u64,
}

#[derive(Debug, Deserialize)]
struct QqSearchResp {
    data: QqSearchData,
}

#[derive(Debug, Deserialize)]
struct QqSearchData {
    song: QqSearchSong,
}

#[derive(Debug, Deserialize)]
struct QqSearchSong {
    list: Vec<QqSongItem>,
}

#[derive(Debug, Deserialize)]
struct QqSongItem {
    songmid: String,
    songname: String,
    albumname: String,
    singer: Vec<QqSinger>,
    interval: i64,
}

#[derive(Debug, Deserialize)]
struct QqSinger {
    name: String,
}

#[derive(Debug, Deserialize)]
struct QqLyricResp {
    lyric: String,
    #[serde(default)]
    trans: Option<String>,
}

fn decode_b64(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(s.trim())
}
