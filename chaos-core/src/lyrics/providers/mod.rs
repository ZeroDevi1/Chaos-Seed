mod gecimi;
mod kugou;
mod lrclib;
mod netease;
mod qq;
mod syair;

use std::time::Duration;

use reqwest::Client;

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsService};

pub use gecimi::GecimiProvider;
pub use kugou::KugouProvider;
pub use lrclib::LrcLibProvider;
pub use netease::NeteaseProvider;
pub use qq::QqMusicProvider;
pub use syair::SyairProvider;

#[derive(Debug, Clone)]
pub enum ProviderToken {
    Netease(netease::NeteaseToken),
    QQ(qq::QqToken),
    Kugou(kugou::KugouToken),
    LrcLib(lrclib::LrcLibToken),
    Gecimi(gecimi::GecimiToken),
    Syair(syair::SyairToken),
}

#[derive(Debug, Clone)]
pub enum Provider {
    Netease(NeteaseProvider),
    QQ(QqMusicProvider),
    Kugou(KugouProvider),
    LrcLib(LrcLibProvider),
    Gecimi(GecimiProvider),
    Syair(SyairProvider),
}

impl Provider {
    pub fn service(&self) -> LyricsService {
        match self {
            Self::Netease(_) => LyricsService::Netease,
            Self::QQ(_) => LyricsService::QQMusic,
            Self::Kugou(_) => LyricsService::Kugou,
            Self::LrcLib(_) => LyricsService::LrcLib,
            Self::Gecimi(_) => LyricsService::Gecimi,
            Self::Syair(_) => LyricsService::Syair,
        }
    }

    pub async fn search(
        &self,
        http: &Client,
        req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<Vec<ProviderToken>, LyricsError> {
        match self {
            Self::Netease(p) => p
                .search(http, req, timeout)
                .await
                .map(|v| v.into_iter().map(ProviderToken::Netease).collect()),
            Self::QQ(p) => p
                .search(http, req, timeout)
                .await
                .map(|v| v.into_iter().map(ProviderToken::QQ).collect()),
            Self::Kugou(p) => p
                .search(http, req, timeout)
                .await
                .map(|v| v.into_iter().map(ProviderToken::Kugou).collect()),
            Self::LrcLib(p) => p
                .search(http, req, timeout)
                .await
                .map(|v| v.into_iter().map(ProviderToken::LrcLib).collect()),
            Self::Gecimi(p) => p
                .search(http, req, timeout)
                .await
                .map(|v| v.into_iter().map(ProviderToken::Gecimi).collect()),
            Self::Syair(p) => p
                .search(http, req, timeout)
                .await
                .map(|v| v.into_iter().map(ProviderToken::Syair).collect()),
        }
    }

    pub async fn fetch(
        &self,
        http: &Client,
        token: ProviderToken,
        req: &LyricsSearchRequest,
        timeout: Duration,
    ) -> Result<LyricsSearchResult, LyricsError> {
        match (self, token) {
            (Self::Netease(p), ProviderToken::Netease(t)) => p.fetch(http, t, req, timeout).await,
            (Self::QQ(p), ProviderToken::QQ(t)) => p.fetch(http, t, req, timeout).await,
            (Self::Kugou(p), ProviderToken::Kugou(t)) => p.fetch(http, t, req, timeout).await,
            (Self::LrcLib(p), ProviderToken::LrcLib(t)) => p.fetch(http, t, req, timeout).await,
            (Self::Gecimi(p), ProviderToken::Gecimi(t)) => p.fetch(http, t, req, timeout).await,
            (Self::Syair(p), ProviderToken::Syair(t)) => p.fetch(http, t, req, timeout).await,
            _ => Err(LyricsError::Parse(
                "provider token does not match provider".to_string(),
            )),
        }
    }
}
