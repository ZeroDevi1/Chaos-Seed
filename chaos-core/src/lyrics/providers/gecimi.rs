use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsService};
use crate::lyrics::util;

#[derive(Debug, Clone)]
pub struct GecimiProvider {
    base_url: String,
}

impl Default for GecimiProvider {
    fn default() -> Self {
        Self {
            base_url: "http://gecimi.com/api/lyric".to_string(),
        }
    }
}

impl GecimiProvider {
    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn search(
        &self,
        http: &Client,
        req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<Vec<GecimiToken>, LyricsError> {
        let (title, artist) = req.term.title_artist();
        let (Some(title), Some(artist)) = (title, artist) else {
            return Err(LyricsError::UnsupportedTerm);
        };
        let title = util::percent_encode_component(title);
        let artist = util::percent_encode_component(artist);
        let url = format!("{}/{}/{}", self.base_url, title, artist);

        let resp = http.get(url).timeout(timeout).send().await?.error_for_status()?;
        let body = resp.bytes().await?;
        let data: GecimiSearchResp = serde_json::from_slice(&body)?;
        Ok(data
            .result
            .into_iter()
            .map(|r| GecimiToken {
                aid: r.aid,
                lrc_url: r.lrc,
            })
            .collect())
    }

    pub async fn fetch(
        &self,
        http: &Client,
        token: GecimiToken,
        _req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<LyricsSearchResult, LyricsError> {
        let resp = http
            .get(&token.lrc_url)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.bytes().await?;
        let s = String::from_utf8_lossy(&body).to_string();
        if s.trim().is_empty() {
            return Err(LyricsError::Parse("gecimi: empty lyric content".to_string()));
        }

        Ok(LyricsSearchResult {
            service: LyricsService::Gecimi,
            service_token: format!("{},{}", token.aid, token.lrc_url),
            title: None,
            artist: None,
            album: None,
            duration_ms: None,
            match_percentage: 0,
            quality: 0.0,
            matched: false,
            has_translation: false,
            has_inline_timetags: false,
            lyrics_original: s,
            lyrics_translation: None,
            debug: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GecimiToken {
    pub(crate) aid: i64,
    pub(crate) lrc_url: String,
}

#[derive(Debug, Deserialize)]
struct GecimiSearchResp {
    result: Vec<GecimiItem>,
}

#[derive(Debug, Deserialize)]
struct GecimiItem {
    lrc: String,
    aid: i64,
}
