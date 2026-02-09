use chaos_core::livestream::model::{LiveInfo, LiveManifest, PlaybackHints, StreamVariant};

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
    pub variant_id: String,
    pub variant_label: String,
    pub url: String,
    pub backup_urls: Vec<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    pub variants: Option<Vec<LivestreamUiVariant>>,
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
    LivestreamUiManifest {
        site: man.site.as_str().to_string(),
        room_id: man.room_id,
        raw_input: man.raw_input,
        info: man.info,
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

