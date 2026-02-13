use std::time::Duration;

use reqwest::Client;

use super::error::MusicError;
use super::model::{AuthState, MusicAlbum, MusicArtist, MusicService, MusicTrack, ProviderConfig};
use super::providers::{kugou, kuwo, netease, qq};

#[derive(Debug, Clone)]
pub struct MusicClient {
    pub http: Client,
    pub cfg: ProviderConfig,
    pub timeout: Duration,
}

impl MusicClient {
    pub fn new(cfg: ProviderConfig) -> Result<Self, MusicError> {
        crate::tls::ensure_rustls_provider();
        let http = Client::builder()
            .user_agent("chaos-seed/0.3")
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            http,
            cfg,
            timeout: Duration::from_secs(15),
        })
    }

    pub fn set_config(&mut self, cfg: ProviderConfig) {
        self.cfg = cfg;
    }

    pub async fn search_tracks(
        &self,
        service: MusicService,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<MusicTrack>, MusicError> {
        match service {
            MusicService::Qq => qq::search_tracks(&self.http, keyword, page, page_size, self.timeout).await,
            MusicService::Kuwo => kuwo::search_tracks(&self.http, keyword, page, page_size, self.timeout).await,
            MusicService::Kugou => kugou::search_tracks(&self.http, &self.cfg, keyword, page, page_size, self.timeout).await,
            MusicService::Netease => netease::search_tracks(&self.http, &self.cfg, keyword, page, page_size, self.timeout).await,
        }
    }

    pub async fn search_albums(
        &self,
        service: MusicService,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<MusicAlbum>, MusicError> {
        match service {
            MusicService::Qq => qq::search_albums(&self.http, keyword, page, page_size, self.timeout).await,
            MusicService::Kuwo => kuwo::search_albums(&self.http, keyword, page, page_size, self.timeout).await,
            MusicService::Kugou => kugou::search_albums(&self.http, &self.cfg, keyword, page, page_size, self.timeout).await,
            MusicService::Netease => netease::search_albums(&self.http, &self.cfg, keyword, page, page_size, self.timeout).await,
        }
    }

    pub async fn search_artists(
        &self,
        service: MusicService,
        keyword: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<MusicArtist>, MusicError> {
        match service {
            MusicService::Qq => qq::search_artists(&self.http, keyword, page, page_size, self.timeout).await,
            MusicService::Kuwo => kuwo::search_artists(&self.http, keyword, page, page_size, self.timeout).await,
            MusicService::Kugou => kugou::search_artists(&self.http, &self.cfg, keyword, page, page_size, self.timeout).await,
            MusicService::Netease => netease::search_artists(&self.http, &self.cfg, keyword, page, page_size, self.timeout).await,
        }
    }

    pub async fn album_tracks(
        &self,
        service: MusicService,
        album_id: &str,
    ) -> Result<Vec<MusicTrack>, MusicError> {
        match service {
            MusicService::Qq => qq::album_tracks(&self.http, album_id, self.timeout).await,
            MusicService::Kuwo => kuwo::album_tracks(&self.http, album_id, self.timeout).await,
            MusicService::Kugou => kugou::album_tracks(&self.http, &self.cfg, album_id, self.timeout).await,
            MusicService::Netease => netease::album_tracks(&self.http, &self.cfg, album_id, self.timeout).await,
        }
    }

    pub async fn artist_albums(
        &self,
        service: MusicService,
        artist_id: &str,
    ) -> Result<Vec<MusicAlbum>, MusicError> {
        match service {
            MusicService::Qq => qq::artist_albums(&self.http, artist_id, self.timeout).await,
            MusicService::Kuwo => kuwo::artist_albums(&self.http, artist_id, self.timeout).await,
            MusicService::Kugou => kugou::artist_albums(&self.http, &self.cfg, artist_id, self.timeout).await,
            MusicService::Netease => netease::artist_albums(&self.http, &self.cfg, artist_id, self.timeout).await,
        }
    }

    pub async fn track_download_url(
        &self,
        service: MusicService,
        track_id: &str,
        quality_id: &str,
        auth: &AuthState,
    ) -> Result<(String, String), MusicError> {
        // returns (url, file_ext)
        match service {
            MusicService::Qq => qq::track_download_url(&self.http, track_id, quality_id, auth, self.timeout).await,
            MusicService::Kuwo => kuwo::track_download_url(&self.http, track_id, quality_id, self.timeout).await,
            MusicService::Kugou => kugou::track_download_url(&self.http, &self.cfg, track_id, quality_id, auth, self.timeout).await,
            MusicService::Netease => netease::track_download_url(&self.http, &self.cfg, track_id, quality_id, auth, self.timeout).await,
        }
    }
}

