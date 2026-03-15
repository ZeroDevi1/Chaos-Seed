use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::llm::{ChatMessage, ChatRequest, LlmClient, ReasoningMode};
use crate::tts::post_process::TrimConfig;
use crate::tts::wav::{TtsPcm16Result, decode_wav_bytes_to_pcm16_mono};
use crate::tts::{TtsError, TtsSftParams};

#[derive(Debug, Clone)]
pub struct VoiceChatConfig {
    pub chunk_ms: u32,
    pub max_text_len: usize,
    pub trim: TrimConfig,
}

impl Default for VoiceChatConfig {
    fn default() -> Self {
        Self {
            chunk_ms: 100,
            max_text_len: 4_096,
            trim: TrimConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VoiceChatRequest {
    pub messages: Vec<ChatMessage>,
    pub reasoning_mode: ReasoningMode,
    pub spk_id: String,
    pub model_dir: String,
    pub tts: TtsSftParams,

    /// Python(.pt) 推理所需 ckpt（路径可为绝对，或相对于 python_workdir）。
    pub llm_ckpt: String,
    pub flow_ckpt: String,

    /// （可选）python workdir / script；为空则使用 python runner 内部的 env 兜底（CHAOS_TTS_PY_WORKDIR/CHAOS_TTS_PY_INFER_SFT）。
    pub python_workdir: Option<String>,
    pub python_infer_script: Option<String>,

    pub cfg: VoiceChatConfig,
}

#[derive(Debug, Clone)]
pub struct VoiceChunk {
    pub seq: u64,
    pub pcm16: Vec<i16>,
    pub is_last: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum VoiceChatError {
    #[error("llm error: {0}")]
    Llm(String),
    #[error("tts error: {0}")]
    Tts(String),
    #[error("canceled")]
    Canceled,
    #[error("internal error: {0}")]
    Internal(String),
}

pub fn realtime_chat_stream(
    req: VoiceChatRequest,
    llm: LlmClient,
    cancel: CancellationToken,
) -> impl Stream<Item = Result<VoiceChunk, VoiceChatError>> {
    let (tx, rx) = mpsc::channel::<Result<VoiceChunk, VoiceChatError>>(16);

    tokio::spawn(async move {
        let r = run_voice_chat(req, llm, cancel.clone(), tx).await;
        if let Err(e) = r {
            // Best-effort: only send error if stream isn't canceled.
            if !cancel.is_cancelled() {
                let _ = cancel.cancel();
                let _ = e;
            }
        }
    });

    ReceiverStream::new(rx)
}

async fn run_voice_chat(
    mut req: VoiceChatRequest,
    llm: LlmClient,
    cancel: CancellationToken,
    tx: mpsc::Sender<Result<VoiceChunk, VoiceChatError>>,
) -> Result<(), VoiceChatError> {
    if cancel.is_cancelled() {
        return Err(VoiceChatError::Canceled);
    }
    if req.cfg.chunk_ms == 0 {
        return Err(VoiceChatError::Internal("chunk_ms must be > 0".into()));
    }
    if req.spk_id.trim().is_empty() {
        return Err(VoiceChatError::Internal("spk_id is empty".into()));
    }
    if req.llm_ckpt.trim().is_empty() {
        return Err(VoiceChatError::Internal("llm_ckpt is empty".into()));
    }
    if req.flow_ckpt.trim().is_empty() {
        return Err(VoiceChatError::Internal("flow_ckpt is empty".into()));
    }

    let llm_req = ChatRequest {
        system: None,
        messages: req.messages.clone(),
        reasoning_mode: req.reasoning_mode,
        temperature: None,
        max_tokens: None,
    };
    let llm_resp = llm
        .chat(llm_req)
        .await
        .map_err(|e| VoiceChatError::Llm(e.to_string()))?;

    let mut text = llm_resp.text.trim().to_string();
    if text.len() > req.cfg.max_text_len {
        text.truncate(req.cfg.max_text_len);
    }

    // Prepare TTS params.
    req.tts.text = text;
    req.tts.spk_id = req.spk_id.clone();
    req.tts.model_dir = req.model_dir.clone();

    // 运行 python 推理（阻塞），同时在 async 侧监控 piece_*.wav，实现真正“边生成边播放”。
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_for_block = cancel_flag.clone();

    let params2 = req.tts.clone();
    let llm_ckpt2 = req.llm_ckpt.clone();
    let flow_ckpt2 = req.flow_ckpt.clone();
    let py_workdir2 = req.python_workdir.clone();
    let py_script2 = req.python_infer_script.clone();

    let out_dir = std::env::temp_dir().join(format!("chaos_voice_chat_{}", fastrand::u64(..)));
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| VoiceChatError::Internal(format!("create out_dir failed: {e}")))?;

    let out_dir2 = out_dir.clone();
    let join = tokio::task::spawn_blocking(move || -> Result<(), VoiceChatError> {
        crate::tts::python_runner::run_infer_sft_pt_to_out_dir_with_cancel(
            &params2,
            &llm_ckpt2,
            &flow_ckpt2,
            py_workdir2.as_deref(),
            py_script2.as_deref(),
            &out_dir2,
            true,
            Some(cancel_flag_for_block.as_ref()),
        )
        .map_err(|e| VoiceChatError::Tts(e.to_string()))
    });

    // piece watcher with “留一块缓冲”策略：保证最后一块能正确标记 is_last。
    let mut next_piece: u32 = 0;
    let mut pending: Option<TtsPcm16Result> = None;
    let mut seq: u64 = 0;
    let mut saw_any_piece = false;

    loop {
        if cancel.is_cancelled() {
            cancel_flag.store(true, Ordering::Relaxed);
            let _ = std::fs::remove_dir_all(&out_dir);
            return Err(VoiceChatError::Canceled);
        }

        let piece_path = out_dir.join(format!("piece_{next_piece:04}.wav"));
        match tokio::fs::read(&piece_path).await {
            Ok(bytes) => {
                // 文件可能还在写入中：解码失败时短暂等待重试。
                let decoded = match tokio::task::spawn_blocking(move || {
                    decode_wav_bytes_to_pcm16_mono(&bytes)
                })
                .await
                {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => {
                        if !join.is_finished() {
                            tokio::time::sleep(Duration::from_millis(30)).await;
                            continue;
                        }
                        return Err(VoiceChatError::Tts(e.to_string()));
                    }
                    Err(e) => return Err(VoiceChatError::Internal(e.to_string())),
                };

                saw_any_piece = true;

                if let Some(prev) = pending.take() {
                    // 发送 prev（非最后）。
                    let chunk_samples = ((prev.sample_rate as u64) * (req.cfg.chunk_ms as u64)
                        / 1000)
                        .max(1) as usize;
                    for chunk in prev.pcm16.chunks(chunk_samples) {
                        if tx
                            .send(Ok(VoiceChunk {
                                seq,
                                pcm16: chunk.to_vec(),
                                is_last: false,
                            }))
                            .await
                            .is_err()
                        {
                            let _ = std::fs::remove_dir_all(&out_dir);
                            return Ok(());
                        }
                        seq += 1;
                    }
                }

                pending = Some(decoded);
                next_piece = next_piece.saturating_add(1);
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // no-op
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&out_dir);
                return Err(VoiceChatError::Internal(format!(
                    "read piece wav failed: {}",
                    e
                )));
            }
        }

        if join.is_finished() {
            break;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // 等待 python 推理结束（成功/失败/取消）。
    let infer_res = tokio::select! {
        res = join => res.map_err(|e| VoiceChatError::Internal(e.to_string()))?,
        _ = cancel.cancelled() => {
            cancel_flag.store(true, Ordering::Relaxed);
            let _ = std::fs::remove_dir_all(&out_dir);
            return Err(VoiceChatError::Canceled);
        }
    };
    if let Err(e) = infer_res {
        let _ = std::fs::remove_dir_all(&out_dir);
        return Err(e);
    }

    // 发送最后一块（pending）。
    if let Some(last) = pending.take() {
        let chunk_samples =
            ((last.sample_rate as u64) * (req.cfg.chunk_ms as u64) / 1000).max(1) as usize;
        let total_chunks = (last.pcm16.len() + chunk_samples - 1) / chunk_samples;
        for (i, chunk) in last.pcm16.chunks(chunk_samples).enumerate() {
            let is_last = i + 1 == total_chunks;
            if tx
                .send(Ok(VoiceChunk {
                    seq,
                    pcm16: chunk.to_vec(),
                    is_last,
                }))
                .await
                .is_err()
            {
                let _ = std::fs::remove_dir_all(&out_dir);
                return Ok(());
            }
            seq += 1;
        }
    } else if !saw_any_piece {
        // 极端兜底：若脚本未写出 piece_*.wav，则尝试读取 chunk_0000.wav 作为整段输出。
        let wav_path = out_dir.join("chunk_0000.wav");
        if let Ok(bytes) = tokio::fs::read(&wav_path).await {
            if let Ok(decoded) = decode_wav_bytes_to_pcm16_mono(&bytes) {
                let chunk_samples = ((decoded.sample_rate as u64) * (req.cfg.chunk_ms as u64)
                    / 1000)
                    .max(1) as usize;
                let total_chunks = (decoded.pcm16.len() + chunk_samples - 1) / chunk_samples;
                for (i, chunk) in decoded.pcm16.chunks(chunk_samples).enumerate() {
                    let is_last = i + 1 == total_chunks;
                    if tx
                        .send(Ok(VoiceChunk {
                            seq,
                            pcm16: chunk.to_vec(),
                            is_last,
                        }))
                        .await
                        .is_err()
                    {
                        let _ = std::fs::remove_dir_all(&out_dir);
                        return Ok(());
                    }
                    seq += 1;
                }
            } else {
                warn!("voice_chat: failed to decode chunk_0000.wav fallback");
            }
        }
    }

    // best-effort cleanup
    let _ = std::fs::remove_dir_all(&out_dir);

    Ok(())
}

impl From<TtsError> for VoiceChatError {
    fn from(e: TtsError) -> Self {
        VoiceChatError::Tts(e.to_string())
    }
}
