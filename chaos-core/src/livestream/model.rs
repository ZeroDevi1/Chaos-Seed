use serde::{Deserialize, Serialize};

use crate::danmaku::model::Site;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveInfo {
    pub title: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub cover: Option<String>,
    pub is_living: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackHints {
    pub referer: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamVariant {
    /// Stable id for `resolve_variant`.
    pub id: String,
    /// User-facing label like "原画/蓝光/高清".
    pub label: String,
    /// BiliLive = qn, Huya = bitrate, Douyu = bit.
    pub quality: i32,
    /// Douyu specific: used for second-step resolving.
    pub rate: Option<i32>,
    /// May be absent until `resolve_variant`.
    pub url: Option<String>,
    pub backup_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveManifest {
    pub site: Site,
    /// Canonical room id (BiliLive: long id; Douyu: real room_id).
    pub room_id: String,
    pub raw_input: String,
    pub info: LiveInfo,
    pub playback: PlaybackHints,
    pub variants: Vec<StreamVariant>,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolveOptions {
    /// Default: true. Align with IINA+: if we have a resolved quality, drop variants that are
    /// higher but currently inaccessible (no url).
    pub drop_inaccessible_high_qualities: bool,
}

impl Default for ResolveOptions {
    fn default() -> Self {
        Self {
            drop_inaccessible_high_qualities: true,
        }
    }
}
