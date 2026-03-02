use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::tts::{PromptStrategy, SamplingConfig, TtsError};

/// 语音合成（SFT）通用参数（被 daemon/ffi/voice_chat 共享）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsSftParams {
    pub model_dir: String,
    pub spk_id: String,
    pub text: String,
    pub prompt_text: String,
    pub prompt_strategy: PromptStrategy,
    pub guide_sep: String,
    pub speed: f32,
    pub seed: u64,
    pub sampling: SamplingConfig,
    pub text_frontend: bool,
}

/// CosyVoice3 的说话人 embedding（SFT 路线）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spk2Info {
    pub embedding: Vec<f32>,
}

/// 读取 `spk2info.json`（embedding Vec<f32>）。
pub fn load_spk2info_json(
    path: &Path,
    expected_dim: usize,
) -> Result<HashMap<String, Spk2Info>, TtsError> {
    let bytes = std::fs::read(path)?;
    let map: HashMap<String, Spk2Info> = serde_json::from_slice(&bytes)?;
    for (k, v) in &map {
        if v.embedding.len() != expected_dim {
            return Err(TtsError::InvalidArg(format!(
                "spk2info[{k}].embedding has len={}, expected {expected_dim}",
                v.embedding.len()
            )));
        }
    }
    Ok(map)
}
