use std::time::Duration;

use chaos_core::music::error::MusicError;
use chaos_core::music::model::{AuthState, ProviderConfig};
use chaos_core::music::providers::kugou;

#[tokio::test]
async fn kugou_track_download_url_reports_invalid_input_without_requiring_base_url() {
    let http = reqwest::Client::new();
    let cfg = ProviderConfig::default(); // kugouBaseUrl is ignored
    let auth = AuthState::default();
    let err = kugou::track_download_url(&http, &cfg, "", "mp3_320", &auth, Duration::from_secs(1))
        .await
        .expect_err("should fail");
    assert!(matches!(err, MusicError::InvalidInput(_)));
}

#[tokio::test]
async fn kugou_artist_albums_reports_invalid_input_without_requiring_base_url() {
    let http = reqwest::Client::new();
    let cfg = ProviderConfig::default();
    let err = kugou::artist_albums(&http, &cfg, "", Duration::from_secs(1))
        .await
        .expect_err("should fail");
    assert!(matches!(err, MusicError::InvalidInput(_)));
}

#[tokio::test]
async fn kugou_album_tracks_reports_invalid_input_without_requiring_base_url() {
    let http = reqwest::Client::new();
    let cfg = ProviderConfig::default();
    let err = kugou::album_tracks(&http, &cfg, "", Duration::from_secs(1))
        .await
        .expect_err("should fail");
    assert!(matches!(err, MusicError::InvalidInput(_)));
}

