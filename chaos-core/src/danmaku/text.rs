/// Minimal normalization/sanitization for danmaku text before feeding UI renderers.
///
/// Motivation:
/// - Some platforms emit emoji sequences (ZWJ + variation selectors) that may render as tofu ("口")
///   depending on the backend/font availability.
/// - Some messages contain control characters/newlines which break single-line overlay layout.
///
/// We keep most characters intact, but strip the known problematic joiners/selectors and
/// normalize whitespace.
pub fn sanitize_danmaku_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;

    for ch in s.chars() {
        // Normalize newlines/tabs into spaces for single-line UI.
        let ch = match ch {
            '\r' | '\n' | '\t' => ' ',
            _ => ch,
        };

        // Drop control chars and common emoji shaping helpers that often show as tofu when unsupported.
        if ch.is_control()
            || ch == '\u{200D}' // ZWJ
            || ch == '\u{200C}' // ZWNJ
            || ('\u{FE00}'..='\u{FE0F}').contains(&ch)
        {
            continue;
        }

        // Drop private-use ranges (some clients/platforms may embed them).
        if ('\u{E000}'..='\u{F8FF}').contains(&ch)
            || ('\u{F0000}'..='\u{FFFFD}').contains(&ch)
            || ('\u{100000}'..='\u{10FFFD}').contains(&ch)
        {
            continue;
        }

        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
            continue;
        }

        prev_space = false;
        out.push(ch);
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_variation_selectors() {
        // U+2764 (heart) + U+FE0F (VS16)
        assert_eq!(sanitize_danmaku_text("❤\u{FE0F}"), "❤");
    }

    #[test]
    fn sanitize_strips_zwj() {
        // Woman technologist: "👩" + ZWJ + "💻" (may be unsupported as a ligature).
        assert_eq!(sanitize_danmaku_text("👩\u{200D}💻"), "👩💻");
    }

    #[test]
    fn sanitize_normalizes_whitespace() {
        assert_eq!(sanitize_danmaku_text("a\tb\nc\r\nd"), "a b c d");
    }

    #[test]
    fn sanitize_keeps_cjk() {
        assert_eq!(sanitize_danmaku_text("大大方方解说..."), "大大方方解说...");
    }
}
