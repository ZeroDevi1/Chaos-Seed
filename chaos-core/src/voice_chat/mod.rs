use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::llm::{ChatMessage, ChatRequest, LlmClient, ReasoningMode};
use crate::tts::post_process::{TrimConfig, trim_output_pcm16};
use crate::tts::wav::TtsPcm16Result;
use crate::tts::{CosyVoiceEngine, TtsSftParams};

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
    tts_engine: Arc<CosyVoiceEngine>,
    cancel: CancellationToken,
) -> impl Stream<Item = Result<VoiceChunk, VoiceChatError>> {
    let (tx, rx) = mpsc::channel::<Result<VoiceChunk, VoiceChatError>>(16);

    tokio::spawn(async move {
        let r = run_voice_chat(req, llm, tts_engine, cancel.clone(), tx).await;
        if let Err(e) = r {
            // Best-effort: only send error if stream isn't canceled.
            if !cancel.is_cancelled() {
                let _ = cancel.cancel();
                // We can't reliably push into the channel if receiver is gone.
                let _ = e;
            }
        }
    });

    ReceiverStream::new(rx)
}

async fn run_voice_chat(
    mut req: VoiceChatRequest,
    llm: LlmClient,
    tts_engine: Arc<CosyVoiceEngine>,
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

    // Synthesize in blocking task, but allow cancellation via an atomic flag.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_for_block = cancel_flag.clone();

    let engine2 = tts_engine.clone();
    let params2 = req.tts.clone();
    let join = tokio::task::spawn_blocking(move || {
        engine2
            .synthesize_pcm16_with_cancel(&params2, Some(cancel_flag_for_block.as_ref()))
            .map_err(|e| VoiceChatError::Tts(e.to_string()))
    });

    let pcm_res: TtsPcm16Result = tokio::select! {
        res = join => {
            res.map_err(|e| VoiceChatError::Internal(e.to_string()))??
        }
        _ = cancel.cancelled() => {
            cancel_flag.store(true, Ordering::Relaxed);
            return Err(VoiceChatError::Canceled);
        }
    };

    if cancel.is_cancelled() {
        return Err(VoiceChatError::Canceled);
    }

    // Post-process trim.
    let trimmed = trim_output_pcm16(&pcm_res.pcm16, pcm_res.sample_rate, &req.cfg.trim)
        .map_err(|e| VoiceChatError::Tts(e.to_string()))?;

    // Chunk split.
    let chunk_samples = ((pcm_res.sample_rate as u64) * (req.cfg.chunk_ms as u64) / 1000).max(1) as usize;
    let mut seq = 0u64;
    for (i, chunk) in trimmed.chunks(chunk_samples).enumerate() {
        if cancel.is_cancelled() {
            return Err(VoiceChatError::Canceled);
        }
        let is_last = i == (trimmed.len() + chunk_samples - 1) / chunk_samples - 1;
        let msg = VoiceChunk {
            seq,
            pcm16: chunk.to_vec(),
            is_last,
        };
        seq += 1;
        if tx.send(Ok(msg)).await.is_err() {
            // Client went away.
            return Ok(());
        }
    }

    Ok(())
}
