use crate::danmaku::model::{DanmakuError, Site};

pub fn parse_target_hint(input: &str) -> Result<(Site, String), DanmakuError> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(DanmakuError::InvalidInput("empty input".to_string()));
    }

    // Explicit prefix: "bilibili:xxx", "douyu:xxx", "huya:xxx"
    if let Some((pfx, rest)) = raw.split_once(':') {
        let pfx = pfx.trim().to_ascii_lowercase();
        let rest = rest.trim();
        let site = match pfx.as_str() {
            "bilibili" | "bili" | "bl" => Some(Site::BiliLive),
            "douyu" | "dy" => Some(Site::Douyu),
            "huya" | "hy" => Some(Site::Huya),
            _ => None,
        };
        if let Some(site) = site {
            if rest.is_empty() {
                return Err(DanmakuError::InvalidInput("empty room id".to_string()));
            }
            // Allow `site:https://...` for convenience.
            if rest.starts_with("http://") || rest.starts_with("https://") {
                let url = url::Url::parse(rest)?;
                let path = url.path().trim_matches('/');
                let first_seg = path.split('/').next().unwrap_or("").trim();
                if !first_seg.is_empty() {
                    return Ok((site, first_seg.to_string()));
                }
            }
            return Ok((site, rest.to_string()));
        }
    }

    // URL input.
    if raw.starts_with("http://") || raw.starts_with("https://") {
        let url = url::Url::parse(raw)?;
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        let path = url.path().trim_matches('/');
        let first_seg = path.split('/').next().unwrap_or("").trim();
        if first_seg.is_empty() {
            return Err(DanmakuError::InvalidInput(format!(
                "missing room id in url: {raw}"
            )));
        }

        if host.ends_with("live.bilibili.com") {
            return Ok((Site::BiliLive, first_seg.to_string()));
        }
        if host.ends_with("douyu.com") {
            // Avoid known non-room routes.
            if first_seg.eq_ignore_ascii_case("topic") {
                return Err(DanmakuError::InvalidInput(format!(
                    "unsupported douyu url path: {raw}"
                )));
            }
            return Ok((Site::Douyu, first_seg.to_string()));
        }
        if host.ends_with("huya.com") {
            return Ok((Site::Huya, first_seg.to_string()));
        }

        return Err(DanmakuError::InvalidInput(format!(
            "unsupported url host: {host}"
        )));
    }

    // Room id without platform: prefer BiliLive for numeric ids.
    if raw.chars().all(|c| c.is_ascii_digit()) {
        return Ok((Site::BiliLive, raw.to_string()));
    }

    Err(DanmakuError::AmbiguousInput {
        input: raw.to_string(),
    })
}
