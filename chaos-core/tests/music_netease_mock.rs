use std::time::Duration;

use chaos_core::music::model::{AuthState, ProviderConfig};
use chaos_core::music::providers::netease;
use httpmock::Method::POST;
use httpmock::MockServer;

#[tokio::test]
async fn track_download_url_falls_back_when_song_download_url_is_empty() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(POST).path("/song/download/url");
        then.status(200).json_body(serde_json::json!({
            "code": 200,
            "data": { "url": "" }
        }));
    });

    server.mock(|when, then| {
        when.method(POST).path("/song/url/v1");
        then.status(200).json_body(serde_json::json!({
            "code": 200,
            "data": [{ "url": "http://example.com/audio.flac", "type": "flac" }]
        }));
    });

    let http = reqwest::Client::new();
    let cfg = ProviderConfig {
        netease_base_urls: vec![base],
        ..ProviderConfig::default()
    };
    let auth = AuthState::default();

    let (url, ext) = netease::track_download_url(&http, &cfg, "1", "flac", &auth, Duration::from_secs(2))
        .await
        .expect("download url");
    assert_eq!(url, "http://example.com/audio.flac");
    assert_eq!(ext, "flac");
}

