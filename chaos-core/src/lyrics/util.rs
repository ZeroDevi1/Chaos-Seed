use std::borrow::Cow;

/// Extract the JSON payload from a JSONP-like response by slicing from the first opening
/// brace/bracket to the last closing brace/bracket.
///
/// This is intentionally tolerant because some providers wrap JSON in callbacks with varying
/// prefix/suffix lengths.
pub fn extract_json_from_jsonp(input: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(input);
    extract_json_from_jsonp_str(&s).map(|v| v.to_string())
}

pub fn extract_json_from_jsonp_str(s: &str) -> Option<&str> {
    let obj_idx = s.find('{');
    let arr_idx = s.find('[');
    let (open, close, start) = match (obj_idx, arr_idx) {
        (Some(o), Some(a)) if o < a => ('{', '}', o),
        (Some(_o), Some(a)) => ('[', ']', a),
        (Some(o), None) => ('{', '}', o),
        (None, Some(a)) => ('[', ']', a),
        (None, None) => return None,
    };

    let end = s.rfind(close)?;
    if end <= start {
        return None;
    }
    let slice = &s[start..=end];
    // A quick sanity check: ensure the slice begins with the expected opener.
    if !slice.starts_with(open) {
        return None;
    }
    Some(slice)
}

/// Decode basic XML/HTML entities commonly seen in QQ歌词接口：
/// - Named: &amp; &lt; &gt; &quot; &apos;
/// - Numeric: &#123; and hex: &#x1f;
pub fn decode_xml_entities(input: &str) -> Cow<'_, str> {
    // Fast path: no entities.
    if !input.as_bytes().contains(&b'&') {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        if bytes[i] != b'&' {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        // Find ';'
        let semi = match input[i..].find(';') {
            Some(off) => i + off,
            None => {
                out.push('&');
                i += 1;
                continue;
            }
        };

        let entity = &input[i + 1..semi];
        let decoded_str = match entity {
            "amp" => Some("&"),
            "lt" => Some("<"),
            "gt" => Some(">"),
            "quot" => Some("\""),
            "apos" => Some("'"),
            _ => None,
        };

        let decoded_char = if decoded_str.is_some() {
            None
        } else if entity.starts_with("#x") || entity.starts_with("#X") {
            u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32)
        } else if let Some(num) = entity.strip_prefix('#') {
            num.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            None
        };

        match (decoded_str, decoded_char) {
            (Some(repl), _) => out.push_str(repl),
            (None, Some(ch)) => out.push(ch),
            (None, None) => {
                out.push('&');
                out.push_str(entity);
                out.push(';');
            }
        }
        i = semi + 1;
    }

    Cow::Owned(out)
}

pub fn percent_encode_component(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Very small HTML-to-text helper for Syair: strip tags and keep basic line breaks.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_buf = String::new();

    for ch in html.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let t = tag_buf.trim().to_ascii_lowercase();
                if t.starts_with("br") || t.starts_with("/p") || t.starts_with("p") {
                    out.push('\n');
                }
                tag_buf.clear();
            } else {
                tag_buf.push(ch);
            }
            continue;
        }

        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }

        out.push(ch);
    }

    // Decode entities after tag stripping.
    decode_xml_entities(&out).into_owned()
}
