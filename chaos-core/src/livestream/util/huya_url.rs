use std::collections::BTreeMap;

use base64::Engine;

fn md5_hex(s: &str) -> String {
    format!("{:x}", md5::compute(s))
}

fn httpsify(url: &str) -> String {
    url.replace("http://", "https://")
}

fn percent_decode(s: &str) -> String {
    // Best-effort percent decoding for UTF-8 strings.
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

fn base64_decode_str(s: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    String::from_utf8(bytes).ok()
}

pub fn rot_uid(uid: u64) -> u64 {
    // Equivalent to: keep high 32 bits, rotate low 32 bits left by 8.
    let upper = uid & 0xFFFF_FFFF_0000_0000;
    let lower = (uid as u32).rotate_left(8) as u64;
    upper | lower
}

pub fn ws_secret(
    anti_codes: &BTreeMap<String, String>,
    convert_uid: u64,
    seqid: i64,
    stream_name: &str,
) -> Option<String> {
    let fm = anti_codes.get("fm")?;
    let ws_time = anti_codes.get("wsTime")?;
    let ctype = anti_codes.get("ctype")?;
    let t = anti_codes.get("t").map(|s| s.as_str()).unwrap_or("100");

    let u = percent_decode(fm);
    let u = base64_decode_str(&u)?;

    let s = md5_hex(&format!("{seqid}|{ctype}|{t}"));
    let mut u = u.replace("$0", &convert_uid.to_string());
    u = u.replace("$1", stream_name);
    u = u.replace("$2", &s);
    u = u.replace("$3", ws_time);
    Some(md5_hex(&u))
}

fn parse_query_pairs(s: &str) -> BTreeMap<String, String> {
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

/// Build a Huya live FLV url (ported from IINA+ `HuyaUrl.format`).
pub fn format(
    uid: u32,
    now_ms: i64,
    stream_name: &str,
    flv_url: &str,
    flv_suffix: &str,
    flv_anti_code: &str,
    ratio: Option<i32>,
) -> Option<String> {
    let sid = now_ms;
    let mut anti = parse_query_pairs(flv_anti_code);
    let seqid = uid as i64 + now_ms;
    let convert_uid = rot_uid(uid as u64);
    let ws_secret = ws_secret(&anti, convert_uid, seqid, stream_name)?;

    anti.insert("u".to_string(), convert_uid.to_string());
    anti.insert("wsSecret".to_string(), ws_secret);
    anti.insert("seqid".to_string(), seqid.to_string());
    anti.insert("sdk_sid".to_string(), sid.to_string());
    match ratio {
        Some(v) if v > 0 => {
            anti.insert("ratio".to_string(), v.to_string());
        }
        _ => {
            anti.remove("ratio");
        }
    }

    // Keep a stable order similar to IINA+'s query template; append extras sorted.
    let template_keys = [
        "wsSecret", "wsTime", "seqid", "ctype", "ver", "fs", "ratio", "dMod", "sdkPcdn", "u", "t",
        "sv", "sdk_sid", "a_block", "sf",
    ];

    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut used = std::collections::BTreeSet::<String>::new();
    for k in template_keys {
        if let Some(v) = anti.get(k) {
            pairs.push((k.to_string(), v.to_string()));
            used.insert(k.to_string());
        }
    }
    for (k, v) in anti.iter() {
        if !used.contains(k) {
            pairs.push((k.clone(), v.clone()));
        }
    }

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
    fn rot_uid_matches_rotate_left_low_32bits() {
        let uid = 0x1122_3344_5566_7788_u64;
        let out = rot_uid(uid);
        let upper = uid & 0xFFFF_FFFF_0000_0000;
        let lower = ((uid as u32).rotate_left(8) as u64) & 0xFFFF_FFFF;
        assert_eq!(out, upper | lower);
    }

    #[test]
    fn ws_secret_is_stable() {
        // fm is base64("abc$0|$1|$2|$3")
        let fm_raw = "abc$0|$1|$2|$3".as_bytes();
        let fm_b64 = base64::engine::general_purpose::STANDARD.encode(fm_raw);
        let mut anti = BTreeMap::<String, String>::new();
        anti.insert("fm".to_string(), fm_b64);
        anti.insert("wsTime".to_string(), "67b6c60d".to_string());
        anti.insert("ctype".to_string(), "huya_live".to_string());
        anti.insert("t".to_string(), "100".to_string());
        let sec = ws_secret(&anti, 123, 456, "s").unwrap();
        // Expected: md5("abc123|s|md5(\"456|huya_live|100\")|67b6c60d")
        let s = md5_hex("456|huya_live|100");
        let expected = md5_hex(&format!("abc123|s|{s}|67b6c60d"));
        assert_eq!(sec, expected);
    }
}
