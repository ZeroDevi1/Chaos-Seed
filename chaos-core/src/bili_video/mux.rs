use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::process::Command;

use super::BiliError;

pub async fn mux_ffmpeg(
    ffmpeg_path: &str,
    video_path: &Path,
    audio_path: &Path,
    subtitles: &[PathBuf],
    out_path: &Path,
    overwrite: bool,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<(), BiliError> {
    let bin = ffmpeg_path.trim();
    if bin.is_empty() {
        return Err(BiliError::InvalidInput("ffmpegPath is empty".to_string()));
    }
    if video_path.as_os_str().is_empty() || audio_path.as_os_str().is_empty() || out_path.as_os_str().is_empty() {
        return Err(BiliError::InvalidInput("empty input/output path".to_string()));
    }

    if let Some(parent) = out_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut cmd = Command::new(bin);
    cmd.arg("-hide_banner").arg("-loglevel").arg("error");
    cmd.arg(if overwrite { "-y" } else { "-n" });
    cmd.arg("-i").arg(video_path);
    cmd.arg("-i").arg(audio_path);
    for s in subtitles {
        cmd.arg("-i").arg(s);
    }
    cmd.arg("-map").arg("0:v:0");
    cmd.arg("-map").arg("1:a:0");
    for i in 0..subtitles.len() {
        cmd.arg("-map").arg(format!("{}:0", i + 2));
    }

    cmd.arg("-c").arg("copy");
    if !subtitles.is_empty() {
        cmd.arg("-c:s").arg("mov_text");
    }
    cmd.arg(out_path);
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| BiliError::Mux(e.to_string()))?;

    if let Some(c) = cancel {
        tokio::select! {
            r = child.wait() => {
                let st = r.map_err(|e| BiliError::Mux(e.to_string()))?;
                if st.success() { Ok(()) } else { Err(BiliError::Mux(format!("ffmpeg exit code: {st}"))) }
            }
            _ = async {
                while !c.load(Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                }
            } => {
                let _ = child.kill().await;
                Err(BiliError::Io("canceled".to_string()))
            }
        }
    } else {
        let st = child.wait().await.map_err(|e| BiliError::Mux(e.to_string()))?;
        if st.success() {
            Ok(())
        } else {
            Err(BiliError::Mux(format!("ffmpeg exit code: {st}")))
        }
    }
}

