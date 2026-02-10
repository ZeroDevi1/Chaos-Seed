use std::io::Write;
use std::time::Duration;

use chaos_core::lyrics::model::{LyricsSearchRequest, LyricsSearchTerm};
use chaos_core::lyrics::providers::{KugouProvider, LrcLibProvider, NeteaseProvider, QqMusicProvider};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;

fn req_info(title: &str, artist: &str, duration_ms: Option<u64>) -> LyricsSearchRequest {
    let mut r = LyricsSearchRequest::new(LyricsSearchTerm::Info {
        title: title.to_string(),
        artist: artist.to_string(),
        album: None,
    });
    r.duration_ms = duration_ms;
    r.limit = 3;
    r
}

#[tokio::test]
async fn qq_search_and_fetch_parses_jsonp_and_base64() {
    use base64::Engine as _;
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET)
            .path("/search")
            .query_param("w", "Hello Adele");
        then.status(200).body(
            r#"callback({"data":{"song":{"list":[{"songmid":"m1","songname":"Hello","albumname":"Hello","singer":[{"name":"Adele"}],"interval":296}]}}})"#,
        );
    });

    let lyric_b64 = base64::engine::general_purpose::STANDARD.encode("a&amp;b");
    let trans_b64 = base64::engine::general_purpose::STANDARD.encode("c&lt;d");
    server.mock(|when, then| {
        when.method(GET)
            .path("/lyric")
            .query_param("songmid", "m1")
            .query_param("g_tk", "5381");
        then.status(200).body(format!(
            "MusicJsonCallback({{\"lyric\":\"{lyric_b64}\",\"trans\":\"{trans_b64}\"}})"
        ));
    });

    let p = QqMusicProvider::with_base_url(&format!("{}/search", server.base_url()), &format!("{}/lyric", server.base_url()));
    let http = reqwest::Client::new();
    let req = req_info("Hello", "Adele", Some(296_000));

    let toks = p.search(&http, &req, Duration::from_millis(1000)).await.unwrap();
    assert_eq!(toks.len(), 1);

    let item = p.fetch(&http, toks[0].clone(), &req, Duration::from_millis(1000)).await.unwrap();
    assert_eq!(item.title.as_deref(), Some("Hello"));
    assert_eq!(item.artist.as_deref(), Some("Adele"));
    assert_eq!(item.lyrics_original.trim(), "a&b");
    assert_eq!(item.lyrics_translation.as_deref().unwrap().trim(), "c<d");
}

#[tokio::test]
async fn kugou_fetch_decrypts_krc() {
    use base64::Engine as _;
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/search");
        then.status(200).body(
            r#"{"candidates":[{"id":"1","accesskey":"k","song":"Hello","singer":"Adele","duration":296000}]}"#,
        );
    });

    let plaintext = "[00:01.00]hello";
    let gz = gzip_bytes(plaintext.as_bytes());
    let krc = build_krc_payload(&gz);
    let content_b64 = base64::engine::general_purpose::STANDARD.encode(krc);
    server.mock(|when, then| {
        when.method(GET).path("/download");
        then.status(200).body(format!(r#"{{"content":"{content_b64}","fmt":"krc"}}"#));
    });

    let p = KugouProvider::with_base_url(&format!("{}/search", server.base_url()), &format!("{}/download", server.base_url()));
    let http = reqwest::Client::new();
    let req = req_info("Hello", "Adele", Some(296_000));

    let toks = p.search(&http, &req, Duration::from_millis(1000)).await.unwrap();
    assert_eq!(toks.len(), 1);
    let item = p.fetch(&http, toks[0].clone(), &req, Duration::from_millis(1000)).await.unwrap();
    assert!(item.lyrics_original.contains("hello"));
    assert!(item.has_inline_timetags);
}

#[tokio::test]
async fn netease_search_uses_cookie_roundtrip_and_fetches_lyrics() {
    let server = MockServer::start();

    // Return Set-Cookie and data; the provider may repeat the request with the cookie.
    server.mock(|when, then| {
        when.method(POST).path("/api/search/pc");
        then.status(200)
            .header("Set-Cookie", "os=pc; Path=/;")
            .body(r#"{"result":{"songs":[{"name":"Hello","id":1,"duration":296000,"artists":[{"name":"Adele"}],"album":{"name":"Hello","id":1}}]}}"#);
    });

    server.mock(|when, then| {
        when.method(GET).path("/api/song/lyric").query_param("id", "1");
        then.status(200).body(
            r#"{"lrc":{"lyric":"[00:01:00]hello"},"klyric":null,"tlyric":{"lyric":"[00:01:00]trans"}}"#,
        );
    });

    let p = NeteaseProvider::with_base_url(
        &format!("{}/api/search/pc", server.base_url()),
        &format!("{}/api/song/lyric", server.base_url()),
    );
    let http = reqwest::Client::new();
    let req = req_info("Hello", "Adele", Some(296_000));

    let toks = p.search(&http, &req, Duration::from_millis(1000)).await.unwrap();
    assert_eq!(toks.len(), 1);
    let item = p.fetch(&http, toks[0].clone(), &req, Duration::from_millis(1000)).await.unwrap();
    assert!(item.lyrics_original.contains("[00:01.00]"));
    assert!(item.lyrics_translation.as_deref().unwrap().contains("[00:01.00]"));
}

#[tokio::test]
async fn lrclib_search_returns_synced_lyrics() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/api/search");
        then.status(200).body(
            r#"[{"id":"x1","trackName":"Hello","artistName":"Adele","albumName":"Hello","duration":296.0,"syncedLyrics":"[00:01.00]hello\n"}]"#,
        );
    });

    let p = LrcLibProvider::with_base_url(&format!("{}/api/search", server.base_url()));
    let http = reqwest::Client::new();
    let req = req_info("Hello", "Adele", Some(296_000));

    let toks = p.search(&http, &req, Duration::from_millis(1000)).await.unwrap();
    assert_eq!(toks.len(), 1);
    let item = p.fetch(&http, toks[0].clone(), &req, Duration::from_millis(1000)).await.unwrap();
    assert_eq!(item.service.to_string(), "lrclib");
    assert!(item.lyrics_original.contains("[00:01.00]hello"));
}

fn gzip_bytes(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

fn build_krc_payload(gz: &[u8]) -> Vec<u8> {
    const KEY: [u8; 16] = [64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105];
    let mut out = Vec::with_capacity(4 + gz.len());
    out.extend_from_slice(b"krc1");
    for (i, b) in gz.iter().enumerate() {
        out.push(b ^ KEY[i & 0x0f]);
    }
    out
}
