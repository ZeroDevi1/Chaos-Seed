use chaos_core::lyrics::util;

#[test]
fn extract_json_object_from_jsonp() {
    let s = "callback123({\"a\":1,\"b\":2});";
    let got = util::extract_json_from_jsonp_str(s).unwrap();
    assert_eq!(got, "{\"a\":1,\"b\":2}");
}

#[test]
fn extract_json_array_from_jsonp() {
    let s = "cb([1,2,3])";
    let got = util::extract_json_from_jsonp_str(s).unwrap();
    assert_eq!(got, "[1,2,3]");
}

#[test]
fn decode_xml_entities_named_and_numeric() {
    let s = "a&amp;b&lt;c&gt;d&quot;e&apos;f&#33;&#x21;";
    let got = util::decode_xml_entities(s);
    assert_eq!(got.as_ref(), "a&b<c>d\"e'f!!");
}
