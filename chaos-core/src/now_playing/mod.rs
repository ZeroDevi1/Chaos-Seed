use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NowPlayingOptions {
    pub include_thumbnail: bool,
    pub max_thumbnail_bytes: usize,
    pub max_sessions: usize,
}

impl Default for NowPlayingOptions {
    fn default() -> Self {
        Self {
            include_thumbnail: true,
            max_thumbnail_bytes: 262_144,
            max_sessions: 32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NowPlayingThumbnail {
    pub mime: String,
    pub base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NowPlayingSession {
    pub app_id: String,
    pub is_current: bool,
    pub playback_status: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_title: Option<String>,
    pub position_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song_id: Option<String>,
    pub thumbnail: Option<NowPlayingThumbnail>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NowPlayingSnapshot {
    pub supported: bool,
    pub now_playing: Option<NowPlayingSession>,
    pub sessions: Vec<NowPlayingSession>,
    pub picked_app_id: Option<String>,
    pub retrieved_at_unix_ms: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum NowPlayingError {
    #[error("now playing is not supported on this platform")]
    Unsupported,

    #[cfg(windows)]
    #[error("windows api error: {0}")]
    Windows(#[from] windows::core::Error),

    #[error("{0}")]
    Other(String),
}

fn unix_ms_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(windows)]
fn playback_status_to_string(s: &str) -> String {
    // Normalize to a stable set of strings for cross-language callers.
    match s {
        "Playing" | "Paused" | "Stopped" | "Closed" | "Changing" | "Unknown" => s.to_string(),
        other => other.to_string(),
    }
}

#[cfg(any(test, windows))]
fn pick_now_playing_index(sessions: &[NowPlayingSession]) -> Option<usize> {
    // 1) Prefer the first "Playing" session
    if let Some(i) = sessions
        .iter()
        .position(|s| s.playback_status.eq_ignore_ascii_case("Playing"))
    {
        return Some(i);
    }
    // 2) Fall back to the current session if any
    sessions.iter().position(|s| s.is_current)
}

#[cfg(any(test, windows))]
fn sniff_mime(bytes: &[u8]) -> &'static str {
    // PNG signature: 89 50 4E 47 0D 0A 1A 0A
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() >= PNG.len() && &bytes[..PNG.len()] == PNG {
        return "image/png";
    }
    // JPEG signature: FF D8 FF
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return "image/jpeg";
    }
    // GIF signature: GIF87a / GIF89a
    if bytes.len() >= 6 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a") {
        return "image/gif";
    }
    // WEBP signature: RIFF....WEBP
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    // BMP signature: 42 4D
    if bytes.len() >= 2 && bytes[0] == 0x42 && bytes[1] == 0x4D {
        return "image/bmp";
    }
    // ICO signature: 00 00 01 00
    if bytes.len() >= 4
        && bytes[0] == 0x00
        && bytes[1] == 0x00
        && bytes[2] == 0x01
        && bytes[3] == 0x00
    {
        return "image/x-icon";
    }
    "application/octet-stream"
}

#[cfg(not(windows))]
pub fn snapshot(_opt: NowPlayingOptions) -> Result<NowPlayingSnapshot, NowPlayingError> {
    Ok(NowPlayingSnapshot {
        supported: false,
        now_playing: None,
        sessions: Vec::new(),
        picked_app_id: None,
        retrieved_at_unix_ms: unix_ms_now(),
    })
}

#[cfg(windows)]
pub fn snapshot(opt: NowPlayingOptions) -> Result<NowPlayingSnapshot, NowPlayingError> {
    use base64::Engine as _;
    use windows::Foundation::TimeSpan;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };
    use windows::Storage::Streams::{DataReader, IInputStream, IRandomAccessStreamReference};

    fn timespan_to_ms(ts: TimeSpan) -> Option<u64> {
        // TimeSpan.Duration is i64 in 100ns units.
        let d = ts.Duration;
        if d <= 0 {
            return None;
        }
        Some((d as u64) / 10_000)
    }

    fn status_to_str(s: GlobalSystemMediaTransportControlsSessionPlaybackStatus) -> &'static str {
        match s {
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => "Playing",
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => "Paused",
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => "Stopped",
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed => "Closed",
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing => "Changing",
            _ => "Unknown",
        }
    }

    fn read_thumbnail_bytes(
        thumb: &IRandomAccessStreamReference,
        max_bytes: usize,
    ) -> Result<Vec<u8>, windows::core::Error> {
        // Blocking `.join()` keeps the call stack synchronous, which tends to be more robust for WinRT usage
        // across different host environments.
        let stream = thumb.OpenReadAsync()?.join()?;
        let input: IInputStream = stream.GetInputStreamAt(0)?;
        let reader = DataReader::CreateDataReader(&input)?;

        // Do not rely on `stream.Size()`:
        // - in practice, some GSMTC thumbnail streams report `Size=0` even though data is readable.
        // - using `Size` would cause us to incorrectly return an empty thumbnail.
        const CHUNK: usize = 128 * 1024;
        let mut out: Vec<u8> = Vec::new();
        let size_u64 = stream.Size().unwrap_or(0);
        if size_u64 > 0 {
            out.reserve((size_u64 as usize).min(max_bytes));
        }
        let mut total: usize = 0;
        while total < max_bytes {
            let want = (max_bytes - total).min(CHUNK);
            let loaded = reader.LoadAsync(want as u32)?.join()?;
            if loaded == 0 {
                break;
            }

            let mut buf = vec![0u8; loaded as usize];
            reader.ReadBytes(&mut buf)?;
            out.extend_from_slice(&buf);
            total = out.len();
        }
        Ok(out)
    }

    let max_sessions = opt.max_sessions.max(1).min(128);
    let max_thumb = opt.max_thumbnail_bytes.max(1).min(2_500_000);

    let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
    let cur: Option<GlobalSystemMediaTransportControlsSession> = mgr.GetCurrentSession().ok();
    let cur_app_id = cur
        .as_ref()
        .and_then(|s| s.SourceAppUserModelId().ok())
        .map(|s| s.to_string());

    let mut out_sessions: Vec<NowPlayingSession> = Vec::new();
    // For performance and stability, only fetch the thumbnail bytes for the picked session.
    // (Fetching thumbnails for every session can produce huge base64 payloads and freeze callers.)
    let mut thumb_refs: Vec<Option<IRandomAccessStreamReference>> = Vec::new();
    let sessions = mgr.GetSessions()?;
    for (idx, s) in sessions.into_iter().enumerate() {
        if idx >= max_sessions {
            break;
        }

        let mut thumb_ref: Option<IRandomAccessStreamReference> = None;
        let app_id = s
            .SourceAppUserModelId()
            .map(|v| v.to_string())
            .unwrap_or_default();
        let is_current = cur_app_id
            .as_deref()
            .map(|c| !c.is_empty() && c == app_id)
            .unwrap_or(false);

        let mut item = NowPlayingSession {
            app_id: app_id.clone(),
            is_current,
            playback_status: "Unknown".to_string(),
            title: None,
            artist: None,
            album_title: None,
            position_ms: None,
            duration_ms: None,
            genres: Vec::new(),
            song_id: None,
            thumbnail: None,
            error: None,
        };

        // Playback info / timeline are "best effort": don't fail the whole snapshot if one session errors.
        match s.GetPlaybackInfo() {
            Ok(info) => {
                if let Ok(st) = info.PlaybackStatus() {
                    item.playback_status = playback_status_to_string(status_to_str(st));
                }
            }
            Err(e) => item.error = Some(format!("playback_info: {e}")),
        }

        match s.GetTimelineProperties() {
            Ok(t) => {
                item.position_ms = timespan_to_ms(t.Position().unwrap_or(TimeSpan { Duration: 0 }));
                let start = timespan_to_ms(t.StartTime().unwrap_or(TimeSpan { Duration: 0 }));
                let end = timespan_to_ms(t.EndTime().unwrap_or(TimeSpan { Duration: 0 }));
                item.duration_ms = match (start, end) {
                    (Some(a), Some(b)) if b > a => Some(b - a),
                    _ => None,
                };
            }
            Err(e) => {
                if item.error.is_none() {
                    item.error = Some(format!("timeline: {e}"));
                }
            }
        }

        match s.TryGetMediaPropertiesAsync() {
            Ok(op) => match op.join() {
                Ok(props) => {
                    let title = props
                        .Title()
                        .ok()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let artist = props
                        .Artist()
                        .ok()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let album = props
                        .AlbumTitle()
                        .ok()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    item.title = (!title.trim().is_empty()).then_some(title);
                    item.artist = (!artist.trim().is_empty()).then_some(artist);
                    item.album_title = (!album.trim().is_empty()).then_some(album);

                    if let Ok(gs) = props.Genres() {
                        let mut genres: Vec<String> = Vec::new();
                        for g in gs {
                            let s = g.to_string();
                            if !s.trim().is_empty() {
                                genres.push(s);
                            }
                        }
                        item.song_id = genres.iter().find_map(|g| {
                            if let Some(x) = g.strip_prefix("NCM-") {
                                return Some(x.to_string());
                            }
                            if let Some(x) = g.strip_prefix("QQ-") {
                                return Some(x.to_string());
                            }
                            None
                        });
                        item.genres = genres;
                    }

                    // Store the stream reference and fetch bytes later for the picked session only.
                    if opt.include_thumbnail {
                        match props.Thumbnail() {
                            Ok(v) => {
                                thumb_ref = Some(v);
                            }
                            Err(e) => {
                                let msg = format!("thumbnail_ref: {e}");
                                item.error = Some(match item.error.take() {
                                    Some(prev) => format!("{prev}; {msg}"),
                                    None => msg,
                                });
                            }
                        }
                        if thumb_ref.is_none() && item.error.is_none() {
                            item.error = Some("thumbnail_ref: none".to_string());
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("media_properties: {e}");
                    item.error = Some(match item.error.take() {
                        Some(prev) => format!("{prev}; {msg}"),
                        None => msg,
                    });
                }
            },
            Err(e) => {
                let msg = format!("media_properties_async: {e}");
                item.error = Some(match item.error.take() {
                    Some(prev) => format!("{prev}; {msg}"),
                    None => msg,
                });
            }
        }

        out_sessions.push(item);
        thumb_refs.push(thumb_ref);
    }

    let pick_idx = pick_now_playing_index(&out_sessions);

    if opt.include_thumbnail {
        fn same_track(a: &NowPlayingSession, b: &NowPlayingSession) -> bool {
            a.title == b.title
                && a.artist == b.artist
                && a.album_title == b.album_title
                && a.duration_ms == b.duration_ms
        }

        let cur_idx = out_sessions.iter().position(|s| s.is_current);
        let mut try_indices: Vec<usize> = Vec::new();
        if let Some(i) = pick_idx {
            try_indices.push(i);
        }
        if let Some(c) = cur_idx {
            if Some(c) != pick_idx {
                try_indices.push(c);
            }
        }

        for idx in try_indices {
            let Some(Some(thumb)) = thumb_refs.get(idx) else {
                continue;
            };
            match read_thumbnail_bytes(thumb, max_thumb) {
                Ok(bytes) => {
                    if bytes.is_empty() {
                        // No bytes even though a thumbnail reference exists: record a hint for debugging.
                        if let Some(item) = out_sessions.get_mut(idx) {
                            if item.error.is_none() {
                                item.error = Some("thumbnail: empty".to_string());
                            }
                        }
                        continue;
                    }

                    let mime = sniff_mime(&bytes).to_string();
                    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                    if let Some(item) = out_sessions.get_mut(idx) {
                        item.thumbnail = Some(NowPlayingThumbnail { mime, base64: b64 });
                    }

                    // If the picked session didn't get a thumbnail, but the current session did,
                    // and they look like the same track, copy the thumbnail to the picked one
                    // so callers can still show cover art.
                    if let (Some(p), Some(c)) = (pick_idx, cur_idx) {
                        if out_sessions
                            .get(p)
                            .and_then(|s| s.thumbnail.as_ref())
                            .is_none()
                            && idx == c
                        {
                            let same = out_sessions
                                .get(p)
                                .zip(out_sessions.get(c))
                                .map(|(a, b)| same_track(a, b))
                                .unwrap_or(false);
                            if same {
                                let t = out_sessions[c].thumbnail.clone();
                                if let Some(item) = out_sessions.get_mut(p) {
                                    item.thumbnail = t;
                                }
                            }
                        }
                    }

                    // Once the picked session has a thumbnail, stop.
                    if Some(idx) == pick_idx {
                        break;
                    }
                }
                Err(e) => {
                    if let Some(item) = out_sessions.get_mut(idx) {
                        let msg = format!("thumbnail: {e}");
                        item.error = Some(match item.error.take() {
                            Some(prev) => format!("{prev}; {msg}"),
                            None => msg,
                        });
                    }
                }
            }
        }
    }

    let now_playing = pick_idx.and_then(|i| out_sessions.get(i).cloned());
    let picked_app_id = now_playing.as_ref().map(|s| s.app_id.clone());

    Ok(NowPlayingSnapshot {
        supported: true,
        now_playing,
        sessions: out_sessions,
        picked_app_id,
        retrieved_at_unix_ms: unix_ms_now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_now_playing_prefers_playing_over_current() {
        let sessions = vec![
            NowPlayingSession {
                app_id: "a".to_string(),
                is_current: true,
                playback_status: "Paused".to_string(),
                title: None,
                artist: None,
                album_title: None,
                position_ms: None,
                duration_ms: None,
                genres: Vec::new(),
                song_id: None,
                thumbnail: None,
                error: None,
            },
            NowPlayingSession {
                app_id: "b".to_string(),
                is_current: false,
                playback_status: "Playing".to_string(),
                title: None,
                artist: None,
                album_title: None,
                position_ms: None,
                duration_ms: None,
                genres: Vec::new(),
                song_id: None,
                thumbnail: None,
                error: None,
            },
        ];
        assert_eq!(pick_now_playing_index(&sessions), Some(1));
    }

    #[test]
    fn mime_sniff_png_jpeg_unknown() {
        assert_eq!(sniff_mime(b"\x89PNG\r\n\x1a\nxxxx"), "image/png");
        assert_eq!(sniff_mime(b"\xff\xd8\xff\xe0xxxx"), "image/jpeg");
        assert_eq!(sniff_mime(b"GIF89a...."), "image/gif");
        assert_eq!(sniff_mime(b"RIFF\x00\x00\x00\x00WEBP"), "image/webp");
        assert_eq!(sniff_mime(b"BM...."), "image/bmp");
        assert_eq!(sniff_mime(b"\x00\x00\x01\x00...."), "image/x-icon");
        assert_eq!(sniff_mime(b"nope"), "application/octet-stream");
        assert_eq!(sniff_mime(b""), "application/octet-stream");
    }

    #[test]
    fn thumbnail_base64_roundtrip_length_nonzero() {
        use base64::Engine as _;
        let bytes = b"\x89PNG\r\n\x1a\nfake";
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let out = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .expect("decode");
        assert!(!out.is_empty());
        assert_eq!(out, bytes);
    }

    #[test]
    fn snapshot_serializes_and_has_supported_field() {
        let s = snapshot(NowPlayingOptions {
            include_thumbnail: false,
            max_thumbnail_bytes: 64,
            max_sessions: 4,
        })
        .expect("snapshot");

        let json = serde_json::to_string(&s).expect("serialize");
        assert!(json.contains("\"supported\""));
        if cfg!(windows) {
            assert!(s.supported);
        } else {
            assert!(!s.supported);
        }
    }
}
