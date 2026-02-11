#[cfg(windows)]
mod win {
    use chaos_daemon::run_jsonrpc_over_lsp;
    use chaos_app::ChaosApp;
    use chaos_proto::{
        DanmakuFetchImageParams, LiveCloseParams, LiveOpenParams, LivestreamDecodeManifestParams,
        LivestreamDecodeManifestResult, PreferredQuality,
    };
    use std::env;

    struct Svc {
        app: std::sync::Arc<ChaosApp>,
    }

    impl chaos_daemon::ChaosService for Svc {
        fn version(&self) -> String {
            env!("CARGO_PKG_VERSION").to_string()
        }

        async fn livestream_decode_manifest(
            &self,
            params: LivestreamDecodeManifestParams,
        ) -> Result<LivestreamDecodeManifestResult, String> {
            self.app
                .decode_manifest(&params.input)
                .await
                .map_err(|e| e.to_string())
        }

        async fn live_open(
            &self,
            params: LiveOpenParams,
        ) -> Result<
            (
                chaos_proto::LiveOpenResult,
                tokio::sync::mpsc::UnboundedReceiver<chaos_proto::DanmakuMessage>,
            ),
            String,
        > {
            let prefer = params.preferred_quality.unwrap_or_default();
            let prefer_lowest = matches!(prefer, PreferredQuality::Lowest);
            self.app
                .open_live(&params.input, prefer_lowest, params.variant_id.as_deref())
                .await
                .map_err(|e| e.to_string())
        }

        async fn live_close(&self, params: LiveCloseParams) -> Result<(), String> {
            self.app
                .close_live(&params.session_id)
                .await
                .map_err(|e| e.to_string())
        }

        async fn danmaku_fetch_image(
            &self,
            params: DanmakuFetchImageParams,
        ) -> Result<chaos_proto::DanmakuFetchImageResult, String> {
            self.app.fetch_image(params).await.map_err(|e| e.to_string())
        }
    }

    pub async fn main() -> anyhow::Result<()> {
        let mut pipe_name: Option<String> = None;
        let mut auth_token: Option<String> = None;

        let mut args = env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "--pipe-name" => pipe_name = args.next(),
                "--auth-token" => auth_token = args.next(),
                _ => {}
            }
        }

        let pipe_name = pipe_name.ok_or_else(|| anyhow::anyhow!("missing --pipe-name"))?;
        let auth_token = auth_token.ok_or_else(|| anyhow::anyhow!("missing --auth-token"))?;

        let full_name = if pipe_name.starts_with(r"\\.\pipe\") {
            pipe_name
        } else {
            format!(r"\\.\pipe\{pipe_name}")
        };

        let server = tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(true)
            .create(full_name)?;

        server.connect().await?;

        let app = std::sync::Arc::new(ChaosApp::new().map_err(|e| anyhow::anyhow!("{e}"))?);
        let svc = Svc { app };

        run_jsonrpc_over_lsp(&svc, server, &auth_token)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(())
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("chaos-daemon is Windows-only. Build and run it on Windows.");
}

#[cfg(windows)]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    win::main().await
}
