/// Extract a balanced JSON/JS object substring starting at the first `{` after `marker`.
///
/// This is a best-effort helper for sites embedding JSON-like blobs in HTML/JS. We track string
/// literals to avoid counting braces inside `"..."`.
pub fn extract_balanced_object_after_marker(input: &str, marker: &str) -> Option<String> {
    let idx = input.find(marker)?;
    let after = &input[idx + marker.len()..];
    extract_balanced_object(after)
}

/// Extract a balanced object substring starting at the first `{` in `input`.
pub fn extract_balanced_object(input: &str) -> Option<String> {
    let start = input.find('{')?;
    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut escape = false;
    let mut end: Option<usize> = None;

    for (i, ch) in input[start..].char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_str {
            match ch {
                '\\' => escape = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i);
                    break;
                }
            }
            _ => {}
        }
    }

    let end = end?;
    Some(input[start..=end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_object() {
        let s = r#"xxx {"a": {"b": 1}, "c": 2} yyy"#;
        let out = extract_balanced_object(s).unwrap();
        assert_eq!(out, r#"{"a": {"b": 1}, "c": 2}"#);
    }

    #[test]
    fn ignores_braces_in_strings() {
        let s = r#"var x = {"a":"{not}","b":1}"#;
        let out = extract_balanced_object(s).unwrap();
        assert_eq!(out, r#"{"a":"{not}","b":1}"#);
    }

    #[test]
    fn returns_none_when_unbalanced() {
        let s = r#"{"a": {"b": 1}"#;
        assert!(extract_balanced_object(s).is_none());
    }

    #[test]
    fn extracts_after_marker() {
        let s = r#"prefix marker {"k":1} suffix"#;
        let out = extract_balanced_object_after_marker(s, "marker").unwrap();
        assert_eq!(out, r#"{"k":1}"#);
    }
}
