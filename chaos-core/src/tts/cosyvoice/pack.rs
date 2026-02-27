use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tts::TtsError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CosyVoicePackConfig {
    pub pack_version: u32,
    pub sample_rate: u32,
    pub speech_token_size: u32,
    pub stop_token_start: u32,
    pub end_of_prompt_token_id: u32,
    pub spk_embed_dim: u32,
    pub token_mel_ratio: u32,
    /// Whether to add special tokens when encoding with tokenizer.json (HF-style).
    #[serde(default = "default_tokenizer_add_special_tokens")]
    pub tokenizer_add_special_tokens: bool,
    #[serde(default = "default_text_normalize")]
    pub text_normalize: String,
    #[serde(default)]
    pub llm: PackLlmConfig,
    /// Optional IO mapping for flow/hift ONNX models.
    #[serde(default)]
    pub flow_io: Option<PackOnnxIo>,
    #[serde(default)]
    pub hift_io: Option<PackOnnxIo>,
    #[serde(default)]
    pub files: PackFiles,
}

fn default_tokenizer_add_special_tokens() -> bool {
    true
}

fn default_text_normalize() -> String {
    "basic".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFiles {
    #[serde(default = "default_tokenizer_json")]
    pub tokenizer_json: String,
    #[serde(default = "default_spk2info_json")]
    pub spk2info_json: String,
    #[serde(default = "default_llm_prefill")]
    pub llm_prefill_onnx: String,
    #[serde(default = "default_llm_decode")]
    pub llm_decode_onnx: String,
    #[serde(default = "default_flow_infer")]
    pub flow_infer_onnx: String,
    #[serde(default = "default_hift_infer")]
    pub hift_infer_onnx: String,
}

fn default_tokenizer_json() -> String {
    "tokenizer.json".to_string()
}
fn default_spk2info_json() -> String {
    "spk2info.json".to_string()
}
fn default_llm_prefill() -> String {
    "llm_prefill.onnx".to_string()
}
fn default_llm_decode() -> String {
    "llm_decode.onnx".to_string()
}
fn default_flow_infer() -> String {
    "flow_infer.onnx".to_string()
}
fn default_hift_infer() -> String {
    "hift_infer.onnx".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackLlmConfig {
    #[serde(default = "default_min_token_text_ratio")]
    pub min_token_text_ratio: f32,
    #[serde(default = "default_max_token_text_ratio")]
    pub max_token_text_ratio: f32,
    /// Optional IO mapping for tract/candle inference.
    #[serde(default)]
    pub prefill_io: Option<PackOnnxIo>,
    #[serde(default)]
    pub decode_io: Option<PackOnnxIo>,
}

fn default_min_token_text_ratio() -> f32 {
    2.0
}
fn default_max_token_text_ratio() -> f32 {
    20.0
}

impl Default for PackLlmConfig {
    fn default() -> Self {
        Self {
            min_token_text_ratio: default_min_token_text_ratio(),
            max_token_text_ratio: default_max_token_text_ratio(),
            prefill_io: None,
            decode_io: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackOnnxIo {
    /// Input names in ONNX model order.
    pub inputs: Vec<String>,
    /// Output names in ONNX model order.
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spk2Info {
    pub embedding: Vec<f32>,
}

#[derive(Debug)]
pub struct CosyVoicePack {
    pub model_dir: PathBuf,
    pub cfg: CosyVoicePackConfig,
    pub tokenizer: tokenizers::Tokenizer,
    pub spk2info: HashMap<String, Spk2Info>,
}

impl CosyVoicePack {
    pub fn load(model_dir: impl AsRef<Path>) -> Result<Self, TtsError> {
        let model_dir = model_dir.as_ref().to_path_buf();
        let cfg = Self::load_config(&model_dir)?;

        let tokenizer_path = model_dir.join(&cfg.files.tokenizer_json);
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| TtsError::Tokenizer(format!("failed to load tokenizer: {e}")))?;

        let spk2info_path = model_dir.join(&cfg.files.spk2info_json);
        let spk2info = Self::load_spk2info(&spk2info_path, cfg.spk_embed_dim as usize)?;

        Ok(Self {
            model_dir,
            cfg,
            tokenizer,
            spk2info,
        })
    }

    pub fn load_config(model_dir: &Path) -> Result<CosyVoicePackConfig, TtsError> {
        let pack_path = model_dir.join("pack.json");
        let bytes = std::fs::read(&pack_path)?;
        let cfg: CosyVoicePackConfig = serde_json::from_slice(&bytes)?;
        if cfg.pack_version != 1 {
            return Err(TtsError::InvalidArg(format!(
                "unsupported packVersion={} (expected 1)",
                cfg.pack_version
            )));
        }
        if cfg.sample_rate == 0 {
            return Err(TtsError::InvalidArg("sampleRate must be > 0".into()));
        }
        if cfg.spk_embed_dim == 0 {
            return Err(TtsError::InvalidArg("spkEmbedDim must be > 0".into()));
        }
        Ok(cfg)
    }

    pub fn load_spk2info(
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

    pub fn path_llm_prefill(&self) -> PathBuf {
        self.model_dir.join(&self.cfg.files.llm_prefill_onnx)
    }
    pub fn path_llm_decode(&self) -> PathBuf {
        self.model_dir.join(&self.cfg.files.llm_decode_onnx)
    }
    pub fn path_flow_infer(&self) -> PathBuf {
        self.model_dir.join(&self.cfg.files.flow_infer_onnx)
    }
    pub fn path_hift_infer(&self) -> PathBuf {
        self.model_dir.join(&self.cfg.files.hift_infer_onnx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pack_config() {
        let tmp = tempfile::tempdir().unwrap();
        let pack = CosyVoicePackConfig {
            pack_version: 1,
            sample_rate: 24000,
            speech_token_size: 6561,
            stop_token_start: 6561,
            end_of_prompt_token_id: 151646,
            spk_embed_dim: 192,
            token_mel_ratio: 2,
            tokenizer_add_special_tokens: true,
            text_normalize: "basic".into(),
            llm: PackLlmConfig::default(),
            flow_io: None,
            hift_io: None,
            files: PackFiles::default(),
        };
        std::fs::write(
            tmp.path().join("pack.json"),
            serde_json::to_vec_pretty(&pack).unwrap(),
        )
        .unwrap();

        let got = CosyVoicePack::load_config(tmp.path()).unwrap();
        assert_eq!(got.sample_rate, 24000);
        assert_eq!(got.spk_embed_dim, 192);
    }

    #[test]
    fn spk2info_embedding_dim_is_validated() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("spk2info.json");
        let mut map = HashMap::<String, Spk2Info>::new();
        map.insert(
            "dream".into(),
            Spk2Info {
                embedding: vec![0.0; 191],
            },
        );
        std::fs::write(&path, serde_json::to_vec(&map).unwrap()).unwrap();
        let err = CosyVoicePack::load_spk2info(&path, 192).unwrap_err();
        assert!(matches!(err, TtsError::InvalidArg(_)));
    }
}

