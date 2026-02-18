use chaos_core::bili_video::auth::{AuthState, refresh_cookie_if_needed_with};
use chaos_core::bili_video::parse::{fetch_view_info, parse_video_id};
use chaos_core::bili_video::playurl::{choose_qn_by_dfn_priority, fetch_playurl_dash, pick_dash_tracks};
use chaos_core::bili_video::select_page::select_page_indices;
use chaos_core::bili_video::subtitle::bcc_json_to_srt;
use chaos_core::bili_video::{BiliClient, BiliEndpoints};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;

fn mk_client(base: &str) -> BiliClient {
    let ep = BiliEndpoints {
        api_base: base.to_string(),
        passport_base: base.to_string(),
        www_base: base.to_string(),
    };
    BiliClient::with_endpoints(ep, std::time::Duration::from_secs(2)).expect("client")
}

#[test]
fn parse_video_id_bv_and_av() {
    let id = parse_video_id("https://www.bilibili.com/video/BV1AB411c7mD").expect("bv");
    assert_eq!(id.bvid.as_deref(), Some("BV1AB411c7mD"));
    assert!(id.aid.is_none());

    let id = parse_video_id("av123456").expect("av");
    assert_eq!(id.aid.as_deref(), Some("123456"));
}

#[test]
fn select_page_indices_variants() {
    assert_eq!(select_page_indices(3, "ALL").unwrap(), vec![0, 1, 2]);
    assert_eq!(select_page_indices(3, "1").unwrap(), vec![0]);
    assert_eq!(select_page_indices(3, "2,1").unwrap(), vec![0, 1]);
    assert_eq!(select_page_indices(5, "3-4").unwrap(), vec![2, 3]);
    assert_eq!(select_page_indices(5, "LAST").unwrap(), vec![4]);
    assert_eq!(select_page_indices(5, "LATEST").unwrap(), vec![4]);
}

#[tokio::test]
async fn view_and_playurl_dash_pick_codec_and_quality() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET).path("/x/web-interface/view");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "aid": 1,
                "bvid": "BV1AB411c7mD",
                "title": "T",
                "desc": "D",
                "pic": "P",
                "pubdate": 1700000000,
                "owner": { "name": "U", "mid": 99 },
                "pages": [{
                    "page": 1,
                    "cid": 100,
                    "part": "P1",
                    "duration": 12,
                    "dimension": { "width": 1920, "height": 1080 }
                }]
            }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/x/frontend/finger/spi");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "b_3": "b3", "b_4": "b4" }
        }));
    });

    let img_key = "a".repeat(32);
    let sub_key = "b".repeat(32);
    server.mock(move |when, then| {
        when.method(GET).path("/x/web-interface/nav");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "wbi_img": {
                "img_url": format!("https://i.example/{img_key}.png"),
                "sub_url": format!("https://i.example/{sub_key}.png")
            }}
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/x/player/wbi/playurl");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": {
                "quality": 80,
                "accept_quality": [127, 80],
                "accept_description": ["8K 超高清", "1080P 高码率"],
                "dash": {
                    "video": [
                        { "id": 80, "base_url": "http://v_hevc", "backup_url": [], "codecs": "hev1", "codecid": 12, "bandwidth": 2000, "width": 1920, "height": 1080, "frame_rate": "60" },
                        { "id": 80, "base_url": "http://v_avc", "backup_url": [], "codecs": "avc1", "codecid": 7, "bandwidth": 3000, "width": 1920, "height": 1080, "frame_rate": "60" }
                    ],
                    "audio": [
                        { "id": 30280, "base_url": "http://a", "backup_url": [], "codecs": "mp4a", "bandwidth": 128 }
                    ]
                }
            }
        }));
    });

    let client = mk_client(&base);
    let id = parse_video_id("BV1AB411c7mD").expect("id");
    let view = fetch_view_info(&client, &id, None).await.expect("view");
    assert_eq!(view.pages.len(), 1);

    let qn = choose_qn_by_dfn_priority(&[127, 80], &["8K 超高清".to_string(), "1080P 高码率".to_string()], "1080P 高码率,8K 超高清").unwrap();
    assert_eq!(qn, 80);

    let p = fetch_playurl_dash(&client, &view.bvid, &view.aid, &view.pages[0].cid, qn, None).await.expect("playurl");
    let (v, _a) = pick_dash_tracks(&p, "hevc,avc").expect("pick");
    assert_eq!(v.base_url, "http://v_hevc");
}

#[test]
fn bcc_to_srt_ok() {
    let json = serde_json::json!({
        "body": [
            { "from": 0.0, "to": 1.23, "content": "hello" },
            { "from": 2.0, "to": 3.0, "content": "world" }
        ]
    });
    let srt = bcc_json_to_srt(&json).expect("srt");
    assert!(srt.contains("00:00:00,000 --> 00:00:01,230"));
    assert!(srt.contains("hello"));
}

#[tokio::test]
async fn refresh_cookie_flow_mocked_ok() {
    let server = MockServer::start();
    let base = server.base_url();

    server.mock(|when, then| {
        when.method(GET).path("/x/frontend/finger/spi");
        then.status(200).json_body(serde_json::json!({"code":0,"data":{"b_3":"b3","b_4":"b4"}}));
    });

    server.mock(|when, then| {
        when.method(GET).path("/x/passport-login/web/cookie/info");
        then.status(200).json_body(serde_json::json!({
            "code": 0,
            "data": { "refresh": true, "timestamp": 1700000000000i64 }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/correspond/1/testpath");
        then.status(200).body(r#"<div id="1-name">abcdef</div>"#);
    });

    server.mock(|when, then| {
        when.method(POST).path("/x/passport-login/web/cookie/refresh");
        then.status(200)
            .header("set-cookie", "SESSDATA=newsess; Path=/; HttpOnly")
            .header("set-cookie", "bili_jct=newcsrf; Path=/")
            .json_body(serde_json::json!({
                "code": 0,
                "data": { "refresh_token": "newrt" }
            }));
    });

    server.mock(|when, then| {
        when.method(POST).path("/x/passport-login/web/confirm/refresh");
        then.status(200).json_body(serde_json::json!({ "code": 0, "data": {} }));
    });

    let client = mk_client(&base);
    let auth = AuthState {
        cookie: Some("bili_jct=oldcsrf; SESSDATA=oldsess".to_string()),
        refresh_token: Some("oldrt".to_string()),
    };

    let out = refresh_cookie_if_needed_with(&client, &auth, |_ts| Ok("testpath".to_string()))
        .await
        .expect("refresh");
    let c = out.cookie.unwrap();
    assert!(c.contains("SESSDATA=newsess"));
    assert!(c.contains("bili_jct=newcsrf"));
    assert_eq!(out.refresh_token.as_deref(), Some("newrt"));
}

