use std::collections::BTreeMap;

use base64::Engine;

fn md5_hex(s: &str) -> String {
    format!("{:x}", md5::compute(s))
}

fn httpsify(url: &str) -> String {
    url.replace("http://", "https://")
}

fn percent_decode(s: &str) -> String {
    // Best-effort percent decoding for UTF-8 strings. (Keep '+' as '+'.)
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = bytes[i + 1];
            let h2 = bytes[i + 2];
            let v1 = (h1 as char).to_digit(16);
            let v2 = (h2 as char).to_digit(16);
            if let (Some(v1), Some(v2)) = (v1, v2) {
                out.push(((v1 << 4) + v2) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn percent_encode_component(s: &str) -> String {
    // Encode everything not in the unreserved set.
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let unreserved =
            matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~');
        if unreserved {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn base64_decode_str(s: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    String::from_utf8(bytes).ok()
}

fn parse_query_pairs_raw(s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::<String, String>::new();
    for part in s.split('&') {
        let mut it = part.splitn(2, '=');
        let k = it.next().unwrap_or("").trim();
        let v = it.next().unwrap_or("").trim();
        if !k.is_empty() {
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}

fn rotl32_8(v: u32) -> u32 {
    v.rotate_left(8)
}

/// Build a Huya live FLV url (aligned with dart_simple_live `buildAntiCode`).
pub fn format(
    stream_name: &str,
    flv_url: &str,
    flv_suffix: &str,
    flv_anti_code: &str,
    presenter_uid: u32,
    now_ms: i64,
    ratio: Option<i32>,
) -> Option<String> {
    let anti = parse_query_pairs_raw(flv_anti_code);
    let fm_raw = anti.get("fm")?.to_string();
    let ws_time = anti.get("wsTime")?.to_string();
    let ctype = anti.get("ctype")?.to_string();
    let fs = anti.get("fs")?.to_string();
    let platform_id: i32 = anti
        .get("t")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);

    let is_wap = platform_id == 103;
    let seqid = presenter_uid as i64 + now_ms;
    let secret_hash = md5_hex(&format!("{seqid}|{ctype}|{platform_id}"));

    // fm is base64, sometimes percent-encoded.
    let fm_dec = percent_decode(&fm_raw);
    let fm_b64 = base64_decode_str(&fm_dec)?;
    let secret_prefix = fm_b64.split('_').next().unwrap_or("").trim().to_string();
    if secret_prefix.is_empty() {
        return None;
    }

    let convert_uid = rotl32_8(presenter_uid);
    let calc_uid: u32 = if is_wap { presenter_uid } else { convert_uid };

    let secret_str = format!("{secret_prefix}_{calc_uid}_{stream_name}_{secret_hash}_{ws_time}");
    let ws_secret = md5_hex(&secret_str);

    // Keep insertion order stable (Dart maps preserve insertion order).
    let mut pairs: Vec<(String, String)> = Vec::new();
    pairs.push(("wsSecret".to_string(), ws_secret));
    pairs.push(("wsTime".to_string(), ws_time.clone()));
    pairs.push(("seqid".to_string(), seqid.to_string()));
    pairs.push(("ctype".to_string(), ctype));
    pairs.push(("ver".to_string(), "1".to_string()));
    pairs.push(("fs".to_string(), fs));
    // Dart re-encodes the decoded fm value as a component.
    pairs.push(("fm".to_string(), percent_encode_component(&fm_dec)));
    pairs.push(("t".to_string(), platform_id.to_string()));

    if is_wap {
        // This branch is rare for mp endpoints; still implement for completeness.
        let ws_time_i = i64::from_str_radix(
            ws_time.trim_start_matches("0x").trim_start_matches("0X"),
            16,
        )
        .unwrap_or(0);
        let jitter_ms = fastrand::f64();
        let ct = ((ws_time_i as f64 + jitter_ms) * 1000.0) as i64;
        let uuid = (((ct.rem_euclid(10_000_000_000)) as f64 + fastrand::f64()) * 1000.0
            % (u32::MAX as f64)) as u32;
        pairs.push(("uid".to_string(), presenter_uid.to_string()));
        pairs.push(("uuid".to_string(), uuid.to_string()));
    } else {
        pairs.push(("u".to_string(), convert_uid.to_string()));
    }

    if let Some(r) = ratio {
        if r > 0 {
            pairs.push(("ratio".to_string(), r.to_string()));
        }
    }

    // Force H.264 like dart_simple_live (`codec=264`).
    pairs.push(("codec".to_string(), "264".to_string()));

    let qs = pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");

    let base = httpsify(flv_url);
    Some(format!("{base}/{stream_name}.{flv_suffix}?{qs}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotl32_8_matches_rotate_left() {
        assert_eq!(rotl32_8(0x1122_3344), 0x2233_4411);
    }

    #[test]
    fn format_includes_required_params() {
        // fm is base64("prefix_$0_$1_$2_$3"), percent-encoded.
        let fm_raw = "prefix_$0_$1_$2_$3".as_bytes();
        let fm_b64 = base64::engine::general_purpose::STANDARD.encode(fm_raw);
        let anti = format!("wsTime=67b6c60d&ctype=tars_mp&t=102&fs=1&fm={fm_b64}");

        let url = format("s", "http://x", "flv", &anti, 777, 1_000, Some(2000)).expect("url");
        assert!(url.starts_with("https://x/s.flv?"));
        assert!(url.contains("wsSecret="));
        assert!(url.contains("wsTime=67b6c60d"));
        assert!(url.contains("seqid="));
        assert!(url.contains("ver=1"));
        assert!(url.contains("fs=1"));
        assert!(url.contains("t=102"));
        assert!(url.contains("u="));
        assert!(url.contains("ratio=2000"));
        assert!(url.contains("codec=264"));
    }
}
