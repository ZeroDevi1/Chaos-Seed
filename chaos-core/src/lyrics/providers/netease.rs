use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, SET_COOKIE, USER_AGENT};
use serde::Deserialize;

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsService};
#[derive(Debug, Clone)]
pub struct NeteaseProvider {
    search_base_url: String,
    lyric_base_url: String,
    user_agent: String,
}

impl Default for NeteaseProvider {
    fn default() -> Self {
        Self {
            search_base_url: "http://music.163.com/api/search/pc".to_string(),
            lyric_base_url: "http://music.163.com/api/song/lyric".to_string(),
            // Desktop UA; used by LyricsKit as well.
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.4 Safari/605.1.15".to_string(),
        }
    }
}

impl NeteaseProvider {
    pub fn with_base_url(search_base_url: &str, lyric_base_url: &str) -> Self {
        Self {
            search_base_url: search_base_url.trim_end_matches('/').to_string(),
            lyric_base_url: lyric_base_url.trim_end_matches('/').to_string(),
            ..Self::default()
        }
    }

    pub async fn search(
        &self,
        http: &Client,
        req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<Vec<NeteaseToken>, LyricsError> {
        let term = req.term.description();
        if term.trim().is_empty() {
            return Ok(vec![]);
        }

        let mut headers = HeaderMap::new();
        headers.insert(REFERER, HeaderValue::from_static("http://music.163.com/"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent)
                .unwrap_or(HeaderValue::from_static("chaos-seed/0.1")),
        );

        let params = [
            ("s", term.as_str()),
            ("offset", "0"),
            ("limit", "10"),
            ("type", "1"),
        ];

        // First request to obtain cookie (LyricsKit does this explicitly).
        let resp1 = http
            .post(&self.search_base_url)
            .headers(headers.clone())
            .query(&params)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;

        let cookie = resp1
            .headers()
            .get(SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(|v| v.to_string());

        let body1 = resp1.bytes().await?;

        let json_text = if let Some(cookie) = cookie {
            let resp2 = http
                .post(&self.search_base_url)
                .headers(headers)
                .header("Cookie", cookie)
                .query(&params)
                .timeout(timeout)
                .send()
                .await?
                .error_for_status()?;
            String::from_utf8_lossy(&resp2.bytes().await?).to_string()
        } else {
            String::from_utf8_lossy(&body1).to_string()
        };

        let data: NeteaseSearchResp = serde_json::from_str(&json_text)?;
        Ok(data
            .result
            .songs
            .into_iter()
            .map(|s| NeteaseToken {
                id: s.id,
                title: s.name,
                artist: s
                    .artists
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_default(),
                album: s.album.name,
                duration_ms: s.duration.max(0) as u64,
            })
            .collect())
    }

    pub async fn fetch(
        &self,
        http: &Client,
        token: NeteaseToken,
        _req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<LyricsSearchResult, LyricsError> {
        let resp = http
            .get(&self.lyric_base_url)
            .query(&[
                ("id", token.id.to_string()),
                ("lv", "1".to_string()),
                ("kv", "1".to_string()),
                ("tv", "-1".to_string()),
            ])
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;

        let body = resp.bytes().await?;
        let data: NeteaseLyricResp = serde_json::from_slice(&body)?;

        let klyric = data
            .klyric
            .as_ref()
            .and_then(|l| l.lyric.as_deref())
            .map(fix_netease_time_tag);
        let lrc = data
            .lrc
            .as_ref()
            .and_then(|l| l.lyric.as_deref())
            .map(fix_netease_time_tag);

        let mut has_inline_timetags = false;
        let lyrics_original = if let Some(k) = klyric {
            has_inline_timetags = !k.trim().is_empty();
            k
        } else if let Some(l) = lrc {
            l
        } else {
            return Err(LyricsError::Parse(
                "netease: missing lyric content".to_string(),
            ));
        };

        let lyrics_translation = data
            .tlyric
            .as_ref()
            .and_then(|l| l.lyric.as_deref())
            .map(fix_netease_time_tag)
            .and_then(|s| (!s.trim().is_empty()).then_some(s));

        Ok(LyricsSearchResult {
            service: LyricsService::Netease,
            service_token: token.id.to_string(),
            title: Some(token.title),
            artist: Some(token.artist),
            album: Some(token.album),
            duration_ms: Some(token.duration_ms),
            match_percentage: 0,
            quality: 0.0,
            matched: false,
            has_translation: lyrics_translation.is_some(),
            has_inline_timetags,
            lyrics_original,
            lyrics_translation,
            debug: None,
        })
    }
}

fn fix_netease_time_tag(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(\[\d+:\d+):(\d+\])").expect("netease regex"));
    re.replace_all(s, "$1.$2").to_string()
}

#[derive(Debug, Clone)]
pub struct NeteaseToken {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) duration_ms: u64,
}

#[derive(Debug, Deserialize)]
struct NeteaseSearchResp {
    result: NeteaseSearchResult,
}

#[derive(Debug, Deserialize)]
struct NeteaseSearchResult {
    songs: Vec<NeteaseSong>,
}

#[derive(Debug, Deserialize)]
struct NeteaseSong {
    name: String,
    id: i64,
    duration: i64,
    artists: Vec<NeteaseArtist>,
    album: NeteaseAlbum,
}

#[derive(Debug, Deserialize)]
struct NeteaseArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct NeteaseAlbum {
    name: String,
}

#[derive(Debug, Deserialize)]
struct NeteaseLyricResp {
    lrc: Option<NeteaseLyric>,
    klyric: Option<NeteaseLyric>,
    tlyric: Option<NeteaseLyric>,
}

#[derive(Debug, Deserialize)]
struct NeteaseLyric {
    lyric: Option<String>,
}
