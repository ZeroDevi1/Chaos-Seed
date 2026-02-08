use chaos_core::danmaku::model::{DanmakuEvent, DanmakuMethod, Site};
use chaos_core::subtitle::models::ThunderSubtitleItem;

#[test]
fn thunder_subtitle_item_is_serializable() {
    let item = ThunderSubtitleItem {
        name: "Example".to_string(),
        ext: "srt".to_string(),
        score: 9.5,
        languages: vec!["zh".to_string(), "en".to_string()],
        ..ThunderSubtitleItem::default()
    };
    let json = serde_json::to_string(&item).expect("serialize");
    assert!(json.contains("Example"));
}

#[test]
fn danmaku_event_is_serializable() {
    let ev = DanmakuEvent::new(Site::BiliLive, "123", DanmakuMethod::SendDM, "hello", None);
    let json = serde_json::to_string(&ev).expect("serialize");
    assert!(json.contains("hello"));
    assert!(json.contains("received_at_ms"));
}
