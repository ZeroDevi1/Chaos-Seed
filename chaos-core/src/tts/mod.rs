//! Text-to-speech (CosyVoice3 pack) + post-processing + VAD utilities.
//!
//! Design goals:
//! - Core logic lives in `chaos-core` (single source of truth).
//! - Prefer pure-Rust inference backends (planned: candle-onnx). Where some models/op-sets are
//!   not supported, the caller can decide whether to enable a fallback backend.

pub mod cosyvoice;
pub mod post_process;
pub mod sampling;
pub mod text;
pub mod vad;
pub mod wav;

pub use cosyvoice::{CosyVoiceEngine, CosyVoicePack, CosyVoicePackConfig, Spk2Info, TtsSftParams};
pub use post_process::{TrimConfig, trim_output_pcm16};
pub use sampling::{SamplingConfig, sample_ras_next};
pub use text::{END_OF_PROMPT, PromptStrategy, ResolvedTtsText, resolve_tts_text_basic};
pub use vad::{VadConfig, VadEngine, VadSegment, VadError};
pub use wav::{TtsPcm16Result, TtsWavResult};

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("invalid argument: {0}")]
    InvalidArg(String),
    #[error("canceled")]
    Canceled,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tokenizer error: {0}")]
    Tokenizer(String),
    #[error("onnx error: {0}")]
    Onnx(String),
    #[error("vad error: {0}")]
    Vad(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

