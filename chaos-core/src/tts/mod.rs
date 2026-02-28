//! Text-to-speech (CosyVoice3 pack) + post-processing + VAD utilities.
//!
//! Design goals:
//! - Core logic lives in `chaos-core` (single source of truth).
//! - Prefer pure-Rust inference backends (planned: candle-onnx). Where some models/op-sets are
//!   not supported, the caller can decide whether to enable a fallback backend.

pub mod cosyvoice;
#[cfg(feature = "cosyvoice3-candle")]
pub mod cosyvoice3_candle;
pub mod post_process;
pub mod sampling;
pub mod text;
pub mod vad;
pub mod wav;

pub use cosyvoice::{CosyVoiceEngine, CosyVoicePack, CosyVoicePackConfig, Spk2Info, TtsSftParams};
#[cfg(feature = "cosyvoice3-candle")]
pub use cosyvoice3_candle::{CosyVoice3CandleEngine, CosyVoice3CandleParams, CosyVoice3Mode, CosyVoice3PromptFeatures, CosyVoice3WavDebugResult};
pub use post_process::{TrimConfig, trim_output_pcm16, trim_output_pcm16_with_engine};
pub use sampling::{SamplingConfig, sample_ras_next};
pub use text::{
    END_OF_PROMPT, PromptStrategy, ResolvedTtsText, compute_guide_prefix_ratio_tokens,
    resolve_tts_text_basic,
};
#[cfg(feature = "silero-vad")]
pub use vad::SileroVad;
pub use vad::{EnergyVad, VadConfig, VadEngine, VadError, VadSegment};
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
    #[error("candle error: {0}")]
    Candle(String),
    #[error("vad error: {0}")]
    Vad(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}
