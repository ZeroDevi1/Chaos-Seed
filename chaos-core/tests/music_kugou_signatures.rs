use std::collections::BTreeMap;

use chaos_core::music::providers::kugou::signatures::{
    sign_key, sign_key_lite, signature_android_params, signature_register_params, signature_web_params,
};

#[test]
fn kugou_signature_web_matches_known_vector() {
    let mut params = BTreeMap::new();
    params.insert("a".to_string(), "1".to_string());
    params.insert("b".to_string(), "2".to_string());
    assert_eq!(signature_web_params(&params), "70ccbef64fdcc9271fe883d1d7f07395");
}

#[test]
fn kugou_signature_android_matches_known_vector_with_body() {
    let mut params = BTreeMap::new();
    params.insert("a".to_string(), "1".to_string());
    params.insert("b".to_string(), "2".to_string());
    assert_eq!(
        signature_android_params(&params, Some(r#"{"x":1}"#)),
        "4b8732650b581102e39e3543da1d15f9"
    );
}

#[test]
fn kugou_signature_register_matches_known_vector() {
    let mut params = BTreeMap::new();
    params.insert("a".to_string(), "1".to_string());
    params.insert("b".to_string(), "2".to_string());
    assert_eq!(signature_register_params(&params), "3be0f2ebde7da28161927749ab76ba88");
}

#[test]
fn kugou_sign_key_matches_known_vector() {
    assert_eq!(
        sign_key("hash", "mid", "1", "1005"),
        "7d77a317a1c22fe7397cf3bfffb90396"
    );
    assert_eq!(
        sign_key_lite("hash", "mid", "1", "1005"),
        "a960edd7559bfbcf1184151b17874ac8"
    );
}
