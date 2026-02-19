use chaos_core::danmaku::model::{DanmakuEvent, DanmakuMethod, Site};
use chaos_core::subtitle::models::ThunderSubtitleItem;
use chaos_proto::{MusicDownloadStartParams, MusicDownloadTarget, MusicService};

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

fn music_download_start_json_with_target(target_json: &str) -> String {
    format!(
        r#"{{
  "config": {{}},
  "auth": {{}},
  "target": {target_json},
  "options": {{
    "qualityId": "flac",
    "outDir": "/tmp",
    "overwrite": false,
    "concurrency": 3,
    "retries": 2
  }}
}}"#
    )
}

#[test]
fn music_download_target_album_accepts_camelcase_album_id() {
    let json = music_download_start_json_with_target(r#"{ "type": "album", "service": "qq", "albumId": "123" }"#);
    let params: MusicDownloadStartParams = serde_json::from_str(&json).expect("deserialize");
    match params.target {
        MusicDownloadTarget::Album { service, album_id } => {
            assert_eq!(service, MusicService::Qq);
            assert_eq!(album_id, "123");
        }
        other => panic!("expected album target, got {other:?}"),
    }
}

#[test]
fn music_download_target_album_accepts_snakecase_album_id_alias() {
    let json =
        music_download_start_json_with_target(r#"{ "type": "album", "service": "qq", "album_id": "123" }"#);
    let params: MusicDownloadStartParams = serde_json::from_str(&json).expect("deserialize");
    match params.target {
        MusicDownloadTarget::Album { service, album_id } => {
            assert_eq!(service, MusicService::Qq);
            assert_eq!(album_id, "123");
        }
        other => panic!("expected album target, got {other:?}"),
    }
}

#[test]
fn music_download_target_artist_all_accepts_camelcase_artist_id() {
    let json =
        music_download_start_json_with_target(r#"{ "type": "artist_all", "service": "qq", "artistId": "456" }"#);
    let params: MusicDownloadStartParams = serde_json::from_str(&json).expect("deserialize");
    match params.target {
        MusicDownloadTarget::ArtistAll { service, artist_id } => {
            assert_eq!(service, MusicService::Qq);
            assert_eq!(artist_id, "456");
        }
        other => panic!("expected artist_all target, got {other:?}"),
    }
}

#[test]
fn music_download_target_artist_all_accepts_snakecase_artist_id_alias() {
    let json =
        music_download_start_json_with_target(r#"{ "type": "artist_all", "service": "qq", "artist_id": "456" }"#);
    let params: MusicDownloadStartParams = serde_json::from_str(&json).expect("deserialize");
    match params.target {
        MusicDownloadTarget::ArtistAll { service, artist_id } => {
            assert_eq!(service, MusicService::Qq);
            assert_eq!(artist_id, "456");
        }
        other => panic!("expected artist_all target, got {other:?}"),
    }
}

#[test]
fn music_download_target_serializes_camelcase_fields() {
    let target = MusicDownloadTarget::Album {
        service: MusicService::Qq,
        album_id: "123".to_string(),
    };
    let json = serde_json::to_string(&target).expect("serialize");
    assert!(json.contains("\"albumId\""));
    assert!(!json.contains("\"album_id\""));
}
