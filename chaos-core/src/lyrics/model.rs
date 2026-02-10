use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LyricsService {
    #[serde(rename = "netease")]
    Netease,
    #[serde(rename = "qq")]
    QQMusic,
    #[serde(rename = "kugou")]
    Kugou,
    #[serde(rename = "gecimi")]
    Gecimi,
    #[serde(rename = "syair")]
    Syair,
}

impl LyricsService {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Netease => "netease",
            Self::QQMusic => "qq",
            Self::Kugou => "kugou",
            Self::Gecimi => "gecimi",
            Self::Syair => "syair",
        }
    }
}

impl fmt::Display for LyricsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LyricsService {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s.trim().to_ascii_lowercase();
        match v.as_str() {
            "netease" | "163" | "ne" => Ok(Self::Netease),
            "qq" | "qqmusic" | "qq_music" => Ok(Self::QQMusic),
            "kugou" | "kg" => Ok(Self::Kugou),
            "gecimi" | "gc" => Ok(Self::Gecimi),
            "syair" | "sy" => Ok(Self::Syair),
            _ => Err(format!("unknown lyrics service: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LyricsSearchTerm {
    Info {
        title: String,
        artist: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        album: Option<String>,
    },
    Keyword {
        keyword: String,
    },
}

impl LyricsSearchTerm {
    pub fn description(&self) -> String {
        match self {
            Self::Keyword { keyword } => keyword.clone(),
            Self::Info { title, artist, .. } => {
                let t = title.trim();
                let a = artist.trim();
                match (t.is_empty(), a.is_empty()) {
                    (false, false) => format!("{t} {a}"),
                    (false, true) => t.to_string(),
                    (true, false) => a.to_string(),
                    (true, true) => String::new(),
                }
            }
        }
    }

    pub fn title_artist(&self) -> (Option<&str>, Option<&str>) {
        match self {
            Self::Info { title, artist, .. } => (Some(title.as_str()), Some(artist.as_str())),
            Self::Keyword { .. } => (None, None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsSearchRequest {
    pub term: LyricsSearchTerm,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub limit: usize,
}

impl LyricsSearchRequest {
    pub fn new(term: LyricsSearchTerm) -> Self {
        Self {
            term,
            duration_ms: None,
            limit: 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsSearchOptions {
    pub timeout_ms: u64,
    pub strict_match: bool,
    pub services: Vec<LyricsService>,
}

impl Default for LyricsSearchOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 10_000,
            strict_match: false,
            services: vec![
                LyricsService::Netease,
                LyricsService::QQMusic,
                LyricsService::Kugou,
                LyricsService::Gecimi,
                LyricsService::Syair,
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsSearchResult {
    pub service: LyricsService,
    pub service_token: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    pub quality: f64,
    pub matched: bool,
    pub has_translation: bool,
    pub has_inline_timetags: bool,

    pub lyrics_original: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics_translation: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<serde_json::Value>,
}

