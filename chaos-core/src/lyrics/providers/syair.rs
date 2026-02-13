use std::time::Duration;

use regex::Regex;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{
    LyricsSearchRequest, LyricsSearchResult, LyricsSearchTerm, LyricsService,
};
use crate::lyrics::util;

#[derive(Debug, Clone)]
pub struct SyairProvider {
    search_base_url: String,
    base_url: String,
}

impl Default for SyairProvider {
    fn default() -> Self {
        Self {
            search_base_url: "https://syair.info/search".to_string(),
            base_url: "https://syair.info".to_string(),
        }
    }
}

impl SyairProvider {
    pub fn with_base_url(search_base_url: &str, base_url: &str) -> Self {
        Self {
            search_base_url: search_base_url.to_string(),
            base_url: base_url.to_string(),
        }
    }

    pub async fn search(
        &self,
        http: &Client,
        req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<Vec<SyairToken>, LyricsError> {
        let mut params: Vec<(&str, String)> = vec![("page", "1".to_string())];
        match &req.term {
            LyricsSearchTerm::Info { title, artist, .. } => {
                params.push(("artist", artist.clone()));
                params.push(("title", title.clone()));
            }
            LyricsSearchTerm::Keyword { keyword } => {
                params.push(("q", keyword.clone()));
            }
        }

        let resp = http
            .get(&self.search_base_url)
            .query(&params)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.bytes().await?;
        let html = String::from_utf8_lossy(&body);

        let re = Regex::new(r#"<div class="title"><a href="([^"]+)">"#)
            .map_err(|e| LyricsError::Parse(format!("syair: regex: {e}")))?;
        let mut out = Vec::new();
        for cap in re.captures_iter(&html) {
            if let Some(m) = cap.get(1) {
                out.push(SyairToken {
                    link: m.as_str().to_string(),
                });
            }
        }
        Ok(out)
    }

    pub async fn fetch(
        &self,
        http: &Client,
        token: SyairToken,
        _req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<LyricsSearchResult, LyricsError> {
        let url = if token.link.starts_with("http://") || token.link.starts_with("https://") {
            token.link.clone()
        } else {
            format!("{}{}", self.base_url.trim_end_matches('/'), token.link)
        };

        let mut headers = HeaderMap::new();
        headers.insert("Referer", HeaderValue::from_static("https://syair.info/"));
        let resp = http
            .get(url)
            .headers(headers)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.bytes().await?;
        let html = String::from_utf8_lossy(&body).to_string();

        let re = Regex::new(r#"<div class="entry">(.+?)<div"#)
            .map_err(|e| LyricsError::Parse(format!("syair: regex: {e}")))?;
        let lrc_html = re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| LyricsError::Parse("syair: missing lyric content".to_string()))?;

        let text = util::html_to_text(&lrc_html);
        if text.trim().is_empty() {
            return Err(LyricsError::Parse("syair: empty lyric content".to_string()));
        }

        Ok(LyricsSearchResult {
            service: LyricsService::Syair,
            service_token: token.link,
            title: None,
            artist: None,
            album: None,
            duration_ms: None,
            match_percentage: 0,
            quality: 0.0,
            matched: false,
            has_translation: false,
            has_inline_timetags: false,
            lyrics_original: text,
            lyrics_translation: None,
            debug: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SyairToken {
    pub(crate) link: String,
}
