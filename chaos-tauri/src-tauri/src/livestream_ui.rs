use chaos_core::livestream::model::{LiveInfo, LiveManifest, PlaybackHints, StreamVariant};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LivestreamUiVariant {
    pub id: String,
    pub label: String,
    pub quality: i32,
    pub rate: Option<i32>,
    pub url: Option<String>,
    pub backup_urls: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LivestreamUiManifest {
    pub site: String,
    pub room_id: String,
    pub raw_input: String,
    pub info: LiveInfo,
    pub playback: PlaybackHints,
    pub variants: Vec<LivestreamUiVariant>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PlayerBootRequest {
    pub site: String,
    pub room_id: String,
    pub title: String,
    pub cover: Option<String>,
    pub variant_id: String,
    pub variant_label: String,
    pub url: String,
    pub backup_urls: Vec<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    pub variants: Option<Vec<LivestreamUiVariant>>,
}

fn normalize_image_url(u: Option<String>) -> Option<String> {
    let Some(raw) = u else {
        return None;
    };
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    if let Some(rest) = s.strip_prefix("http://") {
        return Some(format!("https://{rest}"));
    }
    Some(s.to_string())
}

pub fn map_variant(v: StreamVariant) -> LivestreamUiVariant {
    LivestreamUiVariant {
        id: v.id,
        label: v.label,
        quality: v.quality,
        rate: v.rate,
        url: v.url,
        backup_urls: v.backup_urls,
    }
}

pub fn map_manifest(man: LiveManifest) -> LivestreamUiManifest {
    let LiveInfo {
        title,
        name,
        avatar,
        cover,
        is_living,
    } = man.info;
    let info = LiveInfo {
        title,
        name,
        avatar: normalize_image_url(avatar),
        cover: normalize_image_url(cover),
        is_living,
    };
    LivestreamUiManifest {
        site: man.site.as_str().to_string(),
        room_id: man.room_id,
        raw_input: man.raw_input,
        info,
        playback: man.playback,
        variants: man.variants.into_iter().map(map_variant).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chaos_core::danmaku::model::Site;
    use chaos_core::livestream::model::{LiveInfo, PlaybackHints, StreamVariant};

    #[test]
    fn normalize_image_url_handles_protocol_relative() {
        assert_eq!(
            normalize_image_url(Some("//a/b.jpg".to_string())).as_deref(),
            Some("https://a/b.jpg")
        );
    }

    #[test]
    fn normalize_image_url_upgrades_http() {
        assert_eq!(
            normalize_image_url(Some("http://a/b.jpg".to_string())).as_deref(),
            Some("https://a/b.jpg")
        );
    }

    #[test]
    fn normalize_image_url_drops_empty() {
        assert_eq!(normalize_image_url(Some("".to_string())), None);
        assert_eq!(normalize_image_url(Some("   ".to_string())), None);
    }

    #[test]
    fn site_maps_to_string() {
        let man = LiveManifest {
            site: Site::BiliLive,
            room_id: "1".to_string(),
            raw_input: "x".to_string(),
            info: LiveInfo {
                title: "t".to_string(),
                name: None,
                avatar: None,
                cover: None,
                is_living: true,
            },
            playback: PlaybackHints {
                referer: Some("r".to_string()),
                user_agent: None,
            },
            variants: vec![],
        };
        let ui = map_manifest(man);
        assert_eq!(ui.site, "bili_live");
    }

    #[test]
    fn variant_serializes_urls() {
        let v = LivestreamUiVariant {
            id: "v".to_string(),
            label: "l".to_string(),
            quality: 1,
            rate: Some(2),
            url: Some("https://u".to_string()),
            backup_urls: vec!["https://b1".to_string(), "https://b2".to_string()],
        };
        let s = serde_json::to_string(&v).expect("json");
        assert!(s.contains("\"url\":\"https://u\""));
        assert!(s.contains("\"backup_urls\":[\"https://b1\",\"https://b2\"]"));
    }

    #[test]
    fn manifest_maps_variants() {
        let man = LiveManifest {
            site: Site::Huya,
            room_id: "123".to_string(),
            raw_input: "https://www.huya.com/123".to_string(),
            info: LiveInfo {
                title: "t".to_string(),
                name: Some("n".to_string()),
                avatar: None,
                cover: None,
                is_living: false,
            },
            playback: PlaybackHints {
                referer: Some("https://www.huya.com/".to_string()),
                user_agent: None,
            },
            variants: vec![StreamVariant {
                id: "id".to_string(),
                label: "lbl".to_string(),
                quality: 2000,
                rate: None,
                url: Some("https://x".to_string()),
                backup_urls: vec![],
            }],
        };
        let ui = map_manifest(man);
        assert_eq!(ui.variants.len(), 1);
        assert_eq!(ui.variants[0].id, "id");
        assert_eq!(ui.variants[0].url.as_deref(), Some("https://x"));
    }
}
