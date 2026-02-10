use chaos_core::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsSearchTerm, LyricsService};
use chaos_core::lyrics::quality;

fn req_info(title: &str, artist: &str, duration_ms: Option<u64>) -> LyricsSearchRequest {
    let mut r = LyricsSearchRequest::new(LyricsSearchTerm::Info {
        title: title.to_string(),
        artist: artist.to_string(),
        album: None,
    });
    r.duration_ms = duration_ms;
    r.limit = 6;
    r
}

fn base_result(title: Option<&str>, artist: Option<&str>, duration_ms: Option<u64>) -> LyricsSearchResult {
    LyricsSearchResult {
        service: LyricsService::QQMusic,
        service_token: "t".to_string(),
        title: title.map(|s| s.to_string()),
        artist: artist.map(|s| s.to_string()),
        album: None,
        duration_ms,
        quality: 0.0,
        matched: false,
        has_translation: false,
        has_inline_timetags: false,
        lyrics_original: "x".to_string(),
        lyrics_translation: None,
        debug: None,
    }
}

#[test]
fn matched_info_requires_title_and_artist() {
    let req = req_info("Hello", "Adele", None);
    let r1 = base_result(Some("Hello"), Some("Adele"), None);
    assert!(quality::is_matched(&r1, &req));

    let r2 = base_result(Some("Hello"), None, None);
    assert!(!quality::is_matched(&r2, &req));
}

#[test]
fn quality_increases_with_translation_and_timetags() {
    let req = req_info("Hello", "Adele", Some(300_000));
    let mut r = base_result(Some("Hello"), Some("Adele"), Some(300_000));
    let q0 = quality::compute_quality(&r, &req);
    r.has_translation = true;
    let q1 = quality::compute_quality(&r, &req);
    r.has_inline_timetags = true;
    let q2 = quality::compute_quality(&r, &req);
    assert!(q1 > q0);
    assert!(q2 > q1);
}

#[test]
fn duration_quality_penalizes_large_diff() {
    let req = req_info("Hello", "Adele", Some(300_000));
    let r_close = base_result(Some("Hello"), Some("Adele"), Some(295_000));
    let r_far = base_result(Some("Hello"), Some("Adele"), Some(200_000));
    let q_close = quality::compute_quality(&r_close, &req);
    let q_far = quality::compute_quality(&r_far, &req);
    assert!(q_close > q_far);
}

