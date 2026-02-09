use chaos_core::danmaku::model::Site;
use chaos_core::livestream::client::{Endpoints, EnvConfig, LivestreamClient, LivestreamConfig};
use chaos_core::livestream::model::ResolveOptions;
use httpmock::Method::{GET, POST};
use httpmock::MockServer;

fn fixed_env() -> EnvConfig {
    EnvConfig {
        now_ms: std::sync::Arc::new(|| 1_700_000_000_000),
        now_s: std::sync::Arc::new(|| 1_700_000_000),
        rng: std::sync::Arc::new(std::sync::Mutex::new(fastrand::Rng::with_seed(2))),
    }
}

#[tokio::test]
async fn decode_manifest_parses_room_id_and_lists_multirates() {
    let server = MockServer::start();
    let base = server.base_url();
    let host = base
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    // Room HTML that contains escaped JSON with \"roomInfo\" marker.
    let html =
        format!(r#"xxx \"roomInfo\" : {{\"room\":{{\"room_id\":9999,\"show_status\":1}}}} yyy"#);
    server.mock(|when, then| {
        when.method(GET).path("/abc");
        then.status(200).body(html);
    });

    server.mock(|when, then| {
        when.method(GET).path("/betard/9999");
        then.status(200).json_body(serde_json::json!({
            "room": {
                "room_name": "douyu-title",
                "nickname": "n",
                "avatar": { "big": "a" },
                "show_status": 1,
                "room_pic": "p"
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/wgapi/livenc/liveweb/websec/getEncryption");
        then.status(200).json_body(serde_json::json!({
            "error": 0,
            "data": {
                "key": "k",
                "rand_str": "r",
                "enc_time": 0,
                "expire_at": 0,
                "enc_data": "ENC",
                "is_special": 0
            }
        }));
    });

    // H5 play: current rate=1
    server.mock(|when, then| {
        when.method(POST).path("/lapi/live/getH5PlayV1/9999");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "room_id": 9999,
                "rtmp_url": "http://play",
                "rtmp_live": "live.flv",
                "rate": 1,
                "multirates": [
                    {"name":"高清","rate":1,"highBit":0,"bit":1000},
                    {"name":"原画","rate":2,"highBit":0,"bit":2000}
                ],
                "p2pMeta": {
                    "xp2p_domain": host,
                    "xp2p_txDelay": 1,
                    "xp2p_txSecret": "s",
                    "xp2p_txTime": "t"
                }
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/live.xs");
        then.status(200).json_body(serde_json::json!({
            "sug": ["p2p1"],
            "bak": ["p2p2"]
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            douyu_base: base.clone(),
            douyu_cdn_scheme: "http".to_string(),
            douyu_p2p_scheme: "http".to_string(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let man = client
        .decode_manifest("https://www.douyu.com/abc", ResolveOptions::default())
        .await
        .expect("manifest");

    assert_eq!(man.site, Site::Douyu);
    assert_eq!(man.room_id, "9999");
    assert_eq!(man.info.title, "douyu-title");
    assert_eq!(man.variants.len(), 2);
    let current = man
        .variants
        .iter()
        .find(|v| v.rate == Some(1))
        .expect("rate=1");
    assert!(current.url.is_some());
    assert!(current.backup_urls.len() >= 1);
    let other = man
        .variants
        .iter()
        .find(|v| v.rate == Some(2))
        .expect("rate=2");
    assert!(other.url.is_none());
}

#[tokio::test]
async fn resolve_variant_fetches_specific_rate() {
    let server = MockServer::start();
    let base = server.base_url();
    let host = base
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    // Minimal set: encryption + rate-specific h5 play.
    server.mock(|when, then| {
        when.method(GET)
            .path("/wgapi/livenc/liveweb/websec/getEncryption");
        then.status(200).json_body(serde_json::json!({
            "error": 0,
            "data": {
                "key": "k",
                "rand_str": "r",
                "enc_time": 0,
                "expire_at": 0,
                "enc_data": "ENC",
                "is_special": 0
            }
        }));
    });

    server.mock(|when, then| {
        when.method(POST).path("/lapi/live/getH5PlayV1/9999");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "room_id": 9999,
                "rtmp_url": "http://play",
                "rtmp_live": "live2.flv",
                "rate": 2,
                "multirates": [
                    {"name":"原画","rate":2,"highBit":0,"bit":2000}
                ],
                "p2pMeta": {
                    "xp2p_domain": host,
                    "xp2p_txDelay": 1,
                    "xp2p_txSecret": "s",
                    "xp2p_txTime": "t"
                }
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/live2.xs");
        then.status(200).json_body(serde_json::json!({
            "sug": [],
            "bak": []
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            douyu_base: base.clone(),
            douyu_cdn_scheme: "http".to_string(),
            douyu_p2p_scheme: "http".to_string(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let v = client
        .resolve_variant(Site::Douyu, "9999", "douyu:2:原画")
        .await
        .expect("variant");
    assert_eq!(v.rate, Some(2));
    assert!(v.url.unwrap().contains("live2.flv"));
}
