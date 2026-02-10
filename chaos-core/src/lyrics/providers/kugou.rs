use std::io::Read;
use std::time::Duration;

use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsService};

#[derive(Debug, Clone)]
pub struct KugouProvider {
    search_url: String,
    lyric_url: String,
}

impl Default for KugouProvider {
    fn default() -> Self {
        Self {
            search_url: "http://lyrics.kugou.com/search".to_string(),
            lyric_url: "http://lyrics.kugou.com/download".to_string(),
        }
    }
}

impl KugouProvider {
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
    ) -> Result<Vec<KugouToken>, LyricsError> {
        let term = req.term.description();
        if term.trim().is_empty() {
            return Ok(vec![]);
        }

        let duration_ms = req.duration_ms.unwrap_or(0).to_string();
        let params: Vec<(&str, String)> = vec![
            ("keyword", term),
            ("duration", duration_ms),
            ("client", "pc".to_string()),
            ("ver", "1".to_string()),
            ("man", "yes".to_string()),
        ];
        let resp = http
            .get(&self.search_url)
            .query(&params)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.bytes().await?;
        let data: KugouSearchResp = serde_json::from_slice(&body)?;

        Ok(data
            .candidates
            .into_iter()
            .map(|it| KugouToken {
                id: it.id,
                accesskey: it.accesskey,
                title: it.song,
                artist: it.singer,
                duration_ms: it.duration.max(0) as u64,
            })
            .collect())
    }

    pub async fn fetch(
        &self,
        http: &Client,
        token: KugouToken,
        _req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<LyricsSearchResult, LyricsError> {
        let resp = http
            .get(&self.lyric_url)
            .query(&[
                ("id", token.id.as_str()),
                ("accesskey", token.accesskey.as_str()),
                ("fmt", "krc"),
                ("charset", "utf8"),
                ("client", "pc"),
                ("ver", "1"),
            ])
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.bytes().await?;
        let data: KugouLyricResp = serde_json::from_slice(&body)?;

        let bytes = decode_b64(&data.content)
            .map_err(|e| LyricsError::Parse(format!("kugou: base64 content decode: {e}")))?;

        let lyric_text = match data.fmt.as_str() {
            "krc" => decrypt_kugou_krc(&bytes)
                .ok_or_else(|| LyricsError::Parse("kugou: failed to decrypt krc".to_string()))?,
            "lrc" => String::from_utf8(bytes)
                .map_err(|e| LyricsError::Parse(format!("kugou: lrc utf8: {e}")))?,
            _ => {
                return Err(LyricsError::Parse(format!(
                    "kugou: unknown fmt {}",
                    data.fmt
                )));
            }
        };

        if lyric_text.trim().is_empty() {
            return Err(LyricsError::Parse("kugou: empty lyric content".to_string()));
        }

        Ok(LyricsSearchResult {
            service: LyricsService::Kugou,
            service_token: format!("{},{}", token.id, token.accesskey),
            title: Some(token.title),
            artist: Some(token.artist),
            album: None,
            duration_ms: Some(token.duration_ms),
            quality: 0.0,
            matched: false,
            has_translation: false,
            has_inline_timetags: data.fmt == "krc",
            lyrics_original: lyric_text,
            lyrics_translation: None,
            debug: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct KugouToken {
    pub(crate) id: String,
    pub(crate) accesskey: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) duration_ms: u64,
}

#[derive(Debug, Deserialize)]
struct KugouSearchResp {
    candidates: Vec<KugouSearchItem>,
}

#[derive(Debug, Deserialize)]
struct KugouSearchItem {
    id: String,
    accesskey: String,
    song: String,
    singer: String,
    duration: i64,
}

#[derive(Debug, Deserialize)]
struct KugouLyricResp {
    content: String,
    fmt: String,
}

fn decode_b64(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(s.trim())
}

fn decrypt_kugou_krc(data: &[u8]) -> Option<String> {
    const FLAG: &[u8] = b"krc1";
    const KEY: [u8; 16] = [64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105];
    if !data.starts_with(FLAG) {
        return None;
    }
    let mut xored = Vec::with_capacity(data.len().saturating_sub(4));
    for (i, b) in data[4..].iter().enumerate() {
        xored.push(b ^ KEY[i & 0x0f]);
    }
    let mut gz = GzDecoder::new(&xored[..]);
    let mut out = String::new();
    gz.read_to_string(&mut out).ok()?;
    Some(out)
}
