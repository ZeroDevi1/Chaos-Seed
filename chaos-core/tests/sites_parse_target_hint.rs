use chaos_core::danmaku::model::{DanmakuError, Site};
use chaos_core::danmaku::sites::parse_target_hint;

#[test]
fn parse_bili_url() {
    let (site, room) = parse_target_hint("https://live.bilibili.com/123").expect("parse");
    assert_eq!(site, Site::BiliLive);
    assert_eq!(room, "123");
}

#[test]
fn parse_douyu_url() {
    let (site, room) = parse_target_hint("https://www.douyu.com/999").expect("parse");
    assert_eq!(site, Site::Douyu);
    assert_eq!(room, "999");
}

#[test]
fn parse_huya_url() {
    let (site, room) = parse_target_hint("https://www.huya.com/abc").expect("parse");
    assert_eq!(site, Site::Huya);
    assert_eq!(room, "abc");
}

#[test]
fn parse_prefix_room_id() {
    let (site, room) = parse_target_hint("douyu:12345").expect("parse");
    assert_eq!(site, Site::Douyu);
    assert_eq!(room, "12345");
}

#[test]
fn parse_empty_is_error() {
    let err = parse_target_hint("   ").expect_err("should error");
    match err {
        DanmakuError::InvalidInput(s) => assert!(s.contains("empty")),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parse_unknown_host_is_error() {
    let err = parse_target_hint("https://example.com/123").expect_err("should error");
    match err {
        DanmakuError::InvalidInput(s) => assert!(s.contains("unsupported url host")),
        other => panic!("unexpected error: {other:?}"),
    }
}
