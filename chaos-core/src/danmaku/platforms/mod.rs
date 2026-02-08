use tokio_util::sync::CancellationToken;

use crate::danmaku::model::{ConnectOptions, DanmakuError, DanmakuEventTx, ResolvedTarget, Site};

mod bili_live;
mod douyu;
mod huya;
mod huya_jce;

pub async fn resolve(
    http: &reqwest::Client,
    site: Site,
    room_id: &str,
) -> Result<ResolvedTarget, DanmakuError> {
    match site {
        Site::BiliLive => bili_live::resolve(http, room_id).await,
        Site::Douyu => douyu::resolve(http, room_id).await,
        Site::Huya => huya::resolve(http, room_id).await,
    }
}

pub async fn run(
    target: ResolvedTarget,
    opt: ConnectOptions,
    tx: DanmakuEventTx,
    cancel: CancellationToken,
    http: reqwest::Client,
) -> Result<(), DanmakuError> {
    match target.site {
        Site::BiliLive => bili_live::run(target, opt, tx, cancel).await,
        Site::Douyu => douyu::run(target, opt, tx, cancel, http).await,
        Site::Huya => huya::run(target, opt, tx, cancel).await,
    }
}
