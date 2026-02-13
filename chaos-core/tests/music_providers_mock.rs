use std::time::Duration;

use chaos_core::music::model::{AuthState, QqMusicCookie};
use chaos_core::music::providers::{kuwo, qq};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;

#[tokio::test]
async fn qq_download_url_parses_vkey_response() {
    let server = MockServer::start();
    let base_url = format!("{}/cgi-bin/musicu.fcg", server.base_url());

    server.mock(|when, then| {
        when.method(POST).path("/cgi-bin/musicu.fcg");
        then.status(200).json_body(serde_json::json!({
            "music.vkey.GetVkey.UrlGetVkey": {
                "code": 0,
                "data": {
                    "midurlinfo": [{
                        "wifiurl": "abc.flac",
                        "purl": "",
                        "ekey": "e",
                        "vkey": "v"
                    }],
                    "sip": ["https://isure.stream.qqmusic.qq.com/"]
                }
            }
        }));
    });

    let http = reqwest::Client::builder().build().unwrap();
    let auth = AuthState {
        qq: Some(QqMusicCookie {
            musicid: Some("123".to_string()),
            musickey: Some("key".to_string()),
            login_type: Some(2),
            ..Default::default()
        }),
        ..Default::default()
    };

    let (url, ext) = qq::track_download_url_with_base(
        &http,
        &base_url,
        "0039MnYb0qxYhV",
        "flac",
        &auth,
        Duration::from_secs(5),
    )
    .await
    .expect("download url");
    assert_eq!(ext, "flac");
    assert_eq!(url, "https://isure.stream.qqmusic.qq.com/abc.flac");
}

#[tokio::test]
async fn kuwo_download_url_parses_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/mobi.s")
            .query_param("rid", "123")
            .query_param("br", "320kmp3");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "url": "http://example/1.mp3",
                "format": "mp3",
                "bitrate": 320
            }
        }));
    });

    let template = format!("{}/mobi.s?rid={{rid}}&br={{br}}", server.base_url());
    let http = reqwest::Client::builder().build().unwrap();

    let (url, ext) = kuwo::track_download_url_with_template(
        &http,
        &template,
        "123",
        "mp3_320",
        Duration::from_secs(5),
    )
    .await
    .expect("download url");
    assert_eq!(ext, "mp3");
    assert_eq!(url, "http://example/1.mp3");
}
