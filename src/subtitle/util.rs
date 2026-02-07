use std::path::{Path, PathBuf};

pub fn sanitize_component(s: &str, max_len: usize) -> String {
    // Remove ASCII control chars and normalize separators / reserved chars.
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_control() || ch == '\u{7f}' {
            continue;
        }
        let replaced = match ch {
            '\\' | '/' => '_',
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            _ => ch,
        };
        out.push(replaced);
    }

    // Collapse whitespace.
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = out.trim().to_string();
    if out.is_empty() {
        out = "untitled".to_string();
    }
    if out.len() > max_len {
        out.truncate(max_len);
        out = out.trim_end().to_string();
        if out.is_empty() {
            out = "untitled".to_string();
        }
    }
    out
}

pub fn ensure_unique_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let suffix = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let dot_suffix = if suffix.is_empty() {
        "".to_string()
    } else {
        format!(".{}", suffix)
    };

    for i in 1..10_000 {
        let cand = parent.join(format!("{} ({}){}", stem, i, dot_suffix));
        if !cand.exists() {
            return Ok(cand);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("unable to find unique filename for: {}", path.display()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_component_basic() {
        assert_eq!(sanitize_component("  a/b  ", 80), "a_b");
        assert_eq!(sanitize_component("<>:\\\"|?*", 80), "________");
        assert_eq!(sanitize_component("", 80), "untitled");
    }

    #[test]
    fn sanitize_component_truncate() {
        let s = "a".repeat(200);
        let out = sanitize_component(&s, 120);
        assert_eq!(out.len(), 120);
    }

    #[test]
    fn ensure_unique_path_increments() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let p = dir.path().join("x.srt");
        std::fs::write(&p, b"1")?;
        let u1 = ensure_unique_path(&p)?;
        assert_eq!(u1.file_name().unwrap().to_str().unwrap(), "x (1).srt");
        std::fs::write(&u1, b"2")?;
        let u2 = ensure_unique_path(&p)?;
        assert_eq!(u2.file_name().unwrap().to_str().unwrap(), "x (2).srt");
        Ok(())
    }
}
