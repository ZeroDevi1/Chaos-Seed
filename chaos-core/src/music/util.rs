use std::path::{Path, PathBuf};

// Keep this simple and deterministic; shared between daemon/UI expectations.
pub fn sanitize_component(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }

    // Windows forbidden: <>:"/\\|?* + ASCII control chars.
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        let bad = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || (ch as u32) < 0x20;
        out.push(if bad { '_' } else { ch });
    }

    let out = out.trim().trim_end_matches('.').trim().to_string();
    if out.is_empty() {
        "unknown".to_string()
    } else if out.len() > 150 {
        out.chars().take(150).collect()
    } else {
        out
    }
}

pub fn pick_primary_artist(artists: &[String]) -> String {
    artists
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

pub fn build_track_path(
    out_dir: &Path,
    artists: &[String],
    album: Option<&str>,
    track_no_1based: Option<u32>,
    title: &str,
    ext: &str,
) -> PathBuf {
    let artist = sanitize_component(&pick_primary_artist(artists));
    let album = album
        .map(|s| sanitize_component(s))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Single".to_string());

    let mut file = String::new();
    if let Some(n) = track_no_1based {
        file.push_str(&format!("{:02} - ", n));
    }
    file.push_str(&sanitize_component(title));
    if !ext.trim().is_empty() {
        file.push('.');
        file.push_str(ext.trim().trim_start_matches('.'));
    }

    out_dir.join(artist).join(album).join(file)
}

fn render_brace_template(template: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    loop {
        let Some(start) = rest.find("{{") else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..start]);
        let after = &rest[(start + 2)..];
        let Some(end) = after.find("}}") else {
            out.push_str(after);
            return out;
        };
        let key = after[..end].trim();
        if let Some(v) = lookup(key) {
            out.push_str(&v);
        }
        rest = &after[(end + 2)..];
    }
}

pub fn build_track_path_by_template(
    out_dir: &Path,
    template: &str,
    artists: &[String],
    album: Option<&str>,
    track_no_1based: Option<u32>,
    title: &str,
    ext: &str,
) -> PathBuf {
    let tpl = template.trim();
    if tpl.is_empty() {
        return build_track_path(out_dir, artists, album, track_no_1based, title, ext);
    }

    let primary_artist = pick_primary_artist(artists);
    let album_value = album
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Single")
        .to_string();
    let ext_value = ext.trim().trim_start_matches('.').to_string();
    let track_no_value = track_no_1based
        .map(|n| format!("{:02}", n))
        .unwrap_or_default();

    let rendered = render_brace_template(tpl, |k| match k {
        "artist" => Some(primary_artist.clone()),
        "album" => Some(album_value.clone()),
        "title" => Some(title.to_string()),
        "ext" => Some(ext_value.clone()),
        "track_no" => Some(track_no_value.clone()),
        _ => None,
    });

    let normalized = rendered.replace('\\', "/");
    let mut parts: Vec<&str> = normalized
        .split('/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return build_track_path(out_dir, artists, album, track_no_1based, title, ext);
    }

    // Prevent traversal ('.' / '..') and sanitize each component.
    let mut p = PathBuf::from(out_dir);
    for seg in parts.drain(..) {
        let seg = if seg == "." || seg == ".." { "unknown" } else { seg };
        p.push(sanitize_component(seg));
    }
    p
}

pub fn quality_fallback_order() -> [&'static str; 4] {
    ["flac", "mp3_320", "mp3_192", "mp3_128"]
}
