//! CosyVoice3 ONNX pack loader + inference engine.
//!
//! The "pack" layout is a stable on-disk contract used by ChaosSeed runtime:
//! - `pack.json`
//! - `tokenizer.json`
//! - `spk2info.json`
//! - `llm_prefill.onnx`, `llm_decode.onnx`, `flow_infer.onnx`, `hift_infer.onnx`

mod engine;
mod pack;

pub use engine::{CosyVoiceEngine, TtsSftParams};
pub use pack::{CosyVoicePack, CosyVoicePackConfig, Spk2Info};
