//! Route2：CosyVoice3 Candle 推理（参考 third_party/cosyvoice3.rs）。
//!
//! 目标：
//! - 在 `chaos-core` 内提供一个“纯 Rust + Candle”的最小推理通路（LLM + Flow + HiFT）
//! - 可选使用 ONNX frontend（campplus + speech_tokenizer_v3）来从 prompt_wav 提取 prompt features
//!
//! 说明：
//! - 当前实现主要用于本地验证/对齐（live-tests），因此默认设备选 CPU。
//! - 如需 CUDA，后续可以扩展：需要 VS cl.exe + nvcc 环境（并启用 candle 的 cuda feature）。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;

use crate::tts::wav::{
    TtsWavResult, duration_ms, encode_wav_pcm16_mono, f32_to_pcm16_mono,
    notch_filter_f32_mono_inplace,
};
use crate::tts::{
    PromptStrategy, SamplingConfig, TtsError, resolve_tts_text_basic,
};

use cv3_candle_core::{DType, Device, Tensor};
use cv3_candle_nn::VarBuilder;
use cv3_candle_transformers::models::cosyvoice::{
    CausalHiFTGenerator, CausalMaskedDiffWithDiT, CosyVoice3LM, DiT,
};

#[cfg(feature = "cosyvoice3-candle-onnx")]
use cv3_candle_transformers::models::cosyvoice::CosyVoice3Frontend;

/// 与 cosyvoice3.rs 的 config.json 字段对齐（只保留推理需要的字段）。
#[derive(Clone, Debug, Deserialize)]
pub struct CosyVoice3CandleConfig {
    pub sample_rate: usize,
    pub llm_input_size: usize,
    pub llm_output_size: usize,
    pub speech_token_size: usize,
    pub spk_embed_dim: usize,
    pub token_frame_rate: usize,
    pub token_mel_ratio: usize,
    pub chunk_size: usize,
    pub pre_lookahead_len: usize,
    pub dit: CosyVoice3DiTConfig,
    pub hift: CosyVoice3HiFTConfig,
    pub qwen2: CosyVoice3Qwen2Config,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CosyVoice3DiTConfig {
    pub dim: usize,
    pub depth: usize,
    pub heads: usize,
    pub dim_head: usize,
    pub ff_mult: usize,
    pub mel_dim: usize,
    pub spk_dim: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CosyVoice3HiFTConfig {
    pub in_channels: usize,
    pub base_channels: usize,
    pub nb_harmonics: usize,
    pub upsample_rates: Vec<usize>,
    pub upsample_kernel_sizes: Vec<usize>,
    pub istft_n_fft: usize,
    pub istft_hop_len: usize,
    pub resblock_kernel_sizes: Vec<usize>,
    pub resblock_dilation_sizes: Vec<Vec<usize>>,
    pub source_resblock_kernel_sizes: Vec<usize>,
    pub source_resblock_dilation_sizes: Vec<Vec<usize>>,
    pub conv_pre_look_right: usize,
    pub nsf_alpha: f64,
    pub nsf_sigma: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CosyVoice3Qwen2Config {
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f64,
    pub vocab_size: usize,
}

impl CosyVoice3CandleConfig {
    pub fn from_model_dir(model_dir: &Path) -> Result<Self, TtsError> {
        let p = model_dir.join("config.json");
        let f = std::fs::File::open(&p).map_err(|e| TtsError::Io(e))?;
        let cfg: Self = serde_json::from_reader(f)?;
        Ok(cfg)
    }

    fn to_llm_config(&self) -> cv3_candle_transformers::models::cosyvoice::CosyVoice3LMConfig {
        cv3_candle_transformers::models::cosyvoice::CosyVoice3LMConfig {
            llm_input_size: self.llm_input_size,
            llm_output_size: self.llm_output_size,
            speech_token_size: self.speech_token_size,
            // cosyvoice3.rs 固定值：与其实现保持一致，便于对齐。
            mix_ratio: (5, 15),
            qwen2: cv3_candle_transformers::models::cosyvoice::Qwen2Config {
                hidden_size: self.qwen2.hidden_size,
                num_hidden_layers: self.qwen2.num_hidden_layers,
                num_attention_heads: self.qwen2.num_attention_heads,
                num_key_value_heads: self.qwen2.num_key_value_heads,
                intermediate_size: self.qwen2.intermediate_size,
                max_position_embeddings: 32768,
                rope_theta: self.qwen2.rope_theta,
                rms_norm_eps: self.qwen2.rms_norm_eps,
                vocab_size: self.qwen2.vocab_size,
                tie_word_embeddings: true,
            },
        }
    }

    fn to_hift_config(&self) -> cv3_candle_transformers::models::cosyvoice::HiFTConfig {
        cv3_candle_transformers::models::cosyvoice::HiFTConfig {
            in_channels: self.hift.in_channels,
            base_channels: self.hift.base_channels,
            nb_harmonics: self.hift.nb_harmonics,
            sampling_rate: self.sample_rate,
            nsf_alpha: self.hift.nsf_alpha,
            nsf_sigma: self.hift.nsf_sigma,
            upsample_rates: self.hift.upsample_rates.clone(),
            upsample_kernel_sizes: self.hift.upsample_kernel_sizes.clone(),
            istft_n_fft: self.hift.istft_n_fft,
            istft_hop_len: self.hift.istft_hop_len,
            resblock_kernel_sizes: self.hift.resblock_kernel_sizes.clone(),
            resblock_dilation_sizes: self.hift.resblock_dilation_sizes.clone(),
            source_resblock_kernel_sizes: self.hift.source_resblock_kernel_sizes.clone(),
            source_resblock_dilation_sizes: self.hift.source_resblock_dilation_sizes.clone(),
            conv_pre_look_right: self.hift.conv_pre_look_right,
        }
    }

    fn to_flow_config(&self) -> cv3_candle_transformers::models::cosyvoice::FlowConfig {
        cv3_candle_transformers::models::cosyvoice::FlowConfig {
            input_size: self.dit.mel_dim,
            output_size: self.dit.mel_dim,
            vocab_size: self.speech_token_size,
            token_mel_ratio: self.token_mel_ratio,
            pre_lookahead_len: self.pre_lookahead_len,
            dit: cv3_candle_transformers::models::cosyvoice::DiTConfig {
                dim: self.dit.dim,
                depth: self.dit.depth,
                heads: self.dit.heads,
                dim_head: self.dit.dim_head,
                ff_mult: self.dit.ff_mult,
                mel_dim: self.dit.mel_dim,
                spk_dim: self.dit.spk_dim,
                // candle-transformers 需要一个“静态 chunk size”（单位是 mel 帧）
                static_chunk_size: self.chunk_size * self.token_mel_ratio,
            },
            cfm: cv3_candle_transformers::models::cosyvoice::CFMConfig::default(),
        }
    }
}

/// 推理模式（与 cosyvoice3.rs 的行为对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CosyVoice3Mode {
    /// zero-shot：LLM 接收 prompt_text + prompt_speech_tokens
    ZeroShot,
    /// cross-lingual：LLM 不接收 prompt_text / prompt_speech_tokens（Flow 仍使用 prompt features）
    CrossLingual,
    /// instruct：LLM 接收 instruct_text（通过 prompt_text 字段传入），不接收 prompt_speech_tokens
    Instruct,
}

/// 从 prompt_wav 提取到的 prompt features（供 zero-shot/cross-lingual/instruct 使用）。
#[derive(Debug, Clone)]
pub struct CosyVoice3PromptFeatures {
    pub prompt_speech_tokens: Vec<u32>,
    pub prompt_mel: Vec<Vec<f32>>,
    pub speaker_embedding: Vec<f32>,
    pub prompt_sample_rate: u32,
}

#[derive(Debug, Clone)]
pub struct CosyVoice3CandleParams {
    pub model_dir: String,
    pub mode: CosyVoice3Mode,
    pub text: String,
    /// ZeroShot：prompt_text；Instruct：instruct_text；CrossLingual：忽略（可为空）
    pub prompt_text: String,
    /// 说话人 ID（SFT 路线）：从 `spk2info.json` 读取 embedding（flow 使用）。
    ///
    /// 说明：CosyVoice3 的 candle LLM 推理接口目前不支持注入 llm_embedding，因此此字段只影响 Flow/Vocoder 音色侧。
    /// 在 Instruct 模式下，上游 python 也会移除 llm_embedding，因此更容易对齐。
    pub spk_id: Option<String>,
    /// 可选覆盖 `spk2info.json` 路径。
    ///
    /// - 若为空：优先读 env `CHAOS_COSYVOICE3_SPK2INFO_JSON`，否则读 `{model_dir}/spk2info.json`。
    pub spk2info_json: Option<PathBuf>,
    /// 参考音频（用于提取 prompt_speech_tokens/prompt_mel/speaker_embedding）。
    ///
    /// - ZeroShot/CrossLingual/Instruct：都建议提供一段“参考声音音频”（通常 3~10 秒更稳定）
    /// - 若不提供：会走 text-only fallback（prompt features 全 0/空），仅用于快速验证链路，音色/质量通常不可控
    pub prompt_wav: Option<PathBuf>,
    /// prompt_text 的使用策略（对齐 `infer_sft.py` 的 `--prompt_strategy`）。
    pub prompt_strategy: PromptStrategy,
    /// guide_prefix 模式下连接 prompt_text 和 text 的分隔符（对齐 `--guide_sep`）。
    pub guide_sep: String,
    /// 是否启用基础文本前处理（换行/空白/标点归一化等）。
    pub text_frontend: bool,
    pub sampling: SamplingConfig,
    pub n_timesteps: usize,
    pub speed: f32,
}

/// 调试/对齐输出：wav + speech_tokens。
#[derive(Debug, Clone)]
pub struct CosyVoice3WavDebugResult {
    pub wav: TtsWavResult,
    pub speech_tokens: Vec<u32>,
}

struct Inner {
    llm: CosyVoice3LM,
    flow_decoder: CausalMaskedDiffWithDiT,
    vocoder: CausalHiFTGenerator,
    tokenizer: tokenizers::Tokenizer,
}

pub struct CosyVoice3CandleEngine {
    #[allow(dead_code)]
    model_dir: PathBuf,
    cfg: CosyVoice3CandleConfig,
    device: Device,
    dtype: DType,
    inner: Mutex<Inner>,
    #[cfg(feature = "cosyvoice3-candle-onnx")]
    frontend: Option<CosyVoice3Frontend>,
}

impl CosyVoice3CandleEngine {
    pub fn load(model_dir: impl AsRef<Path>) -> Result<Self, TtsError> {
        let model_dir = model_dir.as_ref().to_path_buf();
        let cfg = CosyVoice3CandleConfig::from_model_dir(&model_dir)?;

        // 设备选择：
        // - 默认 cpu
        // - 若编译开启了 cosyvoice3-candle-cuda，则支持 `CHAOS_COSYVOICE3_DEVICE=cuda` 或 `auto` 自动尝试 cuda
        let dev_req = std::env::var("CHAOS_COSYVOICE3_DEVICE")
            .ok()
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "cpu".to_string());

        let (device, on_gpu) = select_device(&dev_req)?;

        // 精度选择：GPU 可选 f16（CPU 不建议）。
        let use_f16 = std::env::var("CHAOS_COSYVOICE3_USE_F16")
            .ok()
            .map(|v| {
                let v = v.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
            })
            .unwrap_or(false);
        let dtype = if on_gpu && use_f16 {
            DType::F16
        } else {
            DType::F32
        };

        eprintln!(
            "[cosyvoice3-candle] device={} dtype={:?}",
            if on_gpu { "cuda" } else { "cpu" },
            dtype
        );

        // Load tokenizer：优先 tokenizer/tokenizer.json，否则用 vocab+merges 构建。
        let tokenizer = load_tokenizer(&model_dir)?;

        let llm_path = model_dir.join("llm.safetensors");
        let flow_path = model_dir.join("flow.safetensors");
        let hift_path = model_dir.join("hift.safetensors");

        let llm_vb = unsafe { VarBuilder::from_mmaped_safetensors(&[&llm_path], dtype, &device) }
            .map_err(|e| TtsError::Candle(format!("load llm safetensors failed: {e}")))?;
        let flow_vb = unsafe { VarBuilder::from_mmaped_safetensors(&[&flow_path], dtype, &device) }
            .map_err(|e| TtsError::Candle(format!("load flow safetensors failed: {e}")))?;
        let hift_vb = unsafe { VarBuilder::from_mmaped_safetensors(&[&hift_path], dtype, &device) }
            .map_err(|e| TtsError::Candle(format!("load hift safetensors failed: {e}")))?;

        let llm = CosyVoice3LM::new(&cfg.to_llm_config(), llm_vb)
            .map_err(|e| TtsError::Candle(format!("build llm failed: {e}")))?;

        let flow_cfg = cfg.to_flow_config();
        let dit = DiT::new(flow_cfg.dit.clone(), flow_vb.pp("dit"))
            .map_err(|e| TtsError::Candle(format!("build dit failed: {e}")))?;
        let flow_decoder = CausalMaskedDiffWithDiT::new(
            flow_cfg.vocab_size,
            flow_cfg.output_size,
            flow_cfg.output_size,
            cfg.spk_embed_dim,
            flow_cfg.token_mel_ratio,
            flow_cfg.pre_lookahead_len,
            dit,
            flow_cfg.cfm.clone(),
            flow_vb,
        )
        .map_err(|e| TtsError::Candle(format!("build flow decoder failed: {e}")))?;

        let vocoder = CausalHiFTGenerator::new(cfg.to_hift_config(), hift_vb)
            .map_err(|e| TtsError::Candle(format!("build vocoder failed: {e}")))?;

        #[cfg(feature = "cosyvoice3-candle-onnx")]
        let frontend = {
            // frontend 必须用 CPU（ONNX 仅用于特征提取；核心推理仍由 candle 执行）。
            CosyVoice3Frontend::load(&model_dir, &Device::Cpu).ok()
        };

        let inner = Inner {
            llm,
            flow_decoder,
            vocoder,
            tokenizer,
        };

        Ok(Self {
            model_dir,
            cfg,
            device,
            dtype,
            inner: Mutex::new(inner),
            #[cfg(feature = "cosyvoice3-candle-onnx")]
            frontend,
        })
    }

    pub fn config(&self) -> &CosyVoice3CandleConfig {
        &self.cfg
    }

    /// 从 prompt_wav 提取 prompt features（需要启用 feature `cosyvoice3-candle-onnx`）。
    #[cfg(feature = "cosyvoice3-candle-onnx")]
    pub fn extract_prompt_features(
        &self,
        prompt_wav: &Path,
    ) -> Result<CosyVoice3PromptFeatures, TtsError> {
        let debug = std::env::var_os("CHAOS_COSYVOICE3_DEBUG").is_some();

        let (audio_f32, sr) = decode_wav_to_f32_mono(prompt_wav)?;
        let frontend = self.frontend.as_ref().ok_or_else(|| {
            TtsError::Candle(
                "onnx frontend not available; ensure campplus.onnx + speech_tokenizer_v3.onnx exist in model_dir".into(),
            )
        })?;

        let n = audio_f32.len();
        let audio_tensor = Tensor::from_vec(audio_f32, n, &Device::Cpu)
            .map_err(|e| TtsError::Candle(format!("create audio tensor failed: {e}")))?;
        let (tokens, mel, embedding) = frontend
            .extract_prompt_features(&audio_tensor, sr as usize)
            .map_err(|e| TtsError::Candle(format!("extract prompt features failed: {e}")))?;

        let prompt_tokens: Vec<u32> = tokens
            .flatten_all()
            .map_err(|e| TtsError::Candle(format!("flatten tokens failed: {e}")))?
            .to_vec1::<i64>()
            .map_err(|e| TtsError::Candle(format!("tokens to vec failed: {e}")))?
            .into_iter()
            .map(|x| x as u32)
            .collect();

        let mel = mel
            .to_dtype(DType::F32)
            .map_err(|e| TtsError::Candle(format!("mel to f32 failed: {e}")))?;
        let mel_shape = mel.dims();
        let t_dim = if mel_shape.len() == 3 {
            mel_shape[1]
        } else {
            mel_shape[0]
        };
        let mel_dim = if mel_shape.len() == 3 {
            mel_shape[2]
        } else {
            mel_shape[1]
        };
        let mel_flat: Vec<f32> = mel
            .flatten_all()
            .map_err(|e| TtsError::Candle(format!("flatten mel failed: {e}")))?
            .to_vec1()
            .map_err(|e| TtsError::Candle(format!("mel to vec failed: {e}")))?;
        let prompt_mel: Vec<Vec<f32>> = mel_flat
            .chunks(mel_dim)
            .take(t_dim)
            .map(|c| c.to_vec())
            .collect();

        let speaker_embedding: Vec<f32> = embedding
            .flatten_all()
            .map_err(|e| TtsError::Candle(format!("flatten embedding failed: {e}")))?
            .to_dtype(DType::F32)
            .map_err(|e| TtsError::Candle(format!("embedding to f32 failed: {e}")))?
            .to_vec1()
            .map_err(|e| TtsError::Candle(format!("embedding to vec failed: {e}")))?;

        if debug {
            eprintln!(
                "[cosyvoice3-candle][debug] prompt_features: tokens_len={} mel_T={} mel_dim={} spk_embed_dim={} sr={}",
                prompt_tokens.len(),
                prompt_mel.len(),
                mel_dim,
                speaker_embedding.len(),
                sr
            );
        }

        Ok(CosyVoice3PromptFeatures {
            prompt_speech_tokens: prompt_tokens,
            prompt_mel,
            speaker_embedding,
            prompt_sample_rate: sr,
        })
    }

    /// 端到端推理：返回 wav + speech_tokens（便于对齐）。
    pub fn synthesize_wav_bytes_debug(
        &self,
        params: &CosyVoice3CandleParams,
    ) -> Result<CosyVoice3WavDebugResult, TtsError> {
        if params.speed <= 0.0 {
            return Err(TtsError::InvalidArg("speed must be > 0".into()));
        }
        if params.n_timesteps == 0 {
            return Err(TtsError::InvalidArg("n_timesteps must be > 0".into()));
        }

        // 优先使用 prompt_wav（zero-shot / 指定参考音频的路线）
        if let Some(prompt_wav) = params.prompt_wav.as_ref() {
            #[cfg(feature = "cosyvoice3-candle-onnx")]
            {
                let prompt = self.extract_prompt_features(prompt_wav.as_path())?;
                return self.synthesize_from_prompt_features(params, &prompt);
            }

            #[cfg(not(feature = "cosyvoice3-candle-onnx"))]
            {
                let _ = prompt_wav;
                return Err(TtsError::NotImplemented(
                    "extract prompt features requires feature `cosyvoice3-candle-onnx`",
                ));
            }
        }

        // route B：SFT spk_id（不依赖 prompt_wav / onnx frontend）
        if let Some(spk_id) = params
            .spk_id
            .as_deref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            let embedding =
                self.load_speaker_embedding_from_spk2info(spk_id, params.spk2info_json.as_deref())?;
            let prompt = CosyVoice3PromptFeatures {
                prompt_speech_tokens: Vec::new(),
                prompt_mel: Vec::new(),
                speaker_embedding: embedding,
                prompt_sample_rate: self.cfg.sample_rate as u32,
            };
            return self.synthesize_from_prompt_features(params, &prompt);
        }

        // 若未提供 prompt_wav / spk_id，则走 text-only fallback（方便快速跑通 pipeline）。
        let prompt = CosyVoice3PromptFeatures {
            prompt_speech_tokens: Vec::new(),
            prompt_mel: Vec::new(),
            speaker_embedding: vec![0.0; self.cfg.spk_embed_dim],
            prompt_sample_rate: self.cfg.sample_rate as u32,
        };
        self.synthesize_from_prompt_features(params, &prompt)
    }

    fn synthesize_from_prompt_features(
        &self,
        params: &CosyVoice3CandleParams,
        prompt: &CosyVoice3PromptFeatures,
    ) -> Result<CosyVoice3WavDebugResult, TtsError> {
        let debug = std::env::var_os("CHAOS_COSYVOICE3_DEBUG").is_some();

        let sampling = cv3_candle_transformers::models::cosyvoice::SamplingConfig {
            top_k: params.sampling.top_k,
            top_p: params.sampling.top_p,
            temperature: params.sampling.temperature,
            repetition_penalty: 1.0,
        };

        let mut inner = self.inner.lock().expect("lock cosyvoice3 inner");

        // 文本策略对齐：guide_prefix 时 spoken_text 前缀包含 prompt_text，但 prompt_inject_text 仅注入 endofprompt。
        let resolved = resolve_tts_text_basic(
            &params.text,
            &params.prompt_text,
            params.prompt_strategy,
            &params.guide_sep,
            params.text_frontend,
        )?;

        // Tokenize（spoken_text）
        let text_tokens = tokenize(&inner.tokenizer, &resolved.spoken_text)?;

        // mode 对齐 cosyvoice3.rs 的行为
        let (actual_prompt_text, llm_prompt_speech_tokens) = match params.mode {
            CosyVoice3Mode::ZeroShot => (
                resolved.prompt_inject_text.as_str(),
                prompt.prompt_speech_tokens.clone(),
            ),
            CosyVoice3Mode::CrossLingual => ("", Vec::new()),
            CosyVoice3Mode::Instruct => (resolved.prompt_inject_text.as_str(), Vec::new()),
        };
        let prompt_text_tokens = tokenize(&inner.tokenizer, actual_prompt_text)?;

        if debug {
            eprintln!(
                "[cosyvoice3-candle][debug] tokenize: text_tokens={} prompt_text_tokens={} llm_prompt_speech_tokens={} flow_prompt_speech_tokens={} mode={:?} prompt_strategy={} text_frontend={}",
                text_tokens.len(),
                prompt_text_tokens.len(),
                llm_prompt_speech_tokens.len(),
                prompt.prompt_speech_tokens.len(),
                params.mode,
                params.prompt_strategy.as_str(),
                params.text_frontend
            );
            eprintln!(
                "[cosyvoice3-candle][debug] resolved_text: spoken_text_len={} prompt_inject_text_len={}",
                resolved.spoken_text.len(),
                resolved.prompt_inject_text.len()
            );
        }

        // tensors
        let text_tokens_tensor =
            Tensor::from_slice(&text_tokens, (1, text_tokens.len()), &self.device)
                .map_err(|e| TtsError::Candle(format!("create text_tokens tensor failed: {e}")))?
                .to_dtype(DType::U32)
                .map_err(|e| TtsError::Candle(format!("text_tokens to u32 failed: {e}")))?;
        let prompt_text_tensor = if prompt_text_tokens.is_empty() {
            Tensor::zeros((1, 0), DType::U32, &self.device).map_err(|e| {
                TtsError::Candle(format!("create empty prompt_text tensor failed: {e}"))
            })?
        } else {
            Tensor::from_slice(
                &prompt_text_tokens,
                (1, prompt_text_tokens.len()),
                &self.device,
            )
            .map_err(|e| TtsError::Candle(format!("create prompt_text tensor failed: {e}")))?
            .to_dtype(DType::U32)
            .map_err(|e| TtsError::Candle(format!("prompt_text to u32 failed: {e}")))?
        };
        let llm_prompt_speech_tensor = if llm_prompt_speech_tokens.is_empty() {
            Tensor::zeros((1, 0), DType::U32, &self.device).map_err(|e| {
                TtsError::Candle(format!("create empty llm prompt_speech tensor failed: {e}"))
            })?
        } else {
            Tensor::from_slice(
                &llm_prompt_speech_tokens,
                (1, llm_prompt_speech_tokens.len()),
                &self.device,
            )
            .map_err(|e| TtsError::Candle(format!("create llm prompt_speech tensor failed: {e}")))?
            .to_dtype(DType::U32)
            .map_err(|e| TtsError::Candle(format!("llm prompt_speech to u32 failed: {e}")))?
        };

        let flow_prompt_speech_tensor = if prompt.prompt_speech_tokens.is_empty() {
            Tensor::zeros((1, 0), DType::U32, &self.device).map_err(|e| {
                TtsError::Candle(format!(
                    "create empty flow prompt_speech tensor failed: {e}"
                ))
            })?
        } else {
            Tensor::from_slice(
                &prompt.prompt_speech_tokens,
                (1, prompt.prompt_speech_tokens.len()),
                &self.device,
            )
            .map_err(|e| TtsError::Candle(format!("create flow prompt_speech tensor failed: {e}")))?
            .to_dtype(DType::U32)
            .map_err(|e| TtsError::Candle(format!("flow prompt_speech to u32 failed: {e}")))?
        };

        // prompt_mel: Vec<Vec<f32>> -> Tensor [1, T, 80]
        let (prompt_mel_t, prompt_mel_dim) = if prompt.prompt_mel.is_empty() {
            (
                Tensor::zeros((1, 0, 80), self.dtype, &self.device).map_err(|e| {
                    TtsError::Candle(format!("create empty prompt_mel tensor failed: {e}"))
                })?,
                80usize,
            )
        } else {
            vec2d_to_tensor(&prompt.prompt_mel, &self.device)?
        };
        if prompt_mel_dim != 80 {
            // 实际上 CosyVoice3 mel_dim=80；若不对齐通常是输入特征提取出了问题。
            return Err(TtsError::InvalidArg(format!(
                "prompt_mel dim must be 80, got {prompt_mel_dim}"
            )));
        }

        let speaker_embedding_tensor = Tensor::from_slice(
            &prompt.speaker_embedding,
            (1, prompt.speaker_embedding.len()),
            &self.device,
        )
        .map_err(|e| TtsError::Candle(format!("create speaker embedding tensor failed: {e}")))?;
        if speaker_embedding_tensor.dims().get(1).copied().unwrap_or(0) != self.cfg.spk_embed_dim {
            return Err(TtsError::InvalidArg(format!(
                "speaker_embedding dim must be {}, got {}",
                self.cfg.spk_embed_dim,
                speaker_embedding_tensor.dims().get(1).copied().unwrap_or(0)
            )));
        }

        // LLM
        let speech_tokens = inner
            .llm
            .inference(
                &text_tokens_tensor,
                &prompt_text_tensor,
                &llm_prompt_speech_tensor,
                &sampling,
            )
            .map_err(|e| TtsError::Candle(format!("llm inference failed: {e}")))?;
        if speech_tokens.is_empty() {
            return Err(TtsError::Candle("llm generated no speech tokens".into()));
        }

        if debug {
            let n = 20usize.min(speech_tokens.len());
            eprintln!(
                "[cosyvoice3-candle][debug] llm_speech_tokens_len={} head={:?}",
                speech_tokens.len(),
                &speech_tokens[..n]
            );
        }

        let speech_tokens_tensor =
            Tensor::from_slice(&speech_tokens, (1, speech_tokens.len()), &self.device)
                .map_err(|e| TtsError::Candle(format!("create speech_tokens tensor failed: {e}")))?
                .to_dtype(DType::U32)
                .map_err(|e| TtsError::Candle(format!("speech_tokens to u32 failed: {e}")))?;

        // Flow
        let mel = inner
            .flow_decoder
            .inference(
                &speech_tokens_tensor,
                &flow_prompt_speech_tensor,
                &prompt_mel_t,
                &speaker_embedding_tensor,
                params.n_timesteps,
                false,
            )
            .map_err(|e| TtsError::Candle(format!("flow inference failed: {e}")))?;

        // mel -> f32 vec -> speed scaling -> tensor
        let mel_f32 = mel
            .to_device(&self.device)
            .map_err(|e| TtsError::Candle(format!("mel to device failed: {e}")))?
            .to_dtype(DType::F32)
            .map_err(|e| TtsError::Candle(format!("mel to f32 failed: {e}")))?;

        // Flow 输出的 mel 按 Candle 实现约定是 [B, 80, T]（与 CausalHiFTGenerator::inference 的输入对齐）。
        // 但为了稳健起见，这里也兼容 [B, T, 80] / 2D 情况，并统一到 [1, 80, T]。
        let mel_shape0 = mel_f32.dims();
        let mel_b80t = if mel_shape0.len() == 3 && mel_shape0[1] == 80 {
            mel_f32
        } else if mel_shape0.len() == 3 && mel_shape0[2] == 80 {
            mel_f32.transpose(1, 2).map_err(|e| {
                TtsError::Candle(format!("transpose mel [B,T,80] -> [B,80,T] failed: {e}"))
            })?
        } else if mel_shape0.len() == 2 && mel_shape0[0] == 80 {
            mel_f32
                .reshape((1, mel_shape0[0], mel_shape0[1]))
                .map_err(|e| {
                    TtsError::Candle(format!("reshape mel [80,T] -> [1,80,T] failed: {e}"))
                })?
        } else if mel_shape0.len() == 2 && mel_shape0[1] == 80 {
            mel_f32
                .transpose(0, 1)
                .map_err(|e| {
                    TtsError::Candle(format!("transpose mel [T,80] -> [80,T] failed: {e}"))
                })?
                .reshape((1, mel_shape0[1], mel_shape0[0]))
                .map_err(|e| {
                    TtsError::Candle(format!("reshape mel [80,T] -> [1,80,T] failed: {e}"))
                })?
        } else {
            return Err(TtsError::InvalidArg(format!(
                "unexpected mel shape from flow: rank={} shape={:?}",
                mel_shape0.len(),
                mel_shape0
            )));
        };

        // 可选：根据 speed 做时间轴缩放。缩放在 [T,80]（或 [1,T,80]）布局下更好处理，
        // 但 vocoder 需要 [B,80,T]，因此缩放后再转回去。
        let mel_for_vocoder = if (params.speed - 1.0).abs() > f32::EPSILON {
            // [1,80,T] -> [1,T,80]
            let mel_bt80 = mel_b80t.transpose(1, 2).map_err(|e| {
                TtsError::Candle(format!("transpose mel [1,80,T] -> [1,T,80] failed: {e}"))
            })?;
            let mel_flat: Vec<f32> = mel_bt80
                .flatten_all()
                .map_err(|e| TtsError::Candle(format!("flatten mel failed: {e}")))?
                .to_vec1()
                .map_err(|e| TtsError::Candle(format!("mel to vec failed: {e}")))?;
            let mel_scaled_flat = time_scale_mel_linear(&mel_flat, 80, params.speed)?;
            let new_t = mel_scaled_flat.len() / 80;
            let mel_scaled_bt80 = Tensor::from_vec(mel_scaled_flat, (1, new_t, 80), &self.device)
                .map_err(|e| TtsError::Candle(format!("create scaled mel tensor failed: {e}")))?
                .to_dtype(self.dtype)
                .map_err(|e| TtsError::Candle(format!("scaled mel cast failed: {e}")))?;
            // [1,T,80] -> [1,80,T]
            mel_scaled_bt80.transpose(1, 2).map_err(|e| {
                TtsError::Candle(format!(
                    "transpose scaled mel [1,T,80] -> [1,80,T] failed: {e}"
                ))
            })?
        } else {
            mel_b80t
                .to_dtype(self.dtype)
                .map_err(|e| TtsError::Candle(format!("mel cast failed: {e}")))?
        };

        // Vocoder（输入要求 [B,80,T]）
        let waveform = inner
            .vocoder
            .inference(&mel_for_vocoder, true)
            .map_err(|e| TtsError::Candle(format!("vocoder inference failed: {e}")))?;
        let pcm = if waveform.dims().len() == 3 {
            waveform
                .squeeze(0)
                .map_err(|e| TtsError::Candle(format!("wave squeeze failed: {e}")))?
                .squeeze(0)
                .map_err(|e| TtsError::Candle(format!("wave squeeze failed: {e}")))?
        } else if waveform.dims().len() == 2 {
            waveform
                .squeeze(0)
                .map_err(|e| TtsError::Candle(format!("wave squeeze failed: {e}")))?
        } else {
            waveform
        };
        let mut pcm_f32: Vec<f32> = pcm
            .to_dtype(DType::F32)
            .map_err(|e| TtsError::Candle(format!("pcm to f32 failed: {e}")))?
            .to_vec1()
            .map_err(|e| TtsError::Candle(format!("pcm to vec failed: {e}")))?;

        // 注：早期实现会把 guide_prefix 的 guide 拼到 spoken_text 前面，因此需要裁掉音频前缀。
        // 现在 guide_prefix 会把 guide 放进 prompt_text（不应“读出来”），因此这里不再做裁剪。

        // 可选后处理：陷波去窄带啸叫（与 ONNX 路线保持一致的 env 行为）。
        if let Ok(hz) = std::env::var("CHAOS_TTS_POST_NOTCH_HZ") {
            let hz = hz.trim();
            if !hz.is_empty() {
                if let Ok(hz) = hz.parse::<f32>() {
                    let q = std::env::var("CHAOS_TTS_POST_NOTCH_Q")
                        .ok()
                        .and_then(|v| v.trim().parse::<f32>().ok())
                        .unwrap_or(20.0);
                    let _ = notch_filter_f32_mono_inplace(
                        &mut pcm_f32,
                        self.cfg.sample_rate as u32,
                        hz,
                        q,
                    );
                    eprintln!(
                        "[cosyvoice3-candle] applied notch filter: hz={} q={}",
                        hz, q
                    );
                }
            }
        }

        let pcm16 = f32_to_pcm16_mono(&pcm_f32);
        let wav_bytes = encode_wav_pcm16_mono(self.cfg.sample_rate as u32, &pcm16)?;
        let wav = TtsWavResult {
            sample_rate: self.cfg.sample_rate as u32,
            channels: 1,
            duration_ms: duration_ms(self.cfg.sample_rate as u32, pcm16.len()),
            wav_bytes,
        };

        Ok(CosyVoice3WavDebugResult { wav, speech_tokens })
    }

    fn load_speaker_embedding_from_spk2info(
        &self,
        spk_id: &str,
        spk2info_override: Option<&Path>,
    ) -> Result<Vec<f32>, TtsError> {
        let p = if let Some(p) = spk2info_override {
            p.to_path_buf()
        } else if let Some(raw) = std::env::var_os("CHAOS_COSYVOICE3_SPK2INFO_JSON") {
            PathBuf::from(raw)
        } else {
            self.model_dir.join("spk2info.json")
        };

        if !p.exists() {
            return Err(TtsError::InvalidArg(format!(
                "spk2info.json not found: {} (hint: set CHAOS_COSYVOICE3_SPK2INFO_JSON or put spk2info.json next to config.json)",
                p.display()
            )));
        }

        let map = crate::tts::CosyVoicePack::load_spk2info(&p, self.cfg.spk_embed_dim)?;
        let info = map.get(spk_id).ok_or_else(|| {
            TtsError::InvalidArg(format!(
                "spk_id not found in spk2info.json: spk_id={spk_id} path={}",
                p.display()
            ))
        })?;
        Ok(info.embedding.clone())
    }
}

fn load_tokenizer(model_dir: &Path) -> Result<tokenizers::Tokenizer, TtsError> {
    let tokenizer_dir = model_dir.join("tokenizer");
    let tokenizer_json = tokenizer_dir.join("tokenizer.json");
    if tokenizer_json.exists() {
        tokenizers::Tokenizer::from_file(&tokenizer_json)
            .map_err(|e| TtsError::Tokenizer(format!("failed to load tokenizer.json: {e}")))
    } else {
        // 退化：仅凭 vocab.json/merges.txt 构建（某些模型配置不足时可能导致中文分词异常）
        let vocab_path = tokenizer_dir.join("vocab.json");
        let merges_path = tokenizer_dir.join("merges.txt");
        use tokenizers::models::bpe::BPE;
        let bpe = BPE::from_file(
            &vocab_path.to_string_lossy(),
            &merges_path.to_string_lossy(),
        )
        .build()
        .map_err(|e| TtsError::Tokenizer(format!("failed to build bpe tokenizer: {e}")))?;
        Ok(tokenizers::Tokenizer::new(bpe))
    }
}

fn tokenize(tokenizer: &tokenizers::Tokenizer, text: &str) -> Result<Vec<u32>, TtsError> {
    let enc = tokenizer
        .encode(text, false)
        .map_err(|e| TtsError::Tokenizer(format!("tokenize failed: {e}")))?;
    Ok(enc.get_ids().to_vec())
}

fn select_device(dev_req: &str) -> Result<(Device, bool), TtsError> {
    match dev_req {
        "cpu" => Ok((Device::Cpu, false)),
        "auto" => {
            #[cfg(feature = "cosyvoice3-candle-cuda")]
            {
                match Device::new_cuda(0) {
                    Ok(d) => return Ok((d, true)),
                    Err(e) => {
                        eprintln!("[cosyvoice3-candle] cuda not available, fallback to cpu: {e}");
                    }
                }
            }
            Ok((Device::Cpu, false))
        }
        "cuda" => {
            #[cfg(feature = "cosyvoice3-candle-cuda")]
            {
                let d = Device::new_cuda(0).map_err(|e| {
                    TtsError::Candle(format!("create cuda device failed: {e} (hint: ensure VS cl.exe + nvcc are available during build)"))
                })?;
                return Ok((d, true));
            }
            #[cfg(not(feature = "cosyvoice3-candle-cuda"))]
            {
                Err(TtsError::NotImplemented(
                    "cuda requested but feature `cosyvoice3-candle-cuda` is not enabled",
                ))
            }
        }
        other => Err(TtsError::InvalidArg(format!(
            "invalid CHAOS_COSYVOICE3_DEVICE={other}, expected cpu/auto/cuda"
        ))),
    }
}

/// WAV 解码：支持 PCM16/PCM32/Float32；输出 [-1, 1] 的 mono f32。
fn decode_wav_to_f32_mono(path: &Path) -> Result<(Vec<f32>, u32), TtsError> {
    let mut r = hound::WavReader::open(path).map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
    let spec = r.spec();
    let sr = spec.sample_rate;
    let channels = spec.channels.max(1) as usize;

    let samples_f32: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            let mut out = Vec::new();
            for s in r.samples::<f32>() {
                out.push(s.map_err(|e| TtsError::Io(std::io::Error::other(e)))?);
            }
            out
        }
        hound::SampleFormat::Int => {
            // bits_per_sample 可能是 16/24/32，这里统一按 i32 读取再归一化。
            let max = (1u64 << (spec.bits_per_sample.saturating_sub(1) as u64)) as f32;
            let mut out = Vec::new();
            for s in r.samples::<i32>() {
                let v = s.map_err(|e| TtsError::Io(std::io::Error::other(e)))? as f32;
                out.push((v / max).clamp(-1.0, 1.0));
            }
            out
        }
    };

    if channels == 1 {
        return Ok((samples_f32, sr));
    }

    // 多声道：平均到 mono（提示音频通常是 mono；这里做个兜底）
    let frames = samples_f32.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for i in 0..frames {
        let mut sum = 0.0f32;
        for ch in 0..channels {
            sum += samples_f32[i * channels + ch];
        }
        mono.push(sum / (channels as f32));
    }
    Ok((mono, sr))
}

/// Vec<Vec<f32>> -> Tensor([1, T, D])，并返回 D。
fn vec2d_to_tensor(data: &[Vec<f32>], device: &Device) -> Result<(Tensor, usize), TtsError> {
    if data.is_empty() {
        return Err(TtsError::InvalidArg("prompt_mel is empty".into()));
    }
    let d = data[0].len();
    if d == 0 {
        return Err(TtsError::InvalidArg("prompt_mel dim is 0".into()));
    }
    for (i, row) in data.iter().enumerate() {
        if row.len() != d {
            return Err(TtsError::InvalidArg(format!(
                "prompt_mel row {i} dim mismatch: expected {d}, got {}",
                row.len()
            )));
        }
    }
    let t = data.len();
    let mut flat = Vec::with_capacity(t * d);
    for row in data {
        flat.extend_from_slice(row);
    }
    let t = Tensor::from_vec(flat, (1, t, d), device)
        .map_err(|e| TtsError::Candle(format!("prompt_mel to tensor failed: {e}")))?;
    Ok((t, d))
}

/// 线性插值对 mel 做时间缩放（与 ONNX 路线保持一致的实现）。
fn time_scale_mel_linear(mel: &[f32], channels: usize, speed: f32) -> Result<Vec<f32>, TtsError> {
    if speed <= 0.0 {
        return Err(TtsError::InvalidArg("speed must be > 0".into()));
    }
    if mel.len() % channels != 0 {
        return Err(TtsError::InvalidArg("mel shape invalid".into()));
    }
    let t = mel.len() / channels;
    if t == 0 {
        return Ok(Vec::new());
    }
    let new_t = ((t as f32) / speed).round().max(1.0) as usize;
    if new_t == t {
        return Ok(mel.to_vec());
    }

    // align_corners=false 的线性插值（更不容易引入高频尖啸）。
    let mut out = vec![0.0f32; channels * new_t];
    if new_t == 1 {
        let mid = t / 2;
        for ch in 0..channels {
            out[ch] = mel[ch * t + mid];
        }
        return Ok(out);
    }
    let t_f = t as f32;
    let new_t_f = new_t as f32;
    for ch in 0..channels {
        for i in 0..new_t {
            let mut src_pos = ((i as f32) + 0.5) * (t_f / new_t_f) - 0.5;
            if src_pos < 0.0 {
                src_pos = 0.0;
            }
            let max_pos = (t - 1) as f32;
            if src_pos > max_pos {
                src_pos = max_pos;
            }
            let lo = src_pos.floor() as usize;
            let hi = (lo + 1).min(t - 1);
            let a = src_pos - (lo as f32);
            let lo_v = mel[ch * t + lo];
            let hi_v = mel[ch * t + hi];
            out[ch * new_t + i] = lo_v * (1.0 - a) + hi_v * a;
        }
    }
    Ok(out)
}
