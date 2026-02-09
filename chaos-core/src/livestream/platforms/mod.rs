use crate::danmaku::model::Site;

use super::client::{LivestreamConfig, LivestreamError};
use super::model::{LiveManifest, ResolveOptions, StreamVariant};

mod bili_live;
mod douyu;
mod huya;

pub async fn decode_manifest(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    site: Site,
    room_id: &str,
    raw_input: &str,
    opt: ResolveOptions,
) -> Result<LiveManifest, LivestreamError> {
    match site {
        Site::BiliLive => bili_live::decode_manifest(http, cfg, room_id, raw_input, opt).await,
        Site::Douyu => douyu::decode_manifest(http, cfg, room_id, raw_input, opt).await,
        Site::Huya => huya::decode_manifest(http, cfg, room_id, raw_input, opt).await,
    }
}

pub async fn resolve_variant(
    http: &reqwest::Client,
    cfg: &LivestreamConfig,
    site: Site,
    room_id: &str,
    variant_id: &str,
) -> Result<StreamVariant, LivestreamError> {
    match site {
        Site::BiliLive => bili_live::resolve_variant(http, cfg, room_id, variant_id).await,
        Site::Douyu => douyu::resolve_variant(http, cfg, room_id, variant_id).await,
        Site::Huya => huya::resolve_variant(http, cfg, room_id, variant_id).await,
    }
}
