use chaos_core::danmaku::model::Site;
use chaos_core::live_directory::client::{
    LiveDirectoryClient, LiveDirectoryConfig, LiveDirectoryEndpoints,
};
use chaos_core::livestream::client::EnvConfig;
use httpmock::Method::GET;
use httpmock::MockServer;

fn fixed_env() -> EnvConfig {
    EnvConfig {
        now_ms: std::sync::Arc::new(|| 1_740_031_240_996),
        now_s: std::sync::Arc::new(|| 1_740_031_240),
        rng: std::sync::Arc::new(std::sync::Mutex::new(fastrand::Rng::with_seed(3))),
    }
}

fn mk_client(base: &str) -> LiveDirectoryClient {
    let cfg = LiveDirectoryConfig {
        endpoints: LiveDirectoryEndpoints {
            bili_live_api_base: base.to_string(),
            bili_api_base: base.to_string(),
            bili_live_base: base.to_string(),
            huya_base: base.to_string(),
            huya_live_cdn_base: base.to_string(),
            huya_search_base: base.to_string(),
            douyu_base: base.to_string(),
            douyu_m_base: base.to_string(),
        },
        env: fixed_env(),
        timeout: std::time::Duration::from_secs(2),
    };
    LiveDirectoryClient::with_config(cfg).expect("client")
}

#[test]
fn bili_wbi_mixin_key_matches_known_value() {
    use chaos_core::live_directory::util::bili_wbi::BiliWbi;
    let origin = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-_";
    let mixin = BiliWbi::mixin_key(origin).expect("mixin");
    // Precomputed from the dart algorithm + table in refs/dart_simple_live.
    assert_eq!(mixin, "KLi2R8nwfOavW3JzrH5Nx9GjtseDcCFd");
    assert_eq!(mixin.len(), 32);
}

#[test]
fn bili_wbi_filter_value_removes_reserved_chars() {
    use chaos_core::live_directory::util::bili_wbi::BiliWbi;
    assert_eq!(BiliWbi::filter_wbi_value("a!b'c(d)e*f)g"), "abcdefg");
}

#[test]
fn bili_wbi_sign_query_adds_wts_and_w_rid() {
    use chaos_core::live_directory::util::bili_wbi::BiliWbi;
    let params = vec![
        ("b".to_string(), "2".to_string()),
        ("a".to_string(), "1!2".to_string()),
    ];
    let out = BiliWbi::sign_query(&params, "mixin", 123);

    let mut map = std::collections::HashMap::new();
    for (k, v) in out {
        map.insert(k, v);
    }
    assert_eq!(map.get("wts").map(|s| s.as_str()), Some("123"));
    let rid = map.get("w_rid").expect("w_rid");
    assert_eq!(rid.len(), 32);
    assert!(rid.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn bili_categories_recommend_category_rooms_search_ok() {
    let server = MockServer::start();
    let base = server.base_url();

    // wbi keys
    server.mock(|when, then| {
        when.method(GET).path("/x/web-interface/nav");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "wbi_img": {
                    "img_url": "https://i.example/0123456789abcdef0123456789abcdef.png",
                    "sub_url": "https://i.example/fedcba9876543210fedcba9876543210.png"
                }
            }
        }));
    });

    // access_id page
    server.mock(|when, then| {
        when.method(GET).path("/lol");
        then.status(200)
            .body(r#"<html>"access_id":"abc\"123"</html>"#);
    });

    server.mock(|when, then| {
        when.method(GET).path("/room/v1/Area/getList");
        then.status(200).json_body(serde_json::json!({
            "data": [{
                "id": 1,
                "name": "网游",
                "list": [{ "id": 11, "name": "英雄联盟", "parent_id": 1, "pic": "p" }]
            }]
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-interface/v1/second/getListByArea");
        then.status(200).json_body(serde_json::json!({
            "data": { "list": [{
                "roomid": 100,
                "title": "t",
                "cover": "//c",
                "uname": "u",
                "online": 123
            }]}
        }));
    });

    server.mock(|when, then| {
        when.method(GET)
            .path("/xlive/web-interface/v1/second/getList");
        then.status(200).json_body(serde_json::json!({
            "data": { "has_more": 1, "list": [{
                "roomid": 101,
                "title": "t2",
                "cover": "https://c2",
                "uname": "u2",
                "online": 456
            }]}
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/x/web-interface/search/type");
        then.status(200).json_body(serde_json::json!({
            "data": { "result": { "live_room": [{
                "roomid": 102,
                "title": "<em>xx</em>hello",
                "cover": "//c3",
                "uname": "u3",
                "online": 1
            }]}}
        }));
    });

    let client = mk_client(&base);

    let cats = client.get_categories(Site::BiliLive).await.expect("cats");
    assert_eq!(cats.len(), 1);
    assert_eq!(cats[0].children.len(), 1);

    let rec = client
        .get_recommend_rooms(Site::BiliLive, 1)
        .await
        .expect("rec");
    assert_eq!(rec.items.len(), 1);
    assert_eq!(rec.items[0].input, "bilibili:100");
    assert_eq!(rec.items[0].cover.as_deref(), Some("https://c@400w.jpg"));

    let rooms = client
        .get_category_rooms(Site::BiliLive, Some("1"), "11", 1)
        .await
        .expect("rooms");
    assert!(rooms.has_more);
    assert_eq!(rooms.items[0].input, "bilibili:101");

    let search = client
        .search_rooms(Site::BiliLive, "k", 1)
        .await
        .expect("search");
    assert_eq!(search.items[0].title, "xxhello");
}

#[tokio::test]
async fn huya_categories_recommend_category_rooms_search_ok() {
    let server = MockServer::start();
    let base = server.base_url();

    // sub-categories for bussType 1/2/8/3
    for bt in ["1", "2", "8", "3"] {
        server.mock(move |when, then| {
            when.method(GET)
                .path("/liveconfig/game/bussLive")
                .query_param("bussType", bt);
            then.status(200).json_body(serde_json::json!({
                "data": [{ "gid": { "value": "100,200" }, "gameFullName": "LOL" }]
            }));
        });
    }

    server.mock(|when, then| {
        when.method(GET).path("/cache.php");
        then.status(200).body(r#"{"data":{"datas":[{"profileRoom":123,"screenshot":"c","introduction":"i","roomName":"","nick":"u","totalCount":"99"}],"page":1,"totalPage":2}}"#);
    });

    server.mock(|when, then| {
        when.method(GET).path("/");
        then.status(200).json_body(serde_json::json!({
            "response": { "3": { "docs": [{
                "room_id": "555",
                "game_introduction": "t",
                "game_screenshot": "c",
                "game_nick": "u",
                "game_total_count": 1
            }]}}
        }));
    });

    let client = mk_client(&base);

    let cats = client.get_categories(Site::Huya).await.expect("cats");
    assert_eq!(cats.len(), 4);
    assert!(!cats[0].children.is_empty());

    let rec = client
        .get_recommend_rooms(Site::Huya, 1)
        .await
        .expect("rec");
    assert!(rec.has_more);
    assert_eq!(rec.items[0].input, "huya:123");

    let rooms = client
        .get_category_rooms(Site::Huya, None, "100", 1)
        .await
        .expect("rooms");
    assert_eq!(rooms.items[0].input, "huya:123");

    let search = client
        .search_rooms(Site::Huya, "k", 1)
        .await
        .expect("search");
    assert_eq!(search.items[0].input, "huya:555");
}

#[tokio::test]
async fn douyu_categories_recommend_category_rooms_search_ok() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET).path("/api/cate/list");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "cate1Info": [{ "cate1Id": 1, "cate1Name": "A" }],
                "cate2Info": [{ "cate1Id": 1, "cate2Id": 2, "cate2Name": "B", "icon": "i" }]
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/japi/weblist/apinc/allpage/6/1");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "pgcnt": 2,
                "rl": [{ "type": 1, "rid": 1, "rn": "t", "rs16": "c", "nn": "u", "ol": 3 }]
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/gapi/rkc/directory/mixList/2_2/1");
        then.status(200).json_body(serde_json::json!({
            "data": {
                "pgcnt": 1,
                "rl": [{ "type": 1, "rid": 2, "rn": "t2", "rs16": "c2", "nn": "u2", "ol": 4 }]
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/japi/search/api/searchShow");
        then.status(200).json_body(serde_json::json!({
            "data": { "relateShow": [{
                "rid": 3,
                "roomName": "t3",
                "roomSrc": "c3",
                "nickName": "u3",
                "hot": "2.3万"
            }]}
        }));
    });

    let client = mk_client(&base);

    let cats = client.get_categories(Site::Douyu).await.expect("cats");
    assert_eq!(cats.len(), 1);
    assert_eq!(cats[0].children.len(), 1);

    let rec = client
        .get_recommend_rooms(Site::Douyu, 1)
        .await
        .expect("rec");
    assert!(rec.has_more);
    assert_eq!(rec.items[0].input, "douyu:1");

    let rooms = client
        .get_category_rooms(Site::Douyu, None, "2", 1)
        .await
        .expect("rooms");
    assert!(!rooms.has_more);
    assert_eq!(rooms.items[0].input, "douyu:2");

    let search = client
        .search_rooms(Site::Douyu, "k", 1)
        .await
        .expect("search");
    assert_eq!(search.items[0].online, Some(23_000));
}
