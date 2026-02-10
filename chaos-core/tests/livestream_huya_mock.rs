use chaos_core::danmaku::model::Site;
use chaos_core::livestream::client::{Endpoints, EnvConfig, LivestreamClient, LivestreamConfig};
use chaos_core::livestream::model::ResolveOptions;
use httpmock::Method::GET;
use httpmock::MockServer;

fn fixed_env() -> EnvConfig {
    EnvConfig {
        now_ms: std::sync::Arc::new(|| 1_740_031_240_996), // matches IINA+'s example-ish scale
        now_s: std::sync::Arc::new(|| 1_740_031_240),
        rng: std::sync::Arc::new(std::sync::Mutex::new(fastrand::Rng::with_seed(3))),
    }
}

#[tokio::test]
async fn decode_manifest_mp_profile_room_ok() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET).path("/cache.php").query_param("m", "Live");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "liveData": {
                    "roomName": "r",
                    "introduction": "",
                    "nick": "n",
                    "avatar180": "a",
                    "screenshot": "c",
                    "bitRateInfo": "[{\"sDisplayName\":\"原画\",\"iBitRate\":0},{\"sDisplayName\":\"蓝光\",\"iBitRate\":2000}]",
                    "profileRoom": 123
                },
                "liveStatus": "ON",
                "stream": {
                    "baseSteamInfoList": [{
                        "sStreamName": "s",
                        "lPresenterUid": 777,
                        "sFlvUrl": "http://tx.flv.huya.com/huyalive",
                        "sFlvUrlSuffix": "flv",
                        "sFlvAntiCode": "wsTime=67b6c60d&ctype=tars_mp&t=102&fs=1&fm=RFdxOEJjSjNoNkRKdDZUWV8kMF8kMV8kMl8kMw%3D%3D"
                    }]
                }
            }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            huya_mp_base: base.clone(),
            huya_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let man = client
        .decode_manifest("https://www.huya.com/123", ResolveOptions::default())
        .await
        .expect("manifest");

    assert_eq!(man.site, Site::Huya);
    assert_eq!(man.room_id, "123");
    assert_eq!(man.info.title, "r");
    assert_eq!(man.variants.len(), 2);
    for v in &man.variants {
        let url = v.url.as_ref().expect("url");
        assert!(url.contains("wsSecret="));
        assert!(url.contains("wsTime=67b6c60d"));
        assert!(url.contains("seqid="));
        assert!(url.contains("ver=1"));
        assert!(url.contains("u="));
        if v.quality == 2000 {
            assert!(url.contains("ratio=2000"));
        }
    }
}

#[tokio::test]
async fn decode_manifest_resolves_non_numeric_room_to_rid_via_player_config() {
    let server = MockServer::start();
    let base = server.base_url();

    // Page HTML with `var hyPlayerConfig = ... stream: { ... profileRoom: 456 ... }`.
    let html = r#"
        <html><body>
        <script>
        var hyPlayerConfig = stream: { "data": [ { "gameLiveInfo": { "profileRoom": 456 } } ] } window.TT_LIVE_TIMING
        </script>
        </body></html>
    "#;
    server.mock(|when, then| {
        when.method(GET).path("/abc");
        then.status(200).body(html);
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/cache.php")
            .query_param("roomid", "456");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "liveData": {
                    "roomName": "r2",
                    "introduction": "",
                    "nick": "n2",
                    "avatar180": "a2",
                    "screenshot": "c2",
                    "bitRateInfo": "[]",
                    "profileRoom": 456
                },
                "liveStatus": "OFF",
                "stream": { "baseSteamInfoList": [] }
            }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            huya_mp_base: base.clone(),
            huya_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let man = client
        .decode_manifest("https://www.huya.com/abc", ResolveOptions::default())
        .await
        .expect("manifest");
    assert_eq!(man.room_id, "456");
    assert_eq!(man.info.title, "r2");
}
