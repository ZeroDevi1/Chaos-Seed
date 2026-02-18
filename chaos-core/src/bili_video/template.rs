use std::path::{Path, PathBuf};

use crate::music::util::sanitize_component;

#[derive(Debug, Clone)]
pub struct TemplateVars {
    pub video_title: String,
    pub page_number: u32,
    pub page_title: String,
    pub bvid: String,
    pub aid: String,
    pub cid: String,
    pub dfn: String,
    pub res: String,
    pub fps: String,
    pub video_codecs: String,
    pub audio_codecs: String,
    pub owner_name: String,
    pub owner_mid: String,
}

fn with_zero(n: u32) -> String {
    format!("{:02}", n)
}

pub fn render_bbdown_pattern(pattern: &str, v: &TemplateVars) -> String {
    let mut s = pattern.to_string();
    let rep = |key: &str, val: &str, s: &mut String| {
        *s = s.replace(key, val);
    };

    rep("<videoTitle>", &v.video_title, &mut s);
    rep("<pageNumber>", &v.page_number.to_string(), &mut s);
    rep("<pageNumberWithZero>", &with_zero(v.page_number), &mut s);
    rep("<pageTitle>", &v.page_title, &mut s);
    rep("<bvid>", &v.bvid, &mut s);
    rep("<aid>", &v.aid, &mut s);
    rep("<cid>", &v.cid, &mut s);
    rep("<dfn>", &v.dfn, &mut s);
    rep("<res>", &v.res, &mut s);
    rep("<fps>", &v.fps, &mut s);
    rep("<videoCodecs>", &v.video_codecs, &mut s);
    rep("<audioCodecs>", &v.audio_codecs, &mut s);
    rep("<ownerName>", &v.owner_name, &mut s);
    rep("<ownerMid>", &v.owner_mid, &mut s);
    s
}

pub fn sanitize_rel_path(rel: &str) -> PathBuf {
    let normalized = rel.replace('\\', "/");
    let parts: Vec<&str> = normalized
        .split('/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let mut out = PathBuf::new();
    for seg in parts {
        if seg == "." || seg == ".." {
            out.push("unknown");
            continue;
        }
        out.push(sanitize_component(seg));
    }
    out
}

pub fn build_output_path(
    out_dir: &Path,
    file_pattern: &str,
    multi_file_pattern: &str,
    total_pages: usize,
    vars: &TemplateVars,
    ext: &str,
) -> PathBuf {
    let use_multi = total_pages > 1;
    let ptn = if use_multi { multi_file_pattern } else { file_pattern };
    let rendered = render_bbdown_pattern(ptn, vars);
    let mut rel = sanitize_rel_path(&rendered);

    // Ensure final filename has extension.
    let file_name = rel
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut name = if file_name.is_empty() {
        "video".to_string()
    } else {
        file_name
    };
    if !ext.trim().is_empty() {
        let e = ext.trim().trim_start_matches('.');
        if !e.is_empty() && !name.to_ascii_lowercase().ends_with(&format!(".{e}").to_ascii_lowercase())
        {
            name.push('.');
            name.push_str(e);
        }
    }

    // Replace last component with ensured filename.
    rel.pop();
    rel.push(sanitize_component(&name));

    out_dir.join(rel)
}

