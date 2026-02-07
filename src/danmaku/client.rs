use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::danmaku::model::{
    ConnectOptions, DanmakuError, DanmakuEvent, DanmakuEventRx, DanmakuEventTx, DanmakuMethod,
    DanmakuSession, ResolvedTarget,
};

use crate::danmaku::{platforms, sites};

pub struct DanmakuClient {
    pub(crate) http: reqwest::Client,
}

impl DanmakuClient {
    pub fn new() -> Result<Self, DanmakuError> {
        let http = reqwest::Client::builder()
            .user_agent("chaos-seed/0.1")
            .build()?;
        Ok(Self { http })
    }

    pub async fn resolve(&self, input: &str) -> Result<ResolvedTarget, DanmakuError> {
        let (site, room_id) = sites::parse_target_hint(input)?;
        platforms::resolve(&self.http, site, &room_id).await
    }

    pub async fn connect(
        &self,
        input: &str,
        opt: ConnectOptions,
    ) -> Result<(DanmakuSession, DanmakuEventRx), DanmakuError> {
        let target = self.resolve(input).await?;
        self.connect_resolved(target, opt).await
    }

    pub async fn connect_resolved(
        &self,
        target: ResolvedTarget,
        opt: ConnectOptions,
    ) -> Result<(DanmakuSession, DanmakuEventRx), DanmakuError> {
        let (tx, rx) = mpsc::unbounded_channel::<DanmakuEvent>();
        let cancel = CancellationToken::new();

        let http = self.http.clone();
        let target2 = target.clone();
        let cancel2 = cancel.clone();
        let tx2: DanmakuEventTx = tx.clone();

        let task = tokio::spawn(async move {
            let res = platforms::run(target2.clone(), opt, tx2.clone(), cancel2, http).await;
            if res.is_err() {
                // Best-effort: match IINA+ semantics: empty means ok, "error" means failure.
                let _ = tx2.send(DanmakuEvent::new(
                    target2.site,
                    target2.room_id.clone(),
                    DanmakuMethod::LiveDMServer,
                    "error",
                    None,
                ));
            }
        });

        Ok((
            DanmakuSession {
                cancel,
                tasks: vec![task],
            },
            rx,
        ))
    }
}

