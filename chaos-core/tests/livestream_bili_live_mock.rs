use chaos_core::danmaku::model::Site;
use chaos_core::livestream::client::{Endpoints, EnvConfig, LivestreamClient, LivestreamConfig};
use chaos_core::livestream::model::ResolveOptions;
use httpmock::Method::GET;
use httpmock::MockServer;

fn fixed_env() -> EnvConfig {
    EnvConfig {
        now_ms: std::sync::Arc::new(|| 1_700_000_000_000),
        now_s: std::sync::Arc::new(|| 1_700_000_000),
        rng: std::sync::Arc::new(std::sync::Mutex::new(fastrand::Rng::with_seed(1))),
    }
}

#[tokio::test]
async fn decode_manifest_room_play_info_ok() {
    let server = MockServer::start();
    let base = server.base_url();

    // get_info: short -> long
    server.mock(|when, then| {
        when.method(GET)
            .path("/room/v1/Room/get_info")
            .query_param("room_id", "1");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "room_id": 999,
                "title": "t",
                "live_status": 1,
                "user_cover": "c"
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/live_user/v1/UserInfo/get_anchor_in_room")
            .query_param("roomid", "999");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "info": { "uname": "u", "face": "f" } }
        }));
    });

    let bad_qn0 = server.mock(|when, then| {
        // Guard: quality enumeration should NOT send qn=0 (align with dart_simple_live).
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo")
            .query_param("qn", "0");
        then.status(500).body("unexpected qn=0");
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "encrypted": false,
                "pwd_verified": true,
                "playurl_info": {
                    "playurl": {
                        "g_qn_desc": [
                            {"qn": 1000, "desc": "高清"},
                            {"qn": 2000, "desc": "原画"}
                        ],
                        "stream": [{
                            "protocol_name": "http_stream",
                            "format": [{
                                "format_name": "flv",
                                "codec": [{
                                    "codec_name": "avc",
                                    "current_qn": 2000,
                                    "accept_qn": [1000,2000],
                                    "base_url": "/live-bvc/xx.flv",
                                    "url_info": [
                                        {"host": "https://foo.mcdn.bilivideo.cn", "extra": "?x=1", "stream_ttl": 1},
                                        {"host": "https://up-mirror.bilivideo.com", "extra": "?x=1", "stream_ttl": 1}
                                    ]
                                }]
                            }]
                        }]
                    }
                }
            }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            bili_api_base: base.clone(),
            bili_live_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let man = client
        .decode_manifest("https://live.bilibili.com/1", ResolveOptions::default())
        .await
        .expect("manifest");

    assert_eq!(bad_qn0.hits(), 0, "should not request getRoomPlayInfo with qn=0");
    assert_eq!(man.site, Site::BiliLive);
    assert_eq!(man.room_id, "999");
    assert_eq!(man.info.title, "t");
    assert_eq!(man.info.name.as_deref(), Some("u"));
    assert!(
        man.variants
            .iter()
            .any(|v| v.quality == 2000 && v.url.is_some())
    );
    // Mirror first after MBGA sorting.
    let v = man
        .variants
        .iter()
        .find(|v| v.quality == 2000)
        .expect("variant");
    assert!(
        v.url
            .as_ref()
            .unwrap()
            .starts_with("https://up-mirror.bilivideo.com")
    );
}

#[tokio::test]
async fn decode_manifest_resolves_highest_quality_to_avoid_single_variant() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET)
            .path("/room/v1/Room/get_info")
            .query_param("room_id", "10");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "room_id": 1010, "title": "t10", "live_status": 1, "user_cover": "" }
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/live_user/v1/UserInfo/get_anchor_in_room")
            .query_param("roomid", "1010");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "info": { "uname": "u10", "face": "f10" } }
        }));
    });

    let bad_qn0 = server.mock(|when, then| {
        // Guard: quality enumeration should NOT send qn=0 (align with dart_simple_live).
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo")
            .query_param("qn", "0");
        then.status(500).body("unexpected qn=0");
    });

    // Second call (qn=2000): server honors highest qn and returns a URL for it.
    server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo")
            .query_param("qn", "2000");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "encrypted": false,
                "pwd_verified": true,
                "playurl_info": {
                    "playurl": {
                        "g_qn_desc": [
                            {"qn": 1000, "desc": "高清"},
                            {"qn": 2000, "desc": "原画"}
                        ],
                        "stream": [{
                            "protocol_name": "http_stream",
                            "format": [{
                                "format_name": "flv",
                                "codec": [{
                                    "codec_name": "avc",
                                    "current_qn": 2000,
                                    "accept_qn": [1000,2000],
                                    "base_url": "/live-bvc/high.flv",
                                    "url_info": [
                                        {"host": "https://up-mirror.bilivideo.com", "extra": "?y=1"}
                                    ]
                                }]
                            }]
                        }]
                    }
                }
            }
        }));
    });

    // First call (no qn): server reports current_qn=1000 (low), accept_qn has higher options.
    server.mock(|when, then| {
        when.method(GET).path("/xlive/web-room/v2/index/getRoomPlayInfo");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "encrypted": false,
                "pwd_verified": true,
                "playurl_info": {
                    "playurl": {
                        "g_qn_desc": [
                            {"qn": 1000, "desc": "高清"},
                            {"qn": 2000, "desc": "原画"}
                        ],
                        "stream": [{
                            "protocol_name": "http_stream",
                            "format": [{
                                "format_name": "flv",
                                "codec": [{
                                    "codec_name": "avc",
                                    "current_qn": 1000,
                                    "accept_qn": [1000,2000],
                                    "base_url": "/live-bvc/low.flv",
                                    "url_info": [
                                        {"host": "https://up-mirror.bilivideo.com", "extra": "?x=1"}
                                    ]
                                }]
                            }]
                        }]
                    }
                }
            }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            bili_api_base: base.clone(),
            bili_live_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let man = client
        .decode_manifest("https://live.bilibili.com/10", ResolveOptions::default())
        .await
        .expect("manifest");

    assert_eq!(bad_qn0.hits(), 0, "should not request getRoomPlayInfo with qn=0");
    assert_eq!(man.site, Site::BiliLive);
    assert_eq!(man.room_id, "1010");
    assert!(man.variants.len() >= 2);
    let high = man.variants.iter().find(|v| v.quality == 2000).expect("high");
    assert!(high.url.as_deref().unwrap_or("").contains("high.flv"));
}

#[tokio::test]
async fn decode_manifest_fallback_to_playurl() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET)
            .path("/room/v1/Room/get_info")
            .query_param("room_id", "2");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "room_id": 222, "title": "t2", "live_status": 1, "user_cover": "" }
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo");
        then.status(500).body("nope");
    });

    server.mock(|when, then| {
        when.method(GET).path("/room/v1/Room/playUrl");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "current_quality": 0,
                "accept_quality": [],
                "current_qn": 1000,
                "quality_description": [{"qn": 1000, "desc": "高清"}],
                "durl": [{"url": "https://up-mirror.bilivideo.com/live-bvc/yy.flv?x=1"}]
            }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            bili_api_base: base.clone(),
            bili_live_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let man = client
        .decode_manifest("bilibili:2", ResolveOptions::default())
        .await
        .expect("manifest");
    assert_eq!(man.room_id, "222");
    assert!(man.variants.iter().any(|v| v.url.is_some()));
}

#[tokio::test]
async fn decode_manifest_need_password() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET)
            .path("/room/v1/Room/get_info")
            .query_param("room_id", "3");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "room_id": 333, "title": "t3", "live_status": 1, "user_cover": "" }
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "encrypted": true, "pwd_verified": false }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            bili_api_base: base.clone(),
            bili_live_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let err = client
        .decode_manifest("https://live.bilibili.com/3", ResolveOptions::default())
        .await
        .expect_err("should err");
    let msg = err.to_string();
    assert!(msg.contains("password") || msg.contains("NeedPassword"));
}

#[tokio::test]
async fn decode_manifest_room_play_info_ignores_qn_fallback_to_playurl() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET)
            .path("/room/v1/Room/get_info")
            .query_param("room_id", "11");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "room_id": 1111, "title": "t11", "live_status": 1, "user_cover": "" }
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/live_user/v1/UserInfo/get_anchor_in_room")
            .query_param("roomid", "1111");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "info": { "uname": "u11", "face": "f11" } }
        }));
    });

    // v2 called with qn=10000 but server ignores the switch and still returns current_qn=250.
    let v2_qn_ignored = server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo")
            .query_param("qn", "10000");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "encrypted": false,
                "pwd_verified": true,
                "playurl_info": {
                    "playurl": {
                        "g_qn_desc": [
                            {"qn": 250, "desc": "超清"},
                            {"qn": 10000, "desc": "原画"}
                        ],
                        "stream": [{
                            "protocol_name": "http_stream",
                            "format": [{
                                "format_name": "flv",
                                "codec": [{
                                    "codec_name": "avc",
                                    "current_qn": 250,
                                    "accept_qn": [250, 10000],
                                    "base_url": "/live-bvc/low_2500.flv",
                                    "url_info": [
                                        {"host": "https://up-mirror.bilivideo.com", "extra": "?x=1"}
                                    ]
                                }]
                            }]
                        }]
                    }
                }
            }
        }));
    });

    // Quality enumeration (no qn): current_qn is low.
    server.mock(|when, then| {
        when.method(GET).path("/xlive/web-room/v2/index/getRoomPlayInfo");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "encrypted": false,
                "pwd_verified": true,
                "playurl_info": {
                    "playurl": {
                        "g_qn_desc": [
                            {"qn": 250, "desc": "超清"},
                            {"qn": 10000, "desc": "原画"}
                        ],
                        "stream": [{
                            "protocol_name": "http_stream",
                            "format": [{
                                "format_name": "flv",
                                "codec": [{
                                    "codec_name": "avc",
                                    "current_qn": 250,
                                    "accept_qn": [250, 10000],
                                    "base_url": "/live-bvc/low_2500.flv",
                                    "url_info": [
                                        {"host": "https://up-mirror.bilivideo.com", "extra": "?x=1"}
                                    ]
                                }]
                            }]
                        }]
                    }
                }
            }
        }));
    });

    // v1 playUrl honors qn=10000 and returns a bluray url.
    let playurl_10000 = server.mock(|when, then| {
        when.method(GET)
            .path("/room/v1/Room/playUrl")
            .query_param("cid", "1111")
            .query_param("qn", "10000")
            .query_param("platform", "web");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "current_qn": 10000,
                "quality_description": [
                    {"qn": 250, "desc": "超清"},
                    {"qn": 10000, "desc": "原画"}
                ],
                "durl": [{"url": "https://up-mirror.bilivideo.com/live-bvc/hi_bluray.flv?y=1"}]
            }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            bili_api_base: base.clone(),
            bili_live_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");
    let man = client
        .decode_manifest("https://live.bilibili.com/11", ResolveOptions::default())
        .await
        .expect("manifest");

    assert_eq!(man.site, Site::BiliLive);
    assert_eq!(man.room_id, "1111");
    assert!(v2_qn_ignored.hits() >= 1);
    assert_eq!(playurl_10000.hits(), 1, "should fallback to v1 playUrl for qn=10000");

    let high = man.variants.iter().find(|v| v.quality == 10000).expect("high");
    assert!(
        high.url
            .as_deref()
            .unwrap_or("")
            .contains("hi_bluray.flv"),
        "expected v1 bluray url to be bound to qn=10000 variant"
    );
}

#[tokio::test]
async fn resolve_variant_fallback_to_playurl_when_v2_does_not_switch() {
    let server = MockServer::start();
    let base = server.base_url();

    // v2 called with qn=10000 but server ignores the switch.
    server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-room/v2/index/getRoomPlayInfo")
            .query_param("qn", "10000");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "encrypted": false,
                "pwd_verified": true,
                "playurl_info": {
                    "playurl": {
                        "g_qn_desc": [
                            {"qn": 250, "desc": "超清"},
                            {"qn": 10000, "desc": "原画"}
                        ],
                        "stream": [{
                            "protocol_name": "http_stream",
                            "format": [{
                                "format_name": "flv",
                                "codec": [{
                                    "codec_name": "avc",
                                    "current_qn": 250,
                                    "accept_qn": [250, 10000],
                                    "base_url": "/live-bvc/low_2500.flv",
                                    "url_info": [
                                        {"host": "https://up-mirror.bilivideo.com", "extra": "?x=1"}
                                    ]
                                }]
                            }]
                        }]
                    }
                }
            }
        }));
    });

    // v1 playUrl honors qn=10000.
    server.mock(|when, then| {
        when.method(GET)
            .path("/room/v1/Room/playUrl")
            .query_param("cid", "1111")
            .query_param("qn", "10000")
            .query_param("platform", "web");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "current_qn": 10000,
                "quality_description": [
                    {"qn": 250, "desc": "超清"},
                    {"qn": 10000, "desc": "原画"}
                ],
                "durl": [{"url": "https://up-mirror.bilivideo.com/live-bvc/hi_bluray.flv?y=1"}]
            }
        }));
    });

    let cfg = LivestreamConfig {
        endpoints: Endpoints {
            bili_api_base: base.clone(),
            bili_live_base: base.clone(),
            ..Endpoints::default()
        },
        env: fixed_env(),
    };
    let client = LivestreamClient::with_config(cfg).expect("client");

    let v = client
        .resolve_variant(Site::BiliLive, "1111", "bili_live:10000:原画")
        .await
        .expect("variant");
    assert_eq!(v.quality, 10000);
    assert!(
        v.url.as_deref().unwrap_or("").contains("hi_bluray.flv"),
        "expected v1 bluray url"
    );
}
