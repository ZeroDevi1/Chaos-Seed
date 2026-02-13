use std::time::Duration;

use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::OnceLock;

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsService};

#[derive(Debug, Clone)]
pub struct LrcLibProvider {
    search_url: String,
}

impl Default for LrcLibProvider {
    fn default() -> Self {
        Self {
            search_url: "https://lrclib.net/api/search".to_string(),
        }
    }
}

impl LrcLibProvider {
    pub fn with_base_url(search_url: &str) -> Self {
        Self {
            search_url: search_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn search(
        &self,
        http: &Client,
        req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<Vec<LrcLibToken>, LyricsError> {
        let (title, artist, album) = match &req.term {
            crate::lyrics::model::LyricsSearchTerm::Info {
                title,
                artist,
                album,
            } => (
                title.trim(),
                artist.trim(),
                album.as_deref().unwrap_or("").trim(),
            ),
            crate::lyrics::model::LyricsSearchTerm::Keyword { keyword } => (keyword.trim(), "", ""),
        };

        if title.is_empty() {
            return Ok(vec![]);
        }

        // BetterLyrics query shape: track_name/artist_name/album_name/durationMs.
        // LRCLIB accepts empty artist/album; durationMs is optional but helps precision.
        let mut q: Vec<(&str, String)> = Vec::new();
        q.push(("track_name", title.to_string()));
        q.push(("artist_name", artist.to_string()));
        q.push(("album_name", album.to_string()));
        if let Some(d) = req.duration_ms.filter(|v| *v > 0) {
            q.push(("durationMs", d.to_string()));
        }

        let resp = http
            .get(&self.search_url)
            .query(&q)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;

        let body = resp.bytes().await?;
        let arr: Vec<LrcLibSearchItem> = serde_json::from_slice(&body)?;

        let mut out = Vec::new();
        for (idx, it) in arr.into_iter().enumerate() {
            if idx >= 20 {
                break;
            }
            let synced = it.synced_lyrics.unwrap_or_default();
            if synced.trim().is_empty() {
                continue;
            }
            out.push(LrcLibToken {
                id: it
                    .id
                    .unwrap_or_else(|| format!("lrclib:{idx}:{}", fastrand::u64(..))),
                track_name: it.track_name.unwrap_or_default(),
                artist_name: it.artist_name.unwrap_or_default(),
                album_name: it.album_name.unwrap_or_default(),
                duration_ms: it.duration.map(|s| (s * 1000.0).round() as u64),
                synced_lyrics: synced,
            });
        }

        Ok(out)
    }

    pub async fn fetch(
        &self,
        _http: &Client,
        token: LrcLibToken,
        _req: &LyricsSearchRequest,
        _timeout: Duration,
    ) -> Result<LyricsSearchResult, LyricsError> {
        let has_inline_timetags = looks_like_lrc(&token.synced_lyrics);
        Ok(LyricsSearchResult {
            service: LyricsService::LrcLib,
            service_token: token.id.clone(),
            title: (!token.track_name.trim().is_empty()).then_some(token.track_name),
            artist: (!token.artist_name.trim().is_empty()).then_some(token.artist_name),
            album: (!token.album_name.trim().is_empty()).then_some(token.album_name),
            duration_ms: token.duration_ms,
            match_percentage: 0,
            quality: 0.0,
            matched: false,
            has_translation: false,
            has_inline_timetags,
            lyrics_original: token.synced_lyrics,
            lyrics_translation: None,
            debug: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LrcLibToken {
    pub(crate) id: String,
    pub(crate) track_name: String,
    pub(crate) artist_name: String,
    pub(crate) album_name: String,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) synced_lyrics: String,
}

#[derive(Debug, Deserialize)]
struct LrcLibSearchItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "trackName")]
    #[serde(default)]
    track_name: Option<String>,
    #[serde(rename = "artistName")]
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(rename = "albumName")]
    #[serde(default)]
    album_name: Option<String>,
    /// seconds (per LRCLIB API)
    #[serde(default)]
    duration: Option<f64>,
    #[serde(rename = "syncedLyrics")]
    #[serde(default)]
    synced_lyrics: Option<String>,
}

fn looks_like_lrc(s: &str) -> bool {
    // Quick heuristic to drive UI/quality bonuses.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"\[\d{1,2}:\d{2}(?:\.\d{1,3})?\]").expect("lrc tag regex"));
    re.is_match(s)
}
