//! Text-to-speech (CosyVoice3 SFT) + post-processing + VAD utilities.
//!
//! Design goals:
//! - Core logic lives in `chaos-core` (single source of truth).
//! - 推理默认走 PyO3(Python/.pt) 复刻（更贴近 VoiceLab 的 infer_sft.py）。ONNX 相关依赖已移除（仅保留 VAD 的 onnxruntime）。

pub mod params;
pub mod post_process;
#[cfg(feature = "tts-python")]
pub mod python_infer;
pub mod sampling;
pub mod text;
pub mod vad;
pub mod wav;

pub use params::{Spk2Info, TtsSftParams};
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
    #[error("candle error: {0}")]
    Candle(String),
    #[error("vad error: {0}")]
    Vad(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}
