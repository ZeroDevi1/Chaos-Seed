use std::path::Path;

use chaos_core::music::util::{build_track_path, sanitize_component};

#[test]
fn sanitize_component_replaces_windows_forbidden_chars() {
    let s = r#"a<>:"/\|?*b"#;
    let out = sanitize_component(s);
    assert!(out.contains('_'));
    assert!(!out.contains('<'));
    assert!(!out.contains('>'));
    assert!(!out.contains(':'));
    assert!(!out.contains('"'));
    assert!(!out.contains('/'));
    assert!(!out.contains('\\'));
    assert!(!out.contains('|'));
    assert!(!out.contains('?'));
    assert!(!out.contains('*'));
}

#[test]
fn build_track_path_uses_artist_album_and_trackno() {
    let out_dir = Path::new("D:/Music");
    let p = build_track_path(
        out_dir,
        &["Adele".to_string()],
        Some("Hello: Deluxe"),
        Some(1),
        "Hello/World",
        "mp3",
    );

    let s = p.to_string_lossy().replace('\\', "/");
    assert!(s.contains("Adele"));
    assert!(s.contains("Hello_ Deluxe"));
    assert!(s.contains("01 - Hello_World.mp3"));
}
