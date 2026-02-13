use chaos_core::music::providers::qq::sign_request_payload;

#[test]
fn qq_sign_is_stable_for_known_payload() {
    // Precomputed from the reference algorithm (MD5 -> head/tail/middle xor -> base64 -> cleanup).
    let payload = r#"{"comm":{"ct":"19","cv":"1859","uin":"0"},"req":{"method":"DoSearchForQQMusicDesktop","module":"music.search.SearchCgiService","param":{"search_type":0,"query":"Hello","page_num":1,"num_per_page":10,"grp":1}}}"#;
    let sign = sign_request_payload(payload).expect("sign");
    assert_eq!(sign, "zzb0550d996q7woarat5bllem6fs0bbgce19fd27");
}

