use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "onnx-ort")]
use std::fs::OpenOptions;
#[cfg(feature = "onnx-ort")]
use std::io::Write;
#[cfg(feature = "onnx-ort")]
use std::path::PathBuf;
#[cfg(feature = "onnx-ort")]
use std::sync::OnceLock;
#[cfg(feature = "onnx-ort")]
use std::sync::Mutex;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

use crate::tts::TtsError;
use crate::tts::cosyvoice::pack::CosyVoicePack;
use crate::tts::sampling::{SamplingConfig, sample_ras_next};
use crate::tts::text::{PromptStrategy, resolve_tts_text_basic};
use crate::tts::wav::{
    TtsPcm16Result, TtsWavResult, duration_ms, encode_wav_pcm16_mono, f32_to_pcm16_mono,
    notch_filter_f32_mono_inplace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsJobStage {
    Loading,
    Tokenizing,
    Llm,
    Flow,
    Vocoder,
    Encoding,
}

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

/// 仅用于调试/对齐：返回生成的 speech_tokens 以及关键张量信息（避免重复跑 LLM）。
#[derive(Debug, Clone)]
pub struct TtsWavDebugResult {
    pub wav: TtsWavResult,
    pub speech_tokens: Vec<i64>,
    /// `llm_prefill/llm_decode` 的 logits 最后一帧 vocab 大小（等价于 `last_logits.len()`）。
    pub llm_logits_vocab_size: usize,
}

#[derive(Debug, Clone)]
struct LlmGenerateResult {
    speech_tokens: Vec<i64>,
    logits_vocab_size: usize,
}

#[cfg(feature = "onnx-ort")]
struct OrtOnnxIo {
    inputs: Vec<String>,
    outputs: Vec<String>,
}

#[cfg(feature = "onnx-ort")]
struct OrtModel {
    session: ort::session::Session,
    io: OrtOnnxIo,
}

#[cfg(feature = "onnx-ort")]
struct OrtBackend {
    llm_prefill: OrtModel,
    llm_decode: OrtModel,
    flow_infer: OrtModel,
    hift_infer: OrtModel,
    /// llm_decode 的兼容策略：某些 pack 导出的 decode 图在 past_len>3 时会 shape mismatch；此时用 prefill 反复跑来保证正确性（但更慢）。
    llm_decode_mode: OrtLlmDecodeMode,
    /// 兼容某些导出有问题/做了窗口化的 decode 图：只保留 KV cache 的最后 N 个时间步。
    ///
    /// - `None`：不裁剪（默认）。
    /// - `Some(n)`：每步 decode 前/后都把 past/present 的 seq 维裁剪到 n。
    kv_cache_keep: Option<usize>,
    /// flow_infer 是否只能吃固定长度的 speech_tokens。
    ///
    /// - `None`：支持变长（或未知）。
    /// - `Some(n)`：将 speech_tokens 按 n 分块，逐块跑 flow_infer，再把 mel 在时间轴拼接。
    flow_token_chunk_len: Option<usize>,
}

#[cfg(feature = "onnx-ort")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrtLlmDecodeMode {
    /// 使用 llm_prefill + llm_decode（KV cache）快速解码。
    DecodeGraph,
    /// 不使用 llm_decode；每步都用 llm_prefill 在完整上下文上重新跑一遍得到最后 logits（慢但更稳）。
    PrefillOnly,
}

#[cfg(feature = "onnx-tract")]
struct TractBackend {
    llm_prefill: Arc<
        tract_core::plan::SimplePlan<
            tract_onnx::prelude::TypedFact,
            Box<dyn tract_onnx::prelude::TypedOp>,
        >,
    >,
    llm_decode: Arc<
        tract_core::plan::SimplePlan<
            tract_onnx::prelude::TypedFact,
            Box<dyn tract_onnx::prelude::TypedOp>,
        >,
    >,
    flow_infer: Arc<
        tract_core::plan::SimplePlan<
            tract_onnx::prelude::TypedFact,
            Box<dyn tract_onnx::prelude::TypedOp>,
        >,
    >,
    hift_infer: Arc<
        tract_core::plan::SimplePlan<
            tract_onnx::prelude::TypedFact,
            Box<dyn tract_onnx::prelude::TypedOp>,
        >,
    >,
}

/// ONNX-backed CosyVoice3 engine.
///
/// Note: V1 uses `tract` as the default backend (pure Rust) for maximum ONNX operator coverage.
/// The rest of the architecture (voice chat stream, protocol, post-processing) is backend-agnostic.
pub struct CosyVoiceEngine {
    pack: Arc<CosyVoicePack>,
    #[cfg(feature = "onnx-ort")]
    ort: Option<OrtBackend>,
    #[cfg(feature = "onnx-tract")]
    tract: Option<TractBackend>,
}

impl std::fmt::Debug for CosyVoiceEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // tract/candle 的 plan 很大且不一定实现 Debug；这里仅输出关键信息，避免日志/调试爆炸。
        f.debug_struct("CosyVoiceEngine")
            .field("model_dir", &self.pack.model_dir)
            .field("sample_rate", &self.pack.cfg.sample_rate)
            .finish_non_exhaustive()
    }
}

impl CosyVoiceEngine {
    pub fn load(pack: CosyVoicePack) -> Result<Self, TtsError> {
        let pack = Arc::new(pack);

        // 优先使用 onnxruntime（ort）后端：对大 LLM 图（如 Qwen2 KV cache concat）更稳。
        #[cfg(feature = "onnx-ort")]
        {
            let ort = Some(load_onnx_ort_backend(&pack)?);
            #[cfg(feature = "onnx-tract")]
            let tract = None;
            return Ok(Self {
                pack,
                ort,
                #[cfg(feature = "onnx-tract")]
                tract,
            });
        }

        // 兜底：纯 Rust tract 后端（某些模型可能在分析/优化阶段失败）。
        #[cfg(all(not(feature = "onnx-ort"), feature = "onnx-tract"))]
        {
            let llm_prefill = load_onnx_plan(pack.path_llm_prefill())?;
            let llm_decode = load_onnx_plan(pack.path_llm_decode())?;
            let flow_infer = load_onnx_plan(pack.path_flow_infer())?;
            let hift_infer = load_onnx_plan(pack.path_hift_infer())?;

            let tract = Some(TractBackend {
                llm_prefill,
                llm_decode,
                flow_infer,
                hift_infer,
            });

            return Ok(Self {
                pack,
                #[cfg(feature = "onnx-tract")]
                tract,
            });
        }

        #[cfg(all(not(feature = "onnx-ort"), not(feature = "onnx-tract")))]
        {
            let _ = pack;
            Err(TtsError::NotImplemented(
                "CosyVoiceEngine requires feature `onnx-ort` or `onnx-tract`",
            ))
        }
    }

    pub fn pack(&self) -> &CosyVoicePack {
        &self.pack
    }

    /// 仅运行 LLM，生成 speech_tokens（不经过 flow/vocoder）。
    ///
    /// 用途：Rust/Python 侧对齐 tokens、排查推理慢/电音等问题。
    pub fn synthesize_speech_tokens(&self, params: &TtsSftParams) -> Result<Vec<i64>, TtsError> {
        self.synthesize_speech_tokens_with_cancel(params, None)
    }

    pub fn synthesize_speech_tokens_with_cancel(
        &self,
        params: &TtsSftParams,
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<i64>, TtsError> {
        // 保持与完整推理一致的参数校验，避免“只跑到 LLM 但 spk_id 无效”导致排查混乱。
        if params.speed <= 0.0 {
            return Err(TtsError::InvalidArg("speed must be > 0".into()));
        }
        if !self.pack.spk2info.contains_key(params.spk_id.trim()) {
            return Err(TtsError::InvalidArg(format!(
                "spk_id not found in spk2info.json: {}",
                params.spk_id
            )));
        }

        let (input_ids, spoken_text_len) = self.encode_input_ids(params)?;
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(TtsError::Canceled);
            }
        }
        let mut rng = ChaCha20Rng::seed_from_u64(params.seed);
        let r = self.llm_generate(
            &input_ids,
            spoken_text_len,
            &params.sampling,
            &mut rng,
            cancel,
        )?;
        Ok(r.speech_tokens)
    }

    /// 调试版：一次性返回 wav + speech_tokens + logits vocab（避免重复跑 LLM）。
    pub fn synthesize_wav_bytes_debug(
        &self,
        params: &TtsSftParams,
    ) -> Result<TtsWavDebugResult, TtsError> {
        self.synthesize_wav_bytes_debug_with_cancel(params, None)
    }

    pub fn synthesize_wav_bytes_debug_with_cancel(
        &self,
        params: &TtsSftParams,
        cancel: Option<&AtomicBool>,
    ) -> Result<TtsWavDebugResult, TtsError> {
        let cfg = &self.pack.cfg;
        if params.speed <= 0.0 {
            return Err(TtsError::InvalidArg("speed must be > 0".into()));
        }
        if !self.pack.spk2info.contains_key(params.spk_id.trim()) {
            return Err(TtsError::InvalidArg(format!(
                "spk_id not found in spk2info.json: {}",
                params.spk_id
            )));
        }

        let (input_ids, spoken_text_len) = self.encode_input_ids(params)?;
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(TtsError::Canceled);
            }
        }

        let mut rng = ChaCha20Rng::seed_from_u64(params.seed);
        let llm = self.llm_generate(
            &input_ids,
            spoken_text_len,
            &params.sampling,
            &mut rng,
            cancel,
        )?;
        let speech_tokens = llm.speech_tokens;

        if speech_tokens.is_empty() {
            return Err(TtsError::Onnx("LLM produced no speech tokens".into()));
        }

        // Flow: tokens -> mel
        let spk = self
            .pack
            .spk2info
            .get(params.spk_id.trim())
            .expect("checked");
        let mel = self.flow_tokens_to_mel(&speech_tokens, &spk.embedding, cancel)?;

        // Speed change: only for non-stream mode. We apply linear interpolation on mel time axis.
        let mel = if (params.speed - 1.0).abs() > f32::EPSILON {
            time_scale_mel_linear(&mel, 80, params.speed)?
        } else {
            mel
        };

        // HiFT: mel -> waveform f32
        let mut wav_f32 = self.hift_mel_to_wav(&mel, cancel)?;

        // 兜底后处理：去除窄带高频啸叫（例如导出/推理不稳定导致的 6kHz tone）。
        // 默认关闭；需要时设置：
        // - `CHAOS_TTS_POST_NOTCH_HZ=6000`
        // - `CHAOS_TTS_POST_NOTCH_Q=30`
        if let Ok(hz) = std::env::var("CHAOS_TTS_POST_NOTCH_HZ") {
            let hz = hz.trim();
            if !hz.is_empty() {
                let q = std::env::var("CHAOS_TTS_POST_NOTCH_Q")
                    .ok()
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .unwrap_or(30.0);
                if let Ok(freq_hz) = hz.parse::<f32>() {
                    let _ = notch_filter_f32_mono_inplace(
                        &mut wav_f32,
                        cfg.sample_rate,
                        freq_hz,
                        q,
                    );
                }
            }
        }

        let pcm16 = f32_to_pcm16_mono(&wav_f32);

        let wav_bytes = encode_wav_pcm16_mono(cfg.sample_rate, &pcm16)?;
        let wav = TtsWavResult {
            sample_rate: cfg.sample_rate,
            channels: 1,
            duration_ms: duration_ms(cfg.sample_rate, pcm16.len()),
            wav_bytes,
        };

        Ok(TtsWavDebugResult {
            wav,
            speech_tokens,
            llm_logits_vocab_size: llm.logits_vocab_size,
        })
    }

    fn encode_input_ids(&self, params: &TtsSftParams) -> Result<(Vec<i64>, usize), TtsError> {
        let cfg = &self.pack.cfg;
        let resolved = resolve_tts_text_basic(
            &params.text,
            &params.prompt_text,
            params.prompt_strategy,
            &params.guide_sep,
            params.text_frontend,
        )?;

        let add_special = cfg.tokenizer_add_special_tokens;
        let enc_prompt = self
            .pack
            .tokenizer
            .encode(resolved.prompt_inject_text.clone(), add_special)
            .map_err(|e| TtsError::Tokenizer(format!("encode prompt_text failed: {e}")))?;
        let enc_text = self
            .pack
            .tokenizer
            .encode(resolved.spoken_text.clone(), add_special)
            .map_err(|e| TtsError::Tokenizer(format!("encode text failed: {e}")))?;

        let spoken_text_len = enc_text.get_ids().len();
        let mut input_ids: Vec<i64> =
            Vec::with_capacity(enc_prompt.get_ids().len() + enc_text.get_ids().len());
        input_ids.extend(enc_prompt.get_ids().iter().map(|&x| x as i64));
        input_ids.extend(enc_text.get_ids().iter().map(|&x| x as i64));

        if !input_ids
            .iter()
            .any(|&x| x as u32 == cfg.end_of_prompt_token_id)
        {
            return Err(TtsError::InvalidArg(format!(
                "endOfPromptTokenId={} not found in encoded prompt/text; pack.json and tokenizer.json likely mismatch",
                cfg.end_of_prompt_token_id
            )));
        }

        Ok((input_ids, spoken_text_len))
    }

    pub fn synthesize_pcm16(&self, params: &TtsSftParams) -> Result<TtsPcm16Result, TtsError> {
        self.synthesize_pcm16_with_cancel(params, None)
    }

    pub fn synthesize_pcm16_with_cancel(
        &self,
        params: &TtsSftParams,
        cancel: Option<&AtomicBool>,
    ) -> Result<TtsPcm16Result, TtsError> {
        let cfg = &self.pack.cfg;
        if params.speed <= 0.0 {
            return Err(TtsError::InvalidArg("speed must be > 0".into()));
        }
        if !self.pack.spk2info.contains_key(params.spk_id.trim()) {
            return Err(TtsError::InvalidArg(format!(
                "spk_id not found in spk2info.json: {}",
                params.spk_id
            )));
        }

        let (input_ids, spoken_text_len) = self.encode_input_ids(params)?;

        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(TtsError::Canceled);
            }
        }

        // LLM: autoregressively sample speech tokens until stop token.
        let mut rng = ChaCha20Rng::seed_from_u64(params.seed);
        let llm = self.llm_generate(
            &input_ids,
            spoken_text_len,
            &params.sampling,
            &mut rng,
            cancel,
        )?;
        let speech_tokens = llm.speech_tokens;

        if speech_tokens.is_empty() {
            return Err(TtsError::Onnx("LLM produced no speech tokens".into()));
        }

        // Flow: tokens -> mel
        let spk = self
            .pack
            .spk2info
            .get(params.spk_id.trim())
            .expect("checked");
        let mel = self.flow_tokens_to_mel(&speech_tokens, &spk.embedding, cancel)?;

        // Speed change: only for non-stream mode. We apply linear interpolation on mel time axis.
        let mel = if (params.speed - 1.0).abs() > f32::EPSILON {
            time_scale_mel_linear(&mel, 80, params.speed)?
        } else {
            mel
        };

        // HiFT: mel -> waveform f32
        let mut wav_f32 = self.hift_mel_to_wav(&mel, cancel)?;
        if let Ok(hz) = std::env::var("CHAOS_TTS_POST_NOTCH_HZ") {
            let hz = hz.trim();
            if !hz.is_empty() {
                let q = std::env::var("CHAOS_TTS_POST_NOTCH_Q")
                    .ok()
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .unwrap_or(30.0);
                if let Ok(freq_hz) = hz.parse::<f32>() {
                    let _ = notch_filter_f32_mono_inplace(
                        &mut wav_f32,
                        cfg.sample_rate,
                        freq_hz,
                        q,
                    );
                }
            }
        }

        let pcm16 = f32_to_pcm16_mono(&wav_f32);
        Ok(TtsPcm16Result {
            sample_rate: cfg.sample_rate,
            channels: 1,
            duration_ms: duration_ms(cfg.sample_rate, pcm16.len()),
            pcm16,
        })
    }

    pub fn synthesize_wav_bytes(&self, params: &TtsSftParams) -> Result<TtsWavResult, TtsError> {
        self.synthesize_wav_bytes_with_cancel(params, None)
    }

    pub fn synthesize_wav_bytes_with_cancel(
        &self,
        params: &TtsSftParams,
        cancel: Option<&AtomicBool>,
    ) -> Result<TtsWavResult, TtsError> {
        let pcm = self.synthesize_pcm16_with_cancel(params, cancel)?;
        let wav_bytes = encode_wav_pcm16_mono(pcm.sample_rate, &pcm.pcm16)?;
        Ok(TtsWavResult {
            sample_rate: pcm.sample_rate,
            channels: pcm.channels,
            duration_ms: pcm.duration_ms,
            wav_bytes,
        })
    }

    fn llm_generate(
        &self,
        input_ids: &[i64],
        spoken_text_len: usize,
        sampling: &SamplingConfig,
        rng: &mut ChaCha20Rng,
        cancel: Option<&AtomicBool>,
    ) -> Result<LlmGenerateResult, TtsError> {
        let stop_start = self.pack.cfg.stop_token_start as i64;
        // Mirror CosyVoice's max_token_text_ratio/min_token_text_ratio guardrails (approx).
        let llm_cfg = &self.pack.cfg.llm;
        let min_len = ((spoken_text_len as f32) * llm_cfg.min_token_text_ratio)
            .floor()
            .max(0.0) as usize;
        let mut max_len = ((spoken_text_len as f32) * llm_cfg.max_token_text_ratio)
            .floor()
            .max(1.0) as usize;
        // Absolute safety cap to avoid runaway on bad packs/config.
        max_len = max_len.min(200_000);

        // 基本一致性校验：
        // - stopTokenStart 表示「token_id >= stopTokenStart 即停止」。
        // - 若 logits 的 vocab_size <= stopTokenStart，则永远采样不到 stop token，最终会跑到 max_len：
        //   推理很慢 + 音频大概率是噪声/电音（tokens 已经跑飞）。
        fn ensure_stop_token_reachable(
            stop_start: i64,
            vocab_size: usize,
            stage: &'static str,
        ) -> Result<(), TtsError> {
            if vocab_size == 0 {
                return Err(TtsError::Onnx(format!(
                    "{stage}: logits vocab_size is 0 (invalid model output)"
                )));
            }
            let vocab_i64 = vocab_size as i64;
            if stop_start >= vocab_i64 {
                return Err(TtsError::InvalidArg(format!(
                    "{stage}: stopTokenStart={stop_start} is >= logits_vocab_size={vocab_size}; stop token range is unreachable. Fix pack.json stopTokenStart (or re-export pack)."
                )));
            }
            Ok(())
        }

        #[cfg(feature = "onnx-ort")]
        if let Some(ort) = &self.ort {
            use ort::value::{DynValue, Tensor};

            cosyvoice_debug_log(format_args!(
                "[cosyvoice][llm] begin: mode={:?} kv_keep={:?} input_len={} spoken_text_len={} stopTokenStart={} speechTokenSize={} min_len={} max_len={} sampling={:?}\n",
                ort.llm_decode_mode,
                ort.kv_cache_keep,
                input_ids.len(),
                spoken_text_len,
                stop_start,
                self.pack.cfg.speech_token_size,
                min_len,
                max_len,
                sampling
            ));

            let in_name = ort
                .llm_prefill
                .io
                .inputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("input_ids");
            let out_logits_name = ort
                .llm_prefill
                .io
                .outputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("logits");

            // 兼容：若 llm_decode 图在常规 past_len 下不可靠，则用 PrefillOnly（每步重跑 prefill）保证质量。
            if ort.llm_decode_mode == OrtLlmDecodeMode::PrefillOnly {
                let mut ctx: Vec<i64> = input_ids.to_vec();
                let mut decoded: Vec<i64> = Vec::new();
                let mut decoded_u32: Vec<u32> = Vec::new();
                let mut vocab_size_seen: Option<usize> = None;
                // 性能/质量折中：PrefillOnly 每步都要跑一次 llm_prefill；默认只保留最后 N 个 token 作为上下文，避免 O(n^2)。
                let prefill_window = std::env::var("CHAOS_COSYVOICE_ORT_PREFILL_WINDOW")
                    .ok()
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .filter(|&v| v > 0)
                    .unwrap_or(256);
                let log_every = cosyvoice_debug_log_every();
                cosyvoice_debug_log(format_args!(
                    "[cosyvoice][llm] PrefillOnly: prefill_window={prefill_window} log_every={log_every}\n"
                ));

                while decoded.len() < max_len {
                    if let Some(c) = cancel {
                        if c.load(Ordering::Relaxed) {
                            return Err(TtsError::Canceled);
                        }
                    }

                    let ctx_slice: &[i64] = if ctx.len() > prefill_window {
                        &ctx[ctx.len() - prefill_window..]
                    } else {
                        &ctx
                    };

                    let input = Tensor::<i64>::from_array((
                        vec![1usize, ctx_slice.len()],
                        ctx_slice.to_vec().into_boxed_slice(),
                    ))
                    .map_err(|e| TtsError::Onnx(format!("ort: create input_ids failed: {e}")))?;

                    let mut out = ort
                        .llm_prefill
                        .session
                        .run(vec![(in_name, input.into_dyn())])
                        .map_err(|e| TtsError::Onnx(format!("ort: llm_prefill run failed: {e}")))?;

                    let logits_v = out.remove(out_logits_name).ok_or_else(|| {
                        TtsError::Onnx("ort: llm_prefill missing logits output".into())
                    })?;
                    let mut step_scores = ort_extract_last_logits(
                        &logits_v,
                        "llm_prefill(prefill_only)",
                        decoded.len() == 0,
                    )?;

                    if vocab_size_seen.is_none() {
                        ensure_stop_token_reachable(
                            stop_start,
                            step_scores.len(),
                            "llm_prefill(prefill_only)",
                        )?;
                        vocab_size_seen = Some(step_scores.len());
                    }
                    // Optional guard: avoid stop tokens too early.
                    if decoded.len() < min_len {
                        let start = stop_start.max(0) as usize;
                        for s in step_scores.iter_mut().skip(start) {
                            *s = f32::NEG_INFINITY;
                        }
                        if !step_scores.iter().any(|x| x.is_finite()) {
                            return Err(TtsError::Onnx(
                                "LLM logits are all -inf after min_len stop-token masking".into(),
                            ));
                        }
                    }

                    let token_u32 = sample_ras_next(&step_scores, &decoded_u32, sampling, rng)?;
                    let token = token_u32 as i64;
                    if token >= stop_start {
                        break;
                    }
                    decoded.push(token);
                    decoded_u32.push(token_u32);
                    ctx.push(token);

                    if cosyvoice_debug_log_enabled()
                        && (decoded.len() <= 3 || decoded.len() % log_every == 0)
                    {
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][llm][prefill_only] step={} ctx_len={} token={} (stop_start={})\n",
                            decoded.len(),
                            ctx.len(),
                            token,
                            stop_start
                        ));
                    }
                }
                if decoded.len() >= max_len {
                    return Err(TtsError::Onnx(format!(
                        "LLM reached max_len={max_len} without emitting stop token (stopTokenStart={stop_start}, logits_vocab_size={}). This usually means pack.json stopTokenStart is wrong or the exported LLM has no stop token.",
                        vocab_size_seen.unwrap_or(0)
                    )));
                }
                cosyvoice_debug_log(format_args!(
                    "[cosyvoice][llm] end(PrefillOnly): speech_tokens_len={}\n",
                    decoded.len()
                ));
                if cosyvoice_debug_log_enabled() {
                    let speech_size = self.pack.cfg.speech_token_size as i64;
                    if decoded.is_empty() {
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][llm] speech_tokens stats: <empty>\n"
                        ));
                    } else {
                        let mut min_tok = i64::MAX;
                        let mut max_tok = i64::MIN;
                        let mut out_of_range = 0usize;
                        for &t in &decoded {
                            min_tok = min_tok.min(t);
                            max_tok = max_tok.max(t);
                            if t < 0 || (speech_size > 0 && t >= speech_size) {
                                out_of_range += 1;
                            }
                        }
                        let head_len = decoded.len().min(16);
                        let tail_len = decoded.len().min(16);
                        let head = &decoded[..head_len];
                        let tail = &decoded[decoded.len().saturating_sub(tail_len)..];
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][llm] speech_tokens stats: min={min_tok} max={max_tok} speechTokenSize={speech_size} out_of_range={out_of_range}\n"
                        ));
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][llm] speech_tokens head={head:?} tail={tail:?}\n"
                        ));
                    }
                }
                return Ok(LlmGenerateResult {
                    speech_tokens: decoded,
                    logits_vocab_size: vocab_size_seen.unwrap_or(0),
                });
            }

            let kv_keep = ort.kv_cache_keep;

            let input = Tensor::<i64>::from_array((
                vec![1usize, input_ids.len()],
                input_ids.to_vec().into_boxed_slice(),
            ))
            .map_err(|e| TtsError::Onnx(format!("ort: create input_ids failed: {e}")))?;

            let mut prefill_out = ort
                .llm_prefill
                .session
                .run(vec![(in_name, input.into_dyn())])
                .map_err(|e| TtsError::Onnx(format!("ort: llm_prefill run failed: {e}")))?;

            let logits_v = prefill_out
                .remove(out_logits_name)
                .ok_or_else(|| TtsError::Onnx("ort: llm_prefill missing logits output".into()))?;
            let mut last_logits = ort_extract_last_logits(&logits_v, "llm_prefill", true)?;

            ensure_stop_token_reachable(stop_start, last_logits.len(), "llm_prefill")?;
            // 将 prefill 的 KV cache 输出按 decode 的 past_* 输入顺序对齐，避免“输出顺序 != 输入顺序”导致形状错配。
            let mut past: Vec<DynValue> =
                Vec::with_capacity(ort.llm_decode.io.inputs.len().saturating_sub(1));
            for past_in in ort.llm_decode.io.inputs.iter().skip(1) {
                let mut v =
                    ort_take_kv_for_past_input(&mut prefill_out, "llm_prefill", past_in.as_str())?;
                if let Some(keep) = kv_keep {
                    v = ort_kv_cache_keep_last_f32(v, keep)?;
                }
                past.push(v);
            }

            if cosyvoice_debug_log_enabled() {
                cosyvoice_debug_log(format_args!(
                    "[cosyvoice][llm] DecodeGraph: past_count={} kv_keep={:?}\n",
                    past.len(),
                    kv_keep
                ));
                if let Some(first) = past.first() {
                    if let Some(shape) = ort_try_extract_tensor_shape_f32(first) {
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][llm] past[0] shape={shape:?}\n"
                        ));
                    } else {
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][llm] past[0] shape=<unavailable>\n"
                        ));
                    }
                }
            }

            let mut decoded: Vec<i64> = Vec::new();
            let mut decoded_u32: Vec<u32> = Vec::new();
            ensure_stop_token_reachable(stop_start, last_logits.len(), "llm_prefill(ort)")?;
            let log_every = cosyvoice_debug_log_every();
            while decoded.len() < max_len {
                if let Some(c) = cancel {
                    if c.load(Ordering::Relaxed) {
                        return Err(TtsError::Canceled);
                    }
                }

                // Optional guard: avoid stop tokens too early.
                let mut step_scores = last_logits.clone();
                if decoded.len() < min_len {
                    let start = stop_start.max(0) as usize;
                    for s in step_scores.iter_mut().skip(start) {
                        *s = f32::NEG_INFINITY;
                    }
                    if !step_scores.iter().any(|x| x.is_finite()) {
                        return Err(TtsError::Onnx(
                            "LLM logits are all -inf after min_len stop-token masking".into(),
                        ));
                    }
                }

                let token_u32 = sample_ras_next(&step_scores, &decoded_u32, sampling, rng)?;
                let token = token_u32 as i64;
                if token >= stop_start {
                    break;
                }
                decoded.push(token);
                decoded_u32.push(token_u32);

                // Decode next step.
                let token_name = ort
                    .llm_decode
                    .io
                    .inputs
                    .get(0)
                    .map(|s| s.as_str())
                    .unwrap_or("token_id");
                let logits_name = ort
                    .llm_decode
                    .io
                    .outputs
                    .get(0)
                    .map(|s| s.as_str())
                    .unwrap_or("logits");

                let token_t = Tensor::<i64>::from_array((
                    vec![1usize, 1usize],
                    vec![token].into_boxed_slice(),
                ))
                .map_err(|e| TtsError::Onnx(format!("ort: create token_id failed: {e}")))?;

                let mut inputs: Vec<(&str, DynValue)> = Vec::with_capacity(1 + past.len());
                inputs.push((token_name, token_t.into_dyn()));

                // past_* inputs are in model order; reuse the last step's outputs (present_*) as next step's past_*.
                for (name, v) in ort
                    .llm_decode
                    .io
                    .inputs
                    .iter()
                    .skip(1)
                    .zip(past.into_iter())
                {
                    inputs.push((name.as_str(), v));
                }

                let mut out = ort
                    .llm_decode
                    .session
                    .run(inputs)
                    .map_err(|e| TtsError::Onnx(format!("ort: llm_decode run failed: {e}")))?;

                let logits_v = out.remove(logits_name).ok_or_else(|| {
                    TtsError::Onnx("ort: llm_decode missing logits output".into())
                })?;
                let log_this_step = cosyvoice_debug_log_enabled()
                    && (decoded.len() <= 3 || decoded.len() % log_every == 0);
                last_logits = ort_extract_last_logits(&logits_v, "llm_decode", log_this_step)?;

                // decode 的 present_* 输出也按 “下一步的 past_* 输入” 顺序对齐。
                let mut new_past: Vec<DynValue> =
                    Vec::with_capacity(ort.llm_decode.io.inputs.len().saturating_sub(1));
                for past_in in ort.llm_decode.io.inputs.iter().skip(1) {
                    let mut v = ort_take_kv_for_past_input(
                        &mut out,
                        "llm_decode",
                        past_in.as_str(),
                    )?;
                    if let Some(keep) = kv_keep {
                        v = ort_kv_cache_keep_last_f32(v, keep)?;
                    }
                    new_past.push(v);
                }
                past = new_past;

                if log_this_step {
                    cosyvoice_debug_log(format_args!(
                        "[cosyvoice][llm][decode] step={} token={} stop_start={} logits_vocab={}\n",
                        decoded.len(),
                        token,
                        stop_start,
                        last_logits.len()
                    ));
                    if let Some(first) = past.first() {
                        if let Some(shape) = ort_try_extract_tensor_shape_f32(first) {
                            cosyvoice_debug_log(format_args!(
                                "[cosyvoice][llm][decode] past[0] shape={shape:?}\n"
                            ));
                        }
                    }
                }
            }

            if decoded.len() >= max_len {
                return Err(TtsError::Onnx(format!(
                    "LLM reached max_len={max_len} without emitting stop token (stopTokenStart={stop_start}, logits_vocab_size={}). This usually means pack.json stopTokenStart is wrong or the exported LLM has no stop token.",
                    last_logits.len()
                )));
            }
            cosyvoice_debug_log(format_args!(
                "[cosyvoice][llm] end(DecodeGraph): speech_tokens_len={}\n",
                decoded.len()
            ));
            if cosyvoice_debug_log_enabled() {
                let speech_size = self.pack.cfg.speech_token_size as i64;
                if decoded.is_empty() {
                    cosyvoice_debug_log(format_args!(
                        "[cosyvoice][llm] speech_tokens stats: <empty>\n"
                    ));
                } else {
                    let mut min_tok = i64::MAX;
                    let mut max_tok = i64::MIN;
                    let mut out_of_range = 0usize;
                    for &t in &decoded {
                        min_tok = min_tok.min(t);
                        max_tok = max_tok.max(t);
                        if t < 0 || (speech_size > 0 && t >= speech_size) {
                            out_of_range += 1;
                        }
                    }
                    let head_len = decoded.len().min(16);
                    let tail_len = decoded.len().min(16);
                    let head = &decoded[..head_len];
                    let tail = &decoded[decoded.len().saturating_sub(tail_len)..];
                    cosyvoice_debug_log(format_args!(
                        "[cosyvoice][llm] speech_tokens stats: min={min_tok} max={max_tok} speechTokenSize={speech_size} out_of_range={out_of_range}\n"
                    ));
                    cosyvoice_debug_log(format_args!(
                        "[cosyvoice][llm] speech_tokens head={head:?} tail={tail:?}\n"
                    ));
                }
            }
            return Ok(LlmGenerateResult {
                speech_tokens: decoded,
                logits_vocab_size: last_logits.len(),
            });
        }

        #[cfg(feature = "onnx-tract")]
        if let Some(tract) = &self.tract {
            use tract_onnx::prelude::*;

            let input = tensor1(input_ids)
                .into_shape(&[1, input_ids.len()])
                .map_err(|e| TtsError::Onnx(format!("reshape input_ids failed: {e}")))?;
            let prefill_out = tract
                .llm_prefill
                .run(tvec!(input.into()))
                .map_err(|e| TtsError::Onnx(format!("llm_prefill run failed: {e}")))?;

            if prefill_out.is_empty() {
                return Err(TtsError::Onnx("llm_prefill returned no outputs".into()));
            }

            let mut past: TVec<TValue> = prefill_out.iter().skip(1).cloned().collect();
            let mut last_logits = extract_last_logits(&prefill_out[0])?;

            let mut decoded: Vec<i64> = Vec::new();
            let mut decoded_u32: Vec<u32> = Vec::new();
            while decoded.len() < max_len {
                if let Some(c) = cancel {
                    if c.load(Ordering::Relaxed) {
                        return Err(TtsError::Canceled);
                    }
                }

                // Optional guard: avoid stop tokens too early.
                let mut step_scores = last_logits.clone();
                if decoded.len() < min_len {
                    let start = stop_start.max(0) as usize;
                    for s in step_scores.iter_mut().skip(start) {
                        *s = f32::NEG_INFINITY;
                    }
                    if !step_scores.iter().any(|x| x.is_finite()) {
                        return Err(TtsError::Onnx(
                            "LLM logits are all -inf after min_len stop-token masking".into(),
                        ));
                    }
                }

                let token_u32 = sample_ras_next(&step_scores, &decoded_u32, sampling, rng)?;
                let token = token_u32 as i64;
                if token >= stop_start {
                    break;
                }
                decoded.push(token);
                decoded_u32.push(token_u32);

                // Decode next step.
                let token_t = tensor1(&[token])
                    .into_shape(&[1, 1])
                    .map_err(|e| TtsError::Onnx(format!("reshape decode token failed: {e}")))?;
                let mut inputs: TVec<TValue> = tvec!(token_t.into());
                inputs.extend(past.iter().cloned());
                let out = tract
                    .llm_decode
                    .run(inputs)
                    .map_err(|e| TtsError::Onnx(format!("llm_decode run failed: {e}")))?;
                if out.is_empty() {
                    return Err(TtsError::Onnx("llm_decode returned no outputs".into()));
                }
                last_logits = extract_last_logits(&out[0])?;
                past = out.iter().skip(1).cloned().collect();
            }
            if decoded.len() >= max_len {
                return Err(TtsError::Onnx(format!(
                    "LLM reached max_len={max_len} without emitting stop token (stopTokenStart={stop_start}, logits_vocab_size={}). This usually means pack.json stopTokenStart is wrong or the exported LLM has no stop token.",
                    last_logits.len()
                )));
            }
            return Ok(LlmGenerateResult {
                speech_tokens: decoded,
                logits_vocab_size: last_logits.len(),
            });
        }

        Err(TtsError::NotImplemented("onnx backend is not enabled"))
    }

    fn flow_tokens_to_mel(
        &self,
        speech_tokens: &[i64],
        spk_embedding: &[f32],
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<f32>, TtsError> {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(TtsError::Canceled);
            }
        }

        #[cfg(feature = "onnx-ort")]
        if let Some(ort) = &self.ort {
            use ort::value::Tensor;

            let tok_name = ort
                .flow_infer
                .io
                .inputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("speech_tokens");
            let emb_name = ort
                .flow_infer
                .io
                .inputs
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or("spk_embedding");
            let mel_name = ort
                .flow_infer
                .io
                .outputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("mel");

            if cosyvoice_debug_log_enabled() {
                let speech_size = self.pack.cfg.speech_token_size as i64;
                if speech_tokens.is_empty() {
                    cosyvoice_debug_log(format_args!(
                        "[cosyvoice][flow] begin: speech_tokens_len=0\n"
                    ));
                } else {
                    let mut min_tok = i64::MAX;
                    let mut max_tok = i64::MIN;
                    let mut out_of_range = 0usize;
                    for &t in speech_tokens {
                        min_tok = min_tok.min(t);
                        max_tok = max_tok.max(t);
                        if t < 0 || (speech_size > 0 && t >= speech_size) {
                            out_of_range += 1;
                        }
                    }
                    cosyvoice_debug_log(format_args!(
                        "[cosyvoice][flow] begin: speech_tokens_len={} min={min_tok} max={max_tok} speechTokenSize={speech_size} out_of_range={out_of_range}\n",
                        speech_tokens.len()
                    ));
                }
            }

            // 有些 flow 图只能吃固定长度的 token（例如 token_len=256 => mel_len=512），此时需要分块拼接 mel。
            // 说明：有些导出会把 input shape 标成动态，但图内部仍用常量 256 做广播/逐点乘，导致“看起来支持变长但实际会炸”。
            let run_chunked = |chunk_len: usize| -> Result<Vec<f32>, TtsError> {
                if chunk_len == 0 {
                    return Err(TtsError::Onnx("ort: invalid flow_token_chunk_len=0".into()));
                }

                let orig_tokens_len = speech_tokens.len();
                let mut tokens: Vec<i64> = speech_tokens.to_vec();
                if tokens.is_empty() {
                    return Err(TtsError::Onnx("speech_tokens is empty".into()));
                }

                // 经验：flow 固定长度 chunk 推理若“硬切块”会更容易出现高频嗡嗡声、边界不连贯、局部拉长音。
                // 这里用滑动窗口 + overlap（token 级）并在 mel 级做 cross-fade 来平滑拼接。
                //
                // - 默认 overlap=32 tokens（可用 env 覆盖）。
                // - hop = chunk_len - overlap（确保 >0）。
                let overlap_tokens = std::env::var("CHAOS_COSYVOICE_ORT_FLOW_CHUNK_OVERLAP_TOKENS")
                    .ok()
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(32)
                    .min(chunk_len.saturating_sub(1));
                let hop = chunk_len.saturating_sub(overlap_tokens).max(1);

                // 让最后一窗至少覆盖到最后一个 token；不足的用最后一个 token padding。
                let last_start = if orig_tokens_len <= chunk_len {
                    0usize
                } else {
                    ((orig_tokens_len - chunk_len) + hop - 1) / hop * hop
                };
                let need_len = last_start + chunk_len;
                if tokens.len() < need_len {
                    let pad = need_len - tokens.len();
                    let last = *tokens.last().unwrap_or(&0);
                    tokens.extend(std::iter::repeat(last).take(pad));
                }

                let channels = 80usize;
                let ratio = self.pack.cfg.token_mel_ratio.max(1) as usize;
                let overlap_frames = overlap_tokens.saturating_mul(ratio);

                let mut mel_ch: Vec<Vec<f32>> =
                    (0..channels).map(|_| Vec::<f32>::new()).collect();
                let mut windows = 0usize;

                let mut start = 0usize;
                while start <= last_start {
                    if let Some(c) = cancel {
                        if c.load(Ordering::Relaxed) {
                            return Err(TtsError::Canceled);
                        }
                    }
                    windows += 1;
                    let chunk = &tokens[start..start + chunk_len];

                    let tok_t = Tensor::<i64>::from_array((
                        vec![1usize, chunk.len()],
                        chunk.to_vec().into_boxed_slice(),
                    ))
                    .map_err(|e| {
                        TtsError::Onnx(format!("ort: create speech_tokens chunk failed: {e}"))
                    })?;
                    let emb_t = Tensor::<f32>::from_array((
                        vec![1usize, spk_embedding.len()],
                        spk_embedding.to_vec().into_boxed_slice(),
                    ))
                    .map_err(|e| {
                        TtsError::Onnx(format!("ort: create spk_embedding chunk failed: {e}"))
                    })?;

                    let mut out = ort
                        .flow_infer
                        .session
                        .run(vec![
                            (tok_name, tok_t.into_dyn()),
                            (emb_name, emb_t.into_dyn()),
                        ])
                        .map_err(|e| TtsError::Onnx(format!("ort: flow_infer run failed: {e}")))?;

                    let mel_v = out.remove(mel_name).ok_or_else(|| {
                        TtsError::Onnx("ort: flow_infer missing mel output".into())
                    })?;
                    let mel_t: ort::value::Tensor<f32> = mel_v
                        .downcast()
                        .map_err(|e| TtsError::Onnx(format!("ort: mel is not f32 tensor: {e}")))?;
                    let (shape, data) = mel_t.extract_raw_tensor();
                    // 兼容不同导出：flow 图的 mel 可能是 [1, 80, T] 或 [1, T, 80]。
                    // 引擎内部统一转成 [1, 80, T] 的 flatten（按通道连续），方便拼接与送入 HiFT。
                    let mel_vec = if shape.len() == 3 && shape.get(0).copied().unwrap_or(0) == 1 {
                        let d1 = shape[1] as usize;
                        let d2 = shape[2] as usize;
                        if d1 == 80 {
                            data.to_vec()
                        } else if d2 == 80 {
                            let t = d1;
                            let channels = 80usize;
                            if data.len() != t * channels {
                                return Err(TtsError::Onnx(format!(
                                    "ort: flow mel data len mismatch: shape={shape:?} data_len={}",
                                    data.len()
                                )));
                            }
                            let mut out = vec![0.0f32; channels * t];
                            for ti in 0..t {
                                for ch in 0..channels {
                                    let src = ti * channels + ch; // [T, 80]
                                    let dst = ch * t + ti; // [80, T]
                                    out[dst] = data[src];
                                }
                            }
                            out
                        } else {
                            return Err(TtsError::Onnx(format!(
                                "ort: flow mel shape unsupported: {shape:?} (expected [1,80,T] or [1,T,80])"
                            )));
                        }
                    } else {
                        data.to_vec()
                    };

                    if mel_vec.len() % channels != 0 {
                        return Err(TtsError::Onnx(format!(
                            "ort: flow mel chunk len {} is not divisible by 80",
                            mel_vec.len()
                        )));
                    }
                    let t_each = mel_vec.len() / channels;
                    let ov = overlap_frames.min(t_each.saturating_sub(1));

                    for ch in 0..channels {
                        let src = &mel_vec[ch * t_each..ch * t_each + t_each];
                        let dst = &mut mel_ch[ch];
                        if dst.is_empty() {
                            dst.extend_from_slice(src);
                            continue;
                        }
                        if ov > 0 && dst.len() >= ov {
                            let base = dst.len() - ov;
                            // 线性 cross-fade：让边界更平滑，降低“电流/嗡嗡”感。
                            for j in 0..ov {
                                let w = if ov <= 1 {
                                    1.0
                                } else {
                                    (j as f32) / ((ov - 1) as f32)
                                };
                                dst[base + j] = dst[base + j] * (1.0 - w) + src[j] * w;
                            }
                            dst.extend_from_slice(&src[ov..]);
                        } else {
                            // overlap 不可用则直接拼接（兜底）。
                            dst.extend_from_slice(src);
                        }
                    }

                    start = start.saturating_add(hop);
                }

                let total_t = mel_ch.get(0).map(|v| v.len()).unwrap_or(0);
                if total_t == 0 {
                    return Err(TtsError::Onnx("ort: flow produced empty mel".into()));
                }
                for ch in 1..channels {
                    if mel_ch[ch].len() != total_t {
                        return Err(TtsError::Onnx(format!(
                            "ort: flow mel channel time mismatch: ch0={total_t} ch{ch}={}",
                            mel_ch[ch].len()
                        )));
                    }
                }
                let mut out = vec![0.0f32; channels * total_t];
                for ch in 0..channels {
                    out[ch * total_t..ch * total_t + total_t].copy_from_slice(&mel_ch[ch]);
                }

                // 若因补齐 token 做了 padding，则把末尾多出来的 mel 帧裁掉，避免尾音出现明显“电流/重复”感。
                let expected_t = orig_tokens_len.saturating_mul(ratio).min(total_t);
                if expected_t < total_t {
                    let mut trimmed = vec![0.0f32; channels * expected_t];
                    for ch in 0..channels {
                        let src = &out[ch * total_t..ch * total_t + expected_t];
                        trimmed[ch * expected_t..ch * expected_t + expected_t].copy_from_slice(src);
                    }
                    if cosyvoice_debug_log_enabled() {
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][flow] chunked mel: chunk_len={chunk_len} overlap_tokens={overlap_tokens} hop={hop} windows={windows} channels=80 total_t={total_t} expected_t={expected_t} (trimmed)\n"
                        ));
                        cosyvoice_debug_log_f32_stats("flow.mel", &trimmed);
                    }
                    return Ok(trimmed);
                }

                if cosyvoice_debug_log_enabled() {
                    cosyvoice_debug_log(format_args!(
                        "[cosyvoice][flow] chunked mel: chunk_len={chunk_len} overlap_tokens={overlap_tokens} hop={hop} windows={windows} channels=80 total_t={total_t}\n"
                    ));
                    cosyvoice_debug_log_f32_stats("flow.mel", &out);
                }
                Ok(out)
            };

            if let Some(chunk_len) = ort.flow_token_chunk_len {
                return run_chunked(chunk_len);
            }

            fn infer_chunk_len_from_broadcast_err(msg: &str, input_len: usize) -> Option<usize> {
                // 典型错误："... Attempting to broadcast ... 135 by 256"
                let Some(idx) = msg.rfind("Attempting to broadcast") else {
                    return None;
                };
                let tail = &msg[idx..];
                let mut nums: Vec<usize> = Vec::new();
                for part in tail.split(|c: char| !c.is_ascii_digit()) {
                    if part.is_empty() {
                        continue;
                    }
                    if let Ok(v) = part.parse::<usize>() {
                        nums.push(v);
                    }
                }
                if nums.len() < 2 {
                    return None;
                }
                let a = nums[nums.len() - 2];
                let b = nums[nums.len() - 1];
                if a == input_len {
                    Some(b).filter(|&v| v > 0)
                } else if b == input_len {
                    Some(a).filter(|&v| v > 0)
                } else {
                    Some(a.max(b)).filter(|&v| v > 0)
                }
            }

            // 未能在 load 阶段推断 chunk_len：先尝试直接跑；若遇到广播报错，则从报错推断固定长度并回退到 chunk 模式。
            let tok_t = Tensor::<i64>::from_array((
                vec![1usize, speech_tokens.len()],
                speech_tokens.to_vec().into_boxed_slice(),
            ))
            .map_err(|e| TtsError::Onnx(format!("ort: create speech_tokens failed: {e}")))?;
            let emb_t = Tensor::<f32>::from_array((
                vec![1usize, spk_embedding.len()],
                spk_embedding.to_vec().into_boxed_slice(),
            ))
            .map_err(|e| TtsError::Onnx(format!("ort: create spk_embedding failed: {e}")))?;

            let mut out = match ort.flow_infer.session.run(vec![
                (tok_name, tok_t.into_dyn()),
                (emb_name, emb_t.into_dyn()),
            ]) {
                Ok(v) => v,
                Err(e) => {
                    let msg = e.to_string();
                    if let Some(chunk_len) =
                        infer_chunk_len_from_broadcast_err(&msg, speech_tokens.len())
                    {
                        tracing::warn!(
                            chunk_len,
                            "ort: flow_infer seems fixed-length; retrying with chunking (set CHAOS_COSYVOICE_ORT_FLOW_TOKEN_CHUNK_LEN to skip auto-detect)"
                        );
                        if cosyvoice_debug_log_enabled() {
                            cosyvoice_debug_log(format_args!(
                                "[cosyvoice][flow] direct run failed: {msg}\n"
                            ));
                            cosyvoice_debug_log(format_args!(
                                "[cosyvoice][flow] inferred chunk_len={chunk_len}; retrying chunked\n"
                            ));
                        }
                        return run_chunked(chunk_len);
                    }
                    return Err(TtsError::Onnx(format!("ort: flow_infer run failed: {msg}")));
                }
            };

            let mel_v = out
                .remove(mel_name)
                .ok_or_else(|| TtsError::Onnx("ort: flow_infer missing mel output".into()))?;
            let mel_t: ort::value::Tensor<f32> = mel_v
                .downcast()
                .map_err(|e| TtsError::Onnx(format!("ort: mel is not f32 tensor: {e}")))?;
            let (shape, data) = mel_t.extract_raw_tensor();
            // 兼容 [1,80,T] / [1,T,80]。
            if shape.len() == 3 && shape.get(0).copied().unwrap_or(0) == 1 {
                let d1 = shape[1] as usize;
                let d2 = shape[2] as usize;
                if d1 == 80 {
                    let mel = data.to_vec();
                    if cosyvoice_debug_log_enabled() {
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][flow] mel shape={shape:?}\n"
                        ));
                        cosyvoice_debug_log_f32_stats("flow.mel", &mel);
                    }
                    return Ok(mel);
                }
                if d2 == 80 {
                    let t = d1;
                    let channels = 80usize;
                    if data.len() != t * channels {
                        return Err(TtsError::Onnx(format!(
                            "ort: flow mel data len mismatch: shape={shape:?} data_len={}",
                            data.len()
                        )));
                    }
                    let mut out = vec![0.0f32; channels * t];
                    for ti in 0..t {
                        for ch in 0..channels {
                            let src = ti * channels + ch;
                            let dst = ch * t + ti;
                            out[dst] = data[src];
                        }
                    }
                    if cosyvoice_debug_log_enabled() {
                        cosyvoice_debug_log(format_args!(
                            "[cosyvoice][flow] mel shape={shape:?} (transposed)\n"
                        ));
                        cosyvoice_debug_log_f32_stats("flow.mel", &out);
                    }
                    return Ok(out);
                }
            }
            let mel = data.to_vec();
            if cosyvoice_debug_log_enabled() {
                cosyvoice_debug_log(format_args!(
                    "[cosyvoice][flow] mel shape={shape:?}\n"
                ));
                cosyvoice_debug_log_f32_stats("flow.mel", &mel);
            }
            return Ok(mel);
        }

        #[cfg(feature = "onnx-tract")]
        if let Some(tract) = &self.tract {
            use tract_onnx::prelude::*;
            let token_t = tensor1(speech_tokens)
                .into_shape(&[1, speech_tokens.len()])
                .map_err(|e| TtsError::Onnx(format!("reshape speech_tokens failed: {e}")))?;
            let emb_t = tensor1(spk_embedding)
                .into_shape(&[1, spk_embedding.len()])
                .map_err(|e| TtsError::Onnx(format!("reshape spk_embedding failed: {e}")))?;
            let out = tract
                .flow_infer
                .run(tvec!(token_t.into(), emb_t.into()))
                .map_err(|e| TtsError::Onnx(format!("flow_infer run failed: {e}")))?;
            if out.is_empty() {
                return Err(TtsError::Onnx("flow_infer returned no outputs".into()));
            }
            let mel = out[0]
                .to_array_view::<f32>()
                .map_err(|e| TtsError::Onnx(format!("flow output is not f32: {e}")))?;
            return Ok(mel.iter().copied().collect());
        }

        Err(TtsError::NotImplemented("onnx backend is not enabled"))
    }

    fn hift_mel_to_wav(
        &self,
        mel: &[f32],
        cancel: Option<&AtomicBool>,
    ) -> Result<Vec<f32>, TtsError> {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(TtsError::Canceled);
            }
        }
        // Expect mel layout [1, 80, T].
        if mel.len() % 80 != 0 {
            return Err(TtsError::InvalidArg(format!(
                "mel length {} is not divisible by 80",
                mel.len()
            )));
        }
        let t = mel.len() / 80;

        #[cfg(feature = "onnx-ort")]
        if let Some(ort) = &self.ort {
            use ort::value::Tensor;

            let in_name = ort
                .hift_infer
                .io
                .inputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("mel");
            let out_name = ort
                .hift_infer
                .io
                .outputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("wav");

            if cosyvoice_debug_log_enabled() {
                cosyvoice_debug_log(format_args!(
                    "[cosyvoice][hift] begin: mel_frames={t} mel_len={}\n",
                    mel.len()
                ));
                cosyvoice_debug_log_f32_stats("hift.mel", mel);
            }

            let mel_t = Tensor::<f32>::from_array((
                vec![1usize, 80usize, t],
                mel.to_vec().into_boxed_slice(),
            ))
            .map_err(|e| TtsError::Onnx(format!("ort: create mel failed: {e}")))?;

            let mut out = ort
                .hift_infer
                .session
                .run(vec![(in_name, mel_t.into_dyn())])
                .map_err(|e| TtsError::Onnx(format!("ort: hift_infer run failed: {e}")))?;

            let wav_v = out
                .remove(out_name)
                .ok_or_else(|| TtsError::Onnx("ort: hift_infer missing wav output".into()))?;
            let wav_t: ort::value::Tensor<f32> = wav_v
                .downcast()
                .map_err(|e| TtsError::Onnx(format!("ort: wav is not f32 tensor: {e}")))?;
            let (_shape, data) = wav_t.extract_raw_tensor();
            let wav = data.to_vec();
            if cosyvoice_debug_log_enabled() {
                cosyvoice_debug_log(format_args!(
                    "[cosyvoice][hift] wav_len={}\n",
                    wav.len()
                ));
                cosyvoice_debug_log_f32_stats("hift.wav", &wav);
            }
            return Ok(wav);
        }

        #[cfg(feature = "onnx-tract")]
        if let Some(tract) = &self.tract {
            use tract_onnx::prelude::*;
            let mel_t = tensor1(mel)
                .into_shape(&[1, 80, t])
                .map_err(|e| TtsError::Onnx(format!("reshape mel failed: {e}")))?;
            let out = tract
                .hift_infer
                .run(tvec!(mel_t.into()))
                .map_err(|e| TtsError::Onnx(format!("hift_infer run failed: {e}")))?;
            if out.is_empty() {
                return Err(TtsError::Onnx("hift_infer returned no outputs".into()));
            }
            let wav = out[0]
                .to_array_view::<f32>()
                .map_err(|e| TtsError::Onnx(format!("hift output is not f32: {e}")))?;
            return Ok(wav.iter().copied().collect());
        }

        let _ = (mel, t);
        Err(TtsError::NotImplemented("onnx backend is not enabled"))
    }
}

#[cfg(feature = "onnx-ort")]
fn load_onnx_ort_backend(pack: &CosyVoicePack) -> Result<OrtBackend, TtsError> {
    use std::collections::HashSet;

    use crate::tts::cosyvoice::pack::PackOnnxIo;
    use ort::session::Session;
    use ort::value::Tensor;

    fn configure_session_builder(
        kind: &'static str,
        builder: ort::session::builder::SessionBuilder,
    ) -> Result<ort::session::builder::SessionBuilder, TtsError> {
        // 说明：这里在库内部选择 EP 是“有争议的”，但为了本仓库的开箱即用（daemon / FFI / apps）我们提供一个
        // 可配置的默认行为：
        // - 默认：CPU
        // - 若启用 feature `onnx-ort-cuda`：默认尝试 CUDA（失败则回落 CPU）
        // - 通过环境变量 `CHAOS_ORT_EP=cpu|cuda|auto` 覆盖
        //
        // 对最终应用来说，也可以在更上层自己创建 Session 并注入。
        static LOG_ONCE: OnceLock<()> = OnceLock::new();
        let ep_debug = std::env::var("CHAOS_ORT_EP_DEBUG").ok().as_deref() == Some("1");

        let ep = std::env::var("CHAOS_ORT_EP")
            .ok()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let ep = ep.trim().to_string();

        let prefer_cuda = match ep.as_str() {
            "" | "auto" => cfg!(feature = "onnx-ort-cuda"),
            "cpu" => false,
            "cuda" => true,
            other => {
                // 不阻断推理：给出提示后按默认策略走。
                tracing::warn!(value = other, "unknown CHAOS_ORT_EP; expected cpu|cuda|auto");
                cfg!(feature = "onnx-ort-cuda")
            }
        };

        if !prefer_cuda {
            LOG_ONCE.get_or_init(|| {
                tracing::info!("ort execution provider: CPU");
            });
            if ep_debug {
                eprintln!("[ort] EP=CPU (kind={kind})");
            }
            return Ok(builder);
        }

        #[cfg(feature = "onnx-ort-cuda")]
        {
            use ort::execution_providers::CUDAExecutionProvider;
            use ort::execution_providers::ExecutionProvider;

            let mut cuda = CUDAExecutionProvider::default();
            if let Ok(v) = std::env::var("CHAOS_ORT_CUDA_DEVICE_ID") {
                if let Ok(id) = v.trim().parse::<i32>() {
                    cuda = cuda.with_device_id(id);
                } else {
                    tracing::warn!(value = v, "invalid CHAOS_ORT_CUDA_DEVICE_ID; expected int");
                }
            }

            // 先判断“编译是否包含 CUDA EP”。运行期依赖（cudart/cudnn）缺失仍可能导致 register 失败，下面会回落。
            let cuda_avail = cuda
                .is_available()
                .map_err(|e| TtsError::Onnx(format!("ort: check CUDA EP availability failed: {e}")))?;
            if !cuda_avail {
                LOG_ONCE.get_or_init(|| {
                    tracing::info!("ort execution provider: CUDA not available (compiled without CUDA); falling back to CPU");
                });
                if ep_debug {
                    eprintln!("[ort] EP=CUDA requested but not available; fallback=CPU (kind={kind})");
                }
                return Ok(builder);
            }

            // CUDA + CPU 兜底。
            let cpu_fallback = builder.clone();
            match builder.with_execution_providers([
                cuda.build(),
                ort::execution_providers::CPUExecutionProvider::default().build(),
            ]) {
                Ok(b) => {
                    LOG_ONCE.get_or_init(|| {
                        tracing::info!("ort execution provider: CUDA (with CPU fallback)");
                    });
                    if ep_debug {
                        eprintln!("[ort] EP=CUDA enabled (kind={kind})");
                    }
                    Ok(b)
                }
                Err(e) => {
                    // 常见原因：缺少 CUDA runtime / cuDNN DLL，或者 GPU 环境不可用。
                    tracing::warn!(
                        kind,
                        err = %e,
                        "ort: enable CUDA EP failed; falling back to CPU. (Hint: ensure CUDA runtime + cuDNN are in PATH)"
                    );
                    if ep_debug {
                        eprintln!("[ort] EP=CUDA enable failed; fallback=CPU (kind={kind}) err={e}");
                    }
                    Ok(cpu_fallback)
                }
            }
        }

        #[cfg(not(feature = "onnx-ort-cuda"))]
        {
            tracing::warn!(
                kind,
                "CHAOS_ORT_EP=cuda requested but feature `onnx-ort-cuda` is not enabled; falling back to CPU"
            );
            Ok(builder)
        }
    }

    fn load_model(
        kind: &'static str,
        path: std::path::PathBuf,
        io_override: Option<&PackOnnxIo>,
        prefer_logits_first: bool,
    ) -> Result<OrtModel, TtsError> {
        let builder = Session::builder()
            .map_err(|e| TtsError::Onnx(format!("ort: build {kind} session builder failed: {e}")))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| {
                TtsError::Onnx(format!("ort: set {kind} graph optimization level failed: {e}"))
            })?;
        let builder = configure_session_builder(kind, builder)?;

        let session = builder
            .commit_from_file(&path)
            .map_err(|e| {
                TtsError::Onnx(format!(
                    "ort: load {kind} onnx from {} failed: {e}",
                    path.display()
                ))
            })?;

        let session_inputs: Vec<String> = session.inputs.iter().map(|x| x.name.clone()).collect();
        let session_outputs: Vec<String> = session.outputs.iter().map(|x| x.name.clone()).collect();

        let input_set: HashSet<&str> = session_inputs.iter().map(|s| s.as_str()).collect();
        let output_set: HashSet<&str> = session_outputs.iter().map(|s| s.as_str()).collect();

        let inputs = io_override
            .map(|x| x.inputs.clone())
            .unwrap_or_else(|| session_inputs.clone());
        let mut outputs = io_override
            .map(|x| x.outputs.clone())
            .unwrap_or_else(|| session_outputs.clone());

        // 兼容：有些导出会把 logits 放在最后；为了复用统一的 “outputs[0]=logits, outputs[1..]=past” 逻辑，
        // 我们在没有 pack.json 显式 io mapping 时做一次保守的排序修正。
        if prefer_logits_first && io_override.is_none() {
            if let Some(idx) = outputs.iter().position(|s| {
                s.eq_ignore_ascii_case("logits") || s.to_ascii_lowercase().contains("logits")
            }) {
                if idx != 0 {
                    let logits = outputs.remove(idx);
                    outputs.insert(0, logits);
                }
            }
        }

        if let Some(io) = io_override {
            for name in &io.inputs {
                if !input_set.contains(name.as_str()) {
                    return Err(TtsError::Onnx(format!(
                        "ort: {kind} pack io input name {name:?} not found in model inputs={session_inputs:?} (path={})",
                        path.display()
                    )));
                }
            }
            for name in &io.outputs {
                if !output_set.contains(name.as_str()) {
                    return Err(TtsError::Onnx(format!(
                        "ort: {kind} pack io output name {name:?} not found in model outputs={session_outputs:?} (path={})",
                        path.display()
                    )));
                }
            }
        }

        if outputs.is_empty() {
            return Err(TtsError::Onnx(format!(
                "ort: {kind} session has no outputs (path={})",
                path.display()
            )));
        }

        Ok(OrtModel {
            session,
            io: OrtOnnxIo { inputs, outputs },
        })
    }

    tracing::debug!(
        model_dir = %pack.model_dir.display(),
        "loading cosyvoice ort backend"
    );

    let llm_prefill = load_model(
        "llm_prefill",
        pack.path_llm_prefill(),
        pack.cfg.llm.prefill_io.as_ref(),
        true,
    )?;
    let llm_decode = load_model(
        "llm_decode",
        pack.path_llm_decode(),
        pack.cfg.llm.decode_io.as_ref(),
        true,
    )?;
    let flow_infer = load_model(
        "flow_infer",
        pack.path_flow_infer(),
        pack.cfg.flow_io.as_ref(),
        false,
    )?;
    let hift_infer = load_model(
        "hift_infer",
        pack.path_hift_infer(),
        pack.cfg.hift_io.as_ref(),
        false,
    )?;

    // 一些 pack 的 llm_decode 图在常规 past_len 下会直接 shape mismatch（例如注意力 mask 维度错误）。
    //
    // 旧策略：为了“能跑起来”，自动裁剪 KV cache 强行让图满足固定长度假设；但这会显著损失质量（上下文变短，容易出现电音/短句）。
    // 新策略（默认）：auto smoke 失败时，优先切到 PrefillOnly（更慢但更接近 Python 质量）。如需强制走 decode 图，可设置：
    // - `CHAOS_COSYVOICE_ORT_LLM_DECODE_MODE=decode`
    // - 或 `CHAOS_COSYVOICE_ORT_KV_CACHE_KEEP=...`（仍可能影响质量）
    let forced_mode = std::env::var("CHAOS_COSYVOICE_ORT_LLM_DECODE_MODE")
        .ok()
        .unwrap_or_default()
        .to_ascii_lowercase();

    let mut llm_decode_mode = OrtLlmDecodeMode::DecodeGraph;
    let mut kv_cache_keep: Option<usize> = None;

    if forced_mode == "prefill" || forced_mode == "prefill_only" {
        llm_decode_mode = OrtLlmDecodeMode::PrefillOnly;
    } else if forced_mode == "decode" {
        llm_decode_mode = OrtLlmDecodeMode::DecodeGraph;
    } else {
        fn infer_keep_from_decode_err(msg: &str) -> Option<usize> {
            // 典型错误："... Attempting to broadcast ... 4 by 70"
            let Some(idx) = msg.rfind("Attempting to broadcast") else {
                return None;
            };
            let tail = &msg[idx..];
            let mut nums: Vec<usize> = Vec::new();
            for part in tail.split(|c: char| !c.is_ascii_digit()) {
                if part.is_empty() {
                    continue;
                }
                if let Ok(v) = part.parse::<usize>() {
                    nums.push(v);
                }
            }
            if nums.len() < 2 {
                return None;
            }
            let a = nums[nums.len() - 2];
            let b = nums[nums.len() - 1];
            let total = a.min(b);
            // decode 图里常见是 past_len_plus_1 参与广播，所以 keep = total-1
            total.checked_sub(1).filter(|&x| x > 0)
        }

        // auto：用较长上下文跑一次 prefill->decode，若失败则启用 KV cache 裁剪。
        let eop = pack.cfg.end_of_prompt_token_id as i64;
        let input_ids = vec![eop; 32];

        let in_name = llm_prefill
            .io
            .inputs
            .get(0)
            .map(|s| s.as_str())
            .unwrap_or("input_ids");
        let out_logits_name = llm_prefill
            .io
            .outputs
            .get(0)
            .map(|s| s.as_str())
            .unwrap_or("logits");

        let input = Tensor::<i64>::from_array((
            vec![1usize, input_ids.len()],
            input_ids.into_boxed_slice(),
        ))
        .map_err(|e| TtsError::Onnx(format!("ort: build llm_prefill smoke input failed: {e}")))?;

        let mut prefill_out = llm_prefill
            .session
            .run(vec![(in_name, input.into_dyn())])
            .map_err(|e| TtsError::Onnx(format!("ort: llm_prefill smoke run failed: {e}")))?;
        let _ = prefill_out.remove(out_logits_name);

        let token_name = llm_decode
            .io
            .inputs
            .get(0)
            .map(|s| s.as_str())
            .unwrap_or("token_id");
        let token_id = 0i64;
        let token_t = Tensor::<i64>::from_array((
            vec![1usize, 1usize],
            vec![token_id].into_boxed_slice(),
        ))
        .map_err(|e| TtsError::Onnx(format!("ort: build llm_decode smoke token_id failed: {e}")))?;

        let mut inputs: Vec<(&str, ort::value::DynValue)> =
            Vec::with_capacity(llm_decode.io.inputs.len());
        inputs.push((token_name, token_t.into_dyn()));
        let mut smoke_ok = true;
        for past_in in llm_decode.io.inputs.iter().skip(1) {
            match ort_take_kv_for_past_input(&mut prefill_out, "llm_prefill(smoke)", past_in) {
                Ok(v) => inputs.push((past_in.as_str(), v)),
                Err(e) => {
                    // 说明：smoke test 只是“可选的启发式探测”。如果这里对不齐 KV cache 的名字，
                    // 不应该因此强行启用 kv_cache_keep（容易误判，导致速度/质量双杀）。
                    tracing::debug!(
                        err = %e,
                        "ort: llm_decode smoke test skipped (cannot align prefill KV outputs to decode past inputs)"
                    );
                    smoke_ok = false;
                    break;
                }
            }
        }

        if smoke_ok && inputs.len() == llm_decode.io.inputs.len() {
            if let Err(e) = llm_decode.session.run(inputs) {
                kv_cache_keep = infer_keep_from_decode_err(&e.to_string());
                if let Some(keep) = kv_cache_keep {
                    // 经验阈值：keep 很小（例如 3）意味着上下文会被裁到几乎没有，质量往往“电音/短句”。
                    // 此时宁可回退到 PrefillOnly（慢但更稳）。keep 较大时，仍允许用户先用 KV trim 跑通做 sanity check。
                    if keep < 64 {
                        tracing::warn!(
                            keep,
                            "ort: llm_decode graph seems fixed-length / incompatible; inferred kv_keep is too small, falling back to PrefillOnly for quality. Consider re-exporting ONNX pack."
                        );
                        llm_decode_mode = OrtLlmDecodeMode::PrefillOnly;
                        kv_cache_keep = None;
                    } else {
                        tracing::warn!(
                            keep,
                            "ort: llm_decode graph seems fixed-length; enabling KV cache trimming (may reduce quality). Consider re-exporting ONNX pack or set CHAOS_COSYVOICE_ORT_LLM_DECODE_MODE=prefill."
                        );
                    }
                } else {
                    tracing::warn!(
                        err = %e,
                        "ort: llm_decode smoke run failed, but could not infer a safe kv_cache_keep; falling back to PrefillOnly. Consider re-exporting ONNX pack."
                    );
                    llm_decode_mode = OrtLlmDecodeMode::PrefillOnly;
                }
            }
        }
    }

    // 手动覆盖（更快，但可能电音/短句）：
    // - `CHAOS_COSYVOICE_ORT_KV_CACHE_KEEP=3`
    // - 也可以配合 `CHAOS_COSYVOICE_ORT_LLM_DECODE_MODE=decode` 强制走 decode 图
    if llm_decode_mode == OrtLlmDecodeMode::DecodeGraph {
        if let Ok(raw) = std::env::var("CHAOS_COSYVOICE_ORT_KV_CACHE_KEEP") {
            let raw = raw.trim();
            if !raw.is_empty() {
                match raw.parse::<usize>() {
                    Ok(v) => {
                        // 允许显式设置 0：禁用 KV trim（用于排查 smoke 误判 / 性能问题）。
                        kv_cache_keep = Some(v);
                    }
                    Err(_) => {
                        tracing::warn!(
                            value = raw,
                            "ort: invalid CHAOS_COSYVOICE_ORT_KV_CACHE_KEEP; expected an integer"
                        );
                    }
                }
            }
        }
    }

    // flow_infer：有些 pack 会把 flow 图导出成“固定 token_len -> 固定 mel_len”的形式（便于分块拼接）。
    // 通过输出 mel 的静态 shape 推断 chunk 长度，避免在 load 阶段做大量试跑。
    let flow_token_chunk_len = std::env::var("CHAOS_COSYVOICE_ORT_FLOW_TOKEN_CHUNK_LEN")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&v| v > 0)
        .or_else(|| {
            use ort::value::ValueType;

            // 若 flow 图的 speech_tokens 输入是固定长度（例如 [1, 256]），优先从 input shape 推断 chunk_len。
            // 一些导出会把 mel 的时间维导出成动态 -1，导致“从输出 shape 推断”失效。
            let tok_name = flow_infer
                .io
                .inputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("speech_tokens");
            if let Some(inp) = flow_infer.session.inputs.iter().find(|i| i.name == tok_name) {
                if let ValueType::Tensor { dimensions, .. } = &inp.input_type {
                    if dimensions.len() == 2 {
                        let tok_len = dimensions[1];
                        if tok_len > 0 {
                            return Some(tok_len as usize);
                        }
                    }
                }
            }

            let mel_name = flow_infer
                .io
                .outputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("mel");

            let out = flow_infer.session.outputs.iter().find(|o| o.name == mel_name)?;
            let ValueType::Tensor { dimensions, .. } = &out.output_type else {
                return None;
            };
            if dimensions.len() != 3 {
                return None;
            }
            // 兼容：flow 输出可能是 [1,80,T] 或 [1,T,80]；这里用“哪个维度等于 80”来判定时间维。
            let d1 = dimensions[1];
            let d2 = dimensions[2];
            let mel_t = if d1 == 80 { d2 } else if d2 == 80 { d1 } else { return None };
            if mel_t <= 0 {
                return None;
            }

            let ratio = pack.cfg.token_mel_ratio.max(1) as i64;
            if mel_t % ratio != 0 {
                return None;
            }
            let chunk = (mel_t / ratio) as usize;
            if chunk == 0 {
                return None;
            }

            // 轻量确认：用全 0 embedding + 简单 token 序列跑一次。
            let tok_name = flow_infer
                .io
                .inputs
                .get(0)
                .map(|s| s.as_str())
                .unwrap_or("speech_tokens");
            let emb_name = flow_infer
                .io
                .inputs
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or("spk_embedding");

            let speech_tokens: Vec<i64> = (0..chunk as i64).collect();
            let spk_dim = pack.cfg.spk_embed_dim.max(1) as usize;
            let spk_embedding = vec![0.0f32; spk_dim];

            let tok_t = Tensor::<i64>::from_array((
                vec![1usize, speech_tokens.len()],
                speech_tokens.into_boxed_slice(),
            ))
            .ok()?;
            let emb_t = Tensor::<f32>::from_array((
                vec![1usize, spk_embedding.len()],
                spk_embedding.into_boxed_slice(),
            ))
            .ok()?;

            let mut out = flow_infer
                .session
                .run(vec![(tok_name, tok_t.into_dyn()), (emb_name, emb_t.into_dyn())])
                .ok()?;
            let _ = out.remove(mel_name)?;

            Some(chunk)
        });

    Ok(OrtBackend {
        llm_prefill,
        llm_decode,
        flow_infer,
        hift_infer,
        llm_decode_mode,
        kv_cache_keep,
        flow_token_chunk_len,
    })
}

#[cfg(feature = "onnx-ort")]
fn ort_extract_last_logits(
    v: &ort::value::DynValue,
    stage: &'static str,
    log_shape: bool,
) -> Result<Vec<f32>, TtsError> {
    let (shape, data) = v
        .try_extract_raw_tensor::<f32>()
        .map_err(|e| TtsError::Onnx(format!("ort: {stage} logits output is not a CPU f32 tensor: {e}")))?;

    if log_shape && cosyvoice_debug_log_enabled() {
        cosyvoice_debug_log(format_args!(
            "[cosyvoice][tensor] {stage} logits shape={shape:?} data_len={}\n",
            data.len()
        ));
    }

    // Accept [1, vocab] or [1, seq, vocab] or [seq, vocab] etc.
    let last: Vec<f32> = match shape.len() {
        1 => Ok(data.to_vec()),
        2 => {
            let rows = shape[0].max(0) as usize;
            let cols = shape[1].max(0) as usize;
            if cols == 0 || rows == 0 {
                return Err(TtsError::Onnx(format!(
                    "ort: logits shape invalid: {shape:?}"
                )));
            }
            let start = rows.saturating_sub(1) * cols;
            data.get(start..start + cols)
                .map(|v| v.to_vec())
                .ok_or_else(|| {
                    TtsError::Onnx(format!(
                        "ort: logits slice out of bounds: shape={shape:?} data_len={}",
                        data.len()
                    ))
                })
        }
        3 => {
            let b = shape[0].max(0) as usize;
            let s = shape[1].max(0) as usize;
            let vocab = shape[2].max(0) as usize;
            if b != 1 {
                return Err(TtsError::Onnx(format!(
                    "ort: unsupported batch size for logits: {b} (shape={shape:?})"
                )));
            }
            if vocab == 0 || s == 0 {
                return Err(TtsError::Onnx(format!(
                    "ort: logits shape invalid: {shape:?}"
                )));
            }
            let start = s.saturating_sub(1) * vocab;
            data.get(start..start + vocab)
                .map(|v| v.to_vec())
                .ok_or_else(|| {
                    TtsError::Onnx(format!(
                        "ort: logits slice out of bounds: shape={shape:?} data_len={}",
                        data.len()
                    ))
                })
        }
        _ => Err(TtsError::Onnx(format!(
            "ort: unsupported logits ndim={} shape={shape:?}",
            shape.len()
        ))),
    }?;

    if log_shape && cosyvoice_debug_log_enabled() {
        let mut finite = 0usize;
        let mut nan = 0usize;
        let mut inf = 0usize;
        let mut min_v = f32::INFINITY;
        let mut max_v = f32::NEG_INFINITY;
        let mut argmax: Option<usize> = None;
        let mut sum: f64 = 0.0;
        for (i, &x) in last.iter().enumerate() {
            if x.is_nan() {
                nan += 1;
                continue;
            }
            if x.is_infinite() {
                inf += 1;
                continue;
            }
            finite += 1;
            if x < min_v {
                min_v = x;
            }
            if x > max_v {
                max_v = x;
                argmax = Some(i);
            }
            sum += x as f64;
        }
        let mean = if finite > 0 {
            (sum / (finite as f64)) as f32
        } else {
            f32::NAN
        };
        cosyvoice_debug_log(format_args!(
            "[cosyvoice][tensor] {stage} logits_last: vocab={} finite={finite} nan={nan} inf={inf} min={min_v:.6} max={max_v:.6} mean={mean:.6} argmax={}\n",
            last.len(),
            argmax.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
        ));
    }

    Ok(last)
}

#[cfg(feature = "onnx-ort")]
fn ort_kv_cache_keep_last_f32(
    v: ort::value::DynValue,
    keep: usize,
) -> Result<ort::value::DynValue, TtsError> {
    if keep == 0 {
        return Ok(v);
    }

    let (shape, data) = v
        .try_extract_raw_tensor::<f32>()
        .map_err(|e| TtsError::Onnx(format!("ort: kv cache is not a CPU f32 tensor: {e}")))?;
    if shape.len() != 4 {
        return Err(TtsError::Onnx(format!(
            "ort: unsupported kv cache rank={} shape={shape:?}",
            shape.len()
        )));
    }

    let b = shape[0].max(0) as usize;
    let h = shape[1].max(0) as usize;
    let s = shape[2].max(0) as usize;
    let d = shape[3].max(0) as usize;
    if b == 0 || h == 0 || s == 0 || d == 0 {
        return Err(TtsError::Onnx(format!(
            "ort: kv cache shape invalid: {shape:?}"
        )));
    }

    if keep >= s {
        return Ok(v);
    }

    let start = s - keep;
    let mut out = vec![0.0f32; b * h * keep * d];
    for bb in 0..b {
        for hh in 0..h {
            for ss in 0..keep {
                let src_s = start + ss;
                let src_base = (((bb * h + hh) * s + src_s) * d) as usize;
                let dst_base = (((bb * h + hh) * keep + ss) * d) as usize;
                out[dst_base..dst_base + d].copy_from_slice(&data[src_base..src_base + d]);
            }
        }
    }

    let t = ort::value::Tensor::<f32>::from_array((
        vec![b, h, keep, d],
        out.into_boxed_slice(),
    ))
    .map_err(|e| TtsError::Onnx(format!("ort: build kv cache tensor failed: {e}")))?;
    Ok(t.into_dyn())
}

#[cfg(feature = "onnx-ort")]
fn ort_kv_output_candidates_from_past_input(past_in: &str) -> Vec<String> {
    // 说明：不同导出工具/版本对 KV cache 的命名不一致（past/present、past_key_values/present 等）。
    // 这里做一个“名字推导候选列表”，按顺序尝试 remove()，以便在不依赖严格顺序的情况下对齐 past inputs。
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    };

    push(past_in.to_string());
    push(past_in.replace("past_key_values", "present"));
    push(past_in.replace("past_key_values", "present_key_values"));
    push(past_in.replace("past_", "present_"));
    push(past_in.replace("past.", "present."));
    push(past_in.replace("past", "present"));

    out
}

#[cfg(feature = "onnx-ort")]
fn ort_take_kv_for_past_input(
    out: &mut ort::session::SessionOutputs<'_, '_>,
    stage: &'static str,
    past_in: &str,
) -> Result<ort::value::DynValue, TtsError> {
    let cands = ort_kv_output_candidates_from_past_input(past_in);
    for c in &cands {
        if let Some(v) = out.remove(c.as_str()) {
            return Ok(v);
        }
    }
    Err(TtsError::Onnx(format!(
        "ort: {stage} missing KV cache output for past input {past_in:?} (tried: {cands:?})"
    )))
}

#[cfg(feature = "onnx-ort")]
fn ort_try_extract_tensor_shape_f32(v: &ort::value::DynValue) -> Option<Vec<i64>> {
    v.try_extract_raw_tensor::<f32>()
        .ok()
        .map(|(s, _)| s.to_vec())
}

#[cfg(feature = "onnx-ort")]
fn cosyvoice_debug_log_enabled() -> bool {
    cosyvoice_debug_log_file().is_some()
}

#[cfg(feature = "onnx-ort")]
fn cosyvoice_debug_log_every() -> usize {
    std::env::var("CHAOS_COSYVOICE_DEBUG_LOG_EVERY")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(20)
}

#[cfg(feature = "onnx-ort")]
fn cosyvoice_debug_log_file() -> Option<&'static Mutex<std::fs::File>> {
    static LOG_FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    let opt = LOG_FILE.get_or_init(|| {
        let raw = std::env::var("CHAOS_COSYVOICE_DEBUG_LOG").ok()?;
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }

        // 支持 `CHAOS_COSYVOICE_DEBUG_LOG=1`：在当前工作目录写 cosyvoice_debug.log，方便临时排查。
        let path: PathBuf = if raw == "1" || raw.eq_ignore_ascii_case("true") {
            PathBuf::from("cosyvoice_debug.log")
        } else {
            PathBuf::from(raw)
        };

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let truncate = std::env::var("CHAOS_COSYVOICE_DEBUG_LOG_TRUNCATE")
            .ok()
            .as_deref()
            == Some("1");

        let mut opts = OpenOptions::new();
        opts.create(true);
        if truncate {
            opts.write(true).truncate(true);
        } else {
            opts.append(true);
        }

        match opts.open(&path) {
            Ok(f) => Some(Mutex::new(f)),
            Err(e) => {
                eprintln!(
                    "[cosyvoice] failed to open debug log file {}: {}",
                    path.display(),
                    e
                );
                None
            }
        }
    });
    opt.as_ref()
}

#[cfg(feature = "onnx-ort")]
fn cosyvoice_debug_log(args: std::fmt::Arguments<'_>) {
    let Some(m) = cosyvoice_debug_log_file() else {
        return;
    };
    let mut g = match m.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    let _ = g.write_fmt(args);
}

#[cfg(feature = "onnx-ort")]
fn cosyvoice_debug_log_f32_stats(stage: &'static str, data: &[f32]) {
    if !cosyvoice_debug_log_enabled() {
        return;
    }

    let mut finite = 0usize;
    let mut nan = 0usize;
    let mut inf = 0usize;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    let mut sum: f64 = 0.0;
    for &x in data {
        if x.is_nan() {
            nan += 1;
            continue;
        }
        if x.is_infinite() {
            inf += 1;
            continue;
        }
        finite += 1;
        if x < min_v {
            min_v = x;
        }
        if x > max_v {
            max_v = x;
        }
        sum += x as f64;
    }
    let mean = if finite > 0 {
        (sum / (finite as f64)) as f32
    } else {
        f32::NAN
    };

    cosyvoice_debug_log(format_args!(
        "[cosyvoice][stats] {stage}: len={} finite={finite} nan={nan} inf={inf} min={min_v:.6} max={max_v:.6} mean={mean:.6}\n",
        data.len()
    ));
}

#[cfg(feature = "onnx-tract")]
fn load_onnx_plan(
    path: impl AsRef<std::path::Path>,
) -> Result<
    Arc<
        tract_core::plan::SimplePlan<
            tract_onnx::prelude::TypedFact,
            Box<dyn tract_onnx::prelude::TypedOp>,
        >,
    >,
    TtsError,
> {
    use tract_onnx::prelude::*;
    let path = path.as_ref();
    let mut model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| TtsError::Onnx(format!("failed to load onnx {path:?}: {e}")))?;

    // 重要：给 tract 的推理分析器提供最基本的“输入 rank/shape”信息。
    // 某些导出的 LLM ONNX（尤其包含 KV cache 拼接）在输入 shape 完全未知时会在分析阶段失败（例如 InferenceConcat）。
    // 这里按 pack 约定的四类模型提供宽松但有用的 shape fact：
    // - llm_prefill:  input_ids [1, T]
    // - llm_decode:   token_id  [1, 1]
    // - flow_infer:   speech_tokens [1, T], spk_embedding [1, D]
    // - hift_infer:   mel [1, 80, T]
    //
    // 说明：我们只在能够安全判断时设置，避免对其他模型造成约束。
    let file = path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or_default();
    if file.eq_ignore_ascii_case("llm_prefill.onnx") {
        let text_len = model.sym("text_len");
        let _ = model.set_input_fact(
            0,
            InferenceFact::dt_shape(i64::datum_type(), tract_hir::shapefactoid![1, text_len]),
        );
    } else if file.eq_ignore_ascii_case("llm_decode.onnx") {
        let _ = model.set_input_fact(
            0,
            InferenceFact::dt_shape(i64::datum_type(), tract_hir::shapefactoid![1, 1]),
        );
    } else if file.eq_ignore_ascii_case("flow_infer.onnx") {
        let token_len = model.sym("token_len");
        let spk_dim = model.sym("spk_dim");
        let _ = model.set_input_fact(
            0,
            InferenceFact::dt_shape(i64::datum_type(), tract_hir::shapefactoid![1, token_len]),
        );
        let _ = model.set_input_fact(
            1,
            InferenceFact::dt_shape(f32::datum_type(), tract_hir::shapefactoid![1, spk_dim]),
        );
    } else if file.eq_ignore_ascii_case("hift_infer.onnx") {
        let mel_len = model.sym("mel_len");
        let _ = model.set_input_fact(
            0,
            InferenceFact::dt_shape(f32::datum_type(), tract_hir::shapefactoid![1, 80, mel_len]),
        );
    }
    let typed = model
        .into_optimized()
        .map_err(|e| TtsError::Onnx(format!("failed to optimize onnx {path:?}: {e}")))?;
    typed
        .into_runnable()
        .map_err(|e| TtsError::Onnx(format!("failed to make runnable onnx {path:?}: {e}")))
}

#[cfg(feature = "onnx-tract")]
fn extract_last_logits(v: &tract_onnx::prelude::TValue) -> Result<Vec<f32>, TtsError> {
    let view = v
        .to_array_view::<f32>()
        .map_err(|e| TtsError::Onnx(format!("logits output is not f32: {e}")))?;
    // Accept [1, vocab] or [1, seq, vocab] or [seq, vocab] etc.
    match view.ndim() {
        1 => Ok(view.iter().copied().collect()),
        2 => {
            let shape = view.shape();
            let rows = shape[0];
            let cols = shape[1];
            let flat: Vec<f32> = view.iter().copied().collect();
            let start = (rows.saturating_sub(1)) * cols;
            Ok(flat[start..start + cols].to_vec())
        }
        3 => {
            let shape = view.shape();
            let b = shape[0];
            let s = shape[1];
            let v = shape[2];
            if b != 1 {
                return Err(TtsError::Onnx(format!(
                    "unsupported batch size for logits: {b}"
                )));
            }
            let flat: Vec<f32> = view.iter().copied().collect();
            let start = (s.saturating_sub(1)) * v;
            Ok(flat[start..start + v].to_vec())
        }
        _ => Err(TtsError::Onnx(format!(
            "unsupported logits ndim={} shape={:?}",
            view.ndim(),
            view.shape()
        ))),
    }
}

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

    // 默认使用 align_corners=false 的线性插值（与 PyTorch/F.interpolate 的默认行为更一致）。
    // 经验上这比“强行对齐两端点”的插值更不容易在语音上引入高频尖啸/金属感。
    let mut out = vec![0.0f32; channels * new_t];
    if new_t == 1 {
        // 退化情况：取中间帧（比强行取第 0 帧更稳）。
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
            // align_corners=false: (i+0.5)/new_t * t - 0.5
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mel_time_scaling_changes_length() {
        let mel = vec![0.0f32; 80 * 100];
        let scaled = time_scale_mel_linear(&mel, 80, 2.0).unwrap();
        assert_eq!(scaled.len(), 80 * 50);
    }
}
