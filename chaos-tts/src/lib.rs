//! Pure-Rust CosyVoice pack loader + sampling + WAV encoding.
//!
//! Notes:
//! - This crate is intentionally runtime-only: it assumes the CosyVoice model has been exported
//!   into an ONNX pack (see `pack.json` contract) by an offline tool.

mod engine;
mod pack;
mod sampling;
mod text;
mod wav;

pub use engine::{CosyVoiceEngine, TtsAudioResult, TtsJobStage, TtsSftParams};
pub use pack::{CosyVoicePack, CosyVoicePackConfig, Spk2Info};
pub use sampling::{SamplingConfig, sample_ras_next};
pub use text::{PromptStrategy, ResolvedTtsText, resolve_tts_text_basic};

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("invalid argument: {0}")]
    InvalidArg(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tokenizer error: {0}")]
    Tokenizer(String),
    #[error("onnx error: {0}")]
    Onnx(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

