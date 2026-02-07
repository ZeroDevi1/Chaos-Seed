use std::time::Duration;

use httpmock::Method::GET;
use httpmock::MockServer;

use chaos_seed::subtitle;

fn item_json(name: &str, score: f64) -> serde_json::Value {
    serde_json::json!({
        "gcid": "g",
        "cid": "c",
        "url": "URL_PLACEHOLDER",
        "ext": "srt",
        "name": name,
        "duration": 1,
        "languages": ["zh", "en"],
        "source": 0,
        "score": score,
        "fingerprintf_score": 0.1,
        "extra_name": "",
        "mt": 0
    })
}

#[tokio::test]
async fn search_gate_ok_returns_items() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/oracle/subtitle")
            .query_param("name", "abc");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "result": "ok",
            "data": [item_json("A", 9.0)]
        }));
    });

    let client =
        subtitle::client::ThunderClient::with_base_url(&server.base_url()).expect("client");
    let items = client
        .search("abc", Duration::from_secs(2))
        .await
        .expect("search");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "A");
}

#[tokio::test]
async fn search_gate_not_ok_returns_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/oracle/subtitle")
            .query_param("name", "abc");
        then.status(200).json_body(serde_json::json!({
            "code": 1,
            "result": "ok",
            "data": [item_json("A", 9.0)]
        }));
    });

    let client =
        subtitle::client::ThunderClient::with_base_url(&server.base_url()).expect("client");
    let err = client
        .search("abc", Duration::from_secs(2))
        .await
        .expect_err("should error");
    assert!(err.to_string().contains("api gate failed"));
}

#[tokio::test]
async fn search_items_sorts_and_limits() {
    let server = MockServer::start();

    // NOTE: We fill item urls after we know the server base URL.
    let data = vec![
        item_json("low", 1.0),
        item_json("high", 99.0),
        item_json("mid", 50.0),
    ]
    .into_iter()
    .map(|mut v| {
        v["url"] = serde_json::Value::String(format!(
            "{}/files/{}.srt",
            server.base_url(),
            v["name"].as_str().unwrap()
        ));
        v
    })
    .collect::<Vec<_>>();

    server.mock(|when, then| {
        when.method(GET)
            .path("/oracle/subtitle")
            .query_param("name", "abc");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "result": "ok",
            "data": data
        }));
    });

    let client =
        subtitle::client::ThunderClient::with_base_url(&server.base_url()).expect("client");

    let items = subtitle::core::search_items_with_client(
        &client,
        "abc",
        2,
        None,
        None,
        Duration::from_secs(2),
    )
    .await
    .expect("search");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "high");
    assert_eq!(items[1].name, "mid");
}

#[tokio::test]
async fn download_item_writes_file() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/files/a.srt");
        then.status(200).body("hello");
    });

    let client =
        subtitle::client::ThunderClient::with_base_url(&server.base_url()).expect("client");

    let item = subtitle::models::ThunderSubtitleItem {
        gcid: "g".to_string(),
        cid: "c".to_string(),
        url: format!("{}/files/a.srt", server.base_url()),
        ext: "srt".to_string(),
        name: "My Subtitle".to_string(),
        duration: 0,
        languages: vec!["zh".to_string()],
        source: 0,
        score: 1.0,
        fingerprintf_score: 0.0,
        extra_name: "".to_string(),
        mt: 0,
    };

    let dir = tempfile::tempdir().expect("tempdir");
    let out = subtitle::core::download_item_with_client(
        &client,
        &item,
        dir.path(),
        Duration::from_secs(2),
        0,
        false,
    )
    .await
    .expect("download");

    let bytes = std::fs::read(&out).expect("read");
    assert_eq!(bytes, b"hello");
}
