use chaos_core::music::model::QqMusicCookie;

#[test]
fn qq_cookie_allows_numeric_musicid() {
    let v = serde_json::json!({
        "musicid": 553740271,
        "musickey": "abc",
        "loginType": 0,
        "refreshKey": "rk",
        "expiredAt": 1700000000
    });

    let c: QqMusicCookie = serde_json::from_value(v).expect("deserialize");
    assert_eq!(c.musicid.as_deref(), Some("553740271"));
    assert_eq!(c.musickey.as_deref(), Some("abc"));
}

