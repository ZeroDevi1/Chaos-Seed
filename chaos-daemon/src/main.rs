#[cfg(windows)]
mod win {
    use chaos_daemon::run_jsonrpc_over_lsp;
    use chaos_app::ChaosApp;
    use chaos_core::{lyrics, now_playing};
    use chaos_proto::{
        DanmakuFetchImageParams, LiveCloseParams, LiveOpenParams, LivestreamDecodeManifestParams,
        LivestreamDecodeManifestResult, LyricsSearchParams, LyricsSearchResult, NowPlayingSession,
        NowPlayingSnapshot, NowPlayingSnapshotParams, NowPlayingThumbnail, PreferredQuality,
    };
    use std::env;
    use std::str::FromStr;

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

        async fn now_playing_snapshot(
            &self,
            params: NowPlayingSnapshotParams,
        ) -> Result<NowPlayingSnapshot, String> {
            let include_thumbnail = params.include_thumbnail.unwrap_or(false);
            let max_thumbnail_bytes = params
                .max_thumbnail_bytes
                .unwrap_or(262_144)
                .clamp(1, 2_500_000) as usize;
            let max_sessions = params.max_sessions.unwrap_or(32).clamp(1, 128) as usize;

            let snap = tokio::task::spawn_blocking(move || {
                now_playing::snapshot(now_playing::NowPlayingOptions {
                    include_thumbnail,
                    max_thumbnail_bytes,
                    max_sessions,
                })
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            fn map_thumb(t: &now_playing::NowPlayingThumbnail) -> NowPlayingThumbnail {
                NowPlayingThumbnail {
                    mime: t.mime.clone(),
                    base64: t.base64.clone(),
                }
            }

            fn map_session(s: &now_playing::NowPlayingSession) -> NowPlayingSession {
                NowPlayingSession {
                    app_id: s.app_id.clone(),
                    is_current: s.is_current,
                    playback_status: s.playback_status.clone(),
                    title: s.title.clone(),
                    artist: s.artist.clone(),
                    album_title: s.album_title.clone(),
                    position_ms: s.position_ms,
                    duration_ms: s.duration_ms,
                    genres: s.genres.clone(),
                    song_id: s.song_id.clone(),
                    thumbnail: s.thumbnail.as_ref().map(map_thumb),
                    error: s.error.clone(),
                }
            }

            Ok(NowPlayingSnapshot {
                supported: snap.supported,
                now_playing: snap.now_playing.as_ref().map(map_session),
                sessions: snap.sessions.iter().map(map_session).collect(),
                picked_app_id: snap.picked_app_id.clone(),
                retrieved_at_unix_ms: snap.retrieved_at_unix_ms,
            })
        }

        async fn lyrics_search(
            &self,
            params: LyricsSearchParams,
        ) -> Result<Vec<LyricsSearchResult>, String> {
            let title = params.title.trim().to_string();
            if title.is_empty() {
                return Err("title is empty".to_string());
            }

            let artist = params
                .artist
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();
            let album = params
                .album
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();

            let term = if artist.is_empty() {
                lyrics::model::LyricsSearchTerm::Keyword { keyword: title }
            } else {
                lyrics::model::LyricsSearchTerm::Info {
                    title,
                    artist,
                    album: (!album.is_empty()).then_some(album),
                }
            };

            let mut req = lyrics::model::LyricsSearchRequest::new(term);
            req.duration_ms = params.duration_ms.filter(|v| *v > 0);
            if let Some(limit) = params.limit {
                req.limit = (limit as usize).clamp(1, 50);
            }

            let mut opt = lyrics::model::LyricsSearchOptions::default();
            if let Some(v) = params.timeout_ms {
                opt.timeout_ms = v.max(1);
            }
            if let Some(v) = params.strict_match {
                opt.strict_match = v;
            }

            if let Some(services) = params.services {
                let mut out = Vec::new();
                for s in services {
                    let s = s.trim().to_string();
                    if s.is_empty() {
                        continue;
                    }
                    let svc = lyrics::model::LyricsService::from_str(&s).map_err(|e| e)?;
                    out.push(svc);
                }
                if !out.is_empty() {
                    opt.services = out;
                }
            }

            let items = lyrics::core::search(&req, opt).await.map_err(|e| e.to_string())?;
            Ok(items
                .into_iter()
                .map(|x| LyricsSearchResult {
                    service: x.service.as_str().to_string(),
                    service_token: x.service_token,
                    title: x.title,
                    artist: x.artist,
                    album: x.album,
                    duration_ms: x.duration_ms,
                    match_percentage: x.match_percentage,
                    quality: x.quality,
                    matched: x.matched,
                    has_translation: x.has_translation,
                    has_inline_timetags: x.has_inline_timetags,
                    lyrics_original: x.lyrics_original,
                    lyrics_translation: x.lyrics_translation,
                    debug: x.debug,
                })
                .collect())
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
