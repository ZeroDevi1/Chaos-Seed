use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

use crate::tts::cosyvoice::pack::CosyVoicePack;
use crate::tts::sampling::{SamplingConfig, sample_ras_next};
use crate::tts::text::{PromptStrategy, resolve_tts_text_basic};
use crate::tts::wav::{TtsPcm16Result, TtsWavResult, duration_ms, encode_wav_pcm16_mono, f32_to_pcm16_mono};
use crate::tts::TtsError;

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

/// ONNX-backed CosyVoice3 engine.
///
/// Note: V1 uses `tract` as the default backend (pure Rust) for maximum ONNX operator coverage.
/// The rest of the architecture (voice chat stream, protocol, post-processing) is backend-agnostic.
pub struct CosyVoiceEngine {
    pack: Arc<CosyVoicePack>,
    #[cfg(feature = "onnx-tract")]
    llm_prefill: Arc<tract_core::plan::SimplePlan<tract_onnx::prelude::TypedFact, Box<dyn tract_onnx::prelude::TypedOp>>>,
    #[cfg(feature = "onnx-tract")]
    llm_decode: Arc<tract_core::plan::SimplePlan<tract_onnx::prelude::TypedFact, Box<dyn tract_onnx::prelude::TypedOp>>>,
    #[cfg(feature = "onnx-tract")]
    flow_infer: Arc<tract_core::plan::SimplePlan<tract_onnx::prelude::TypedFact, Box<dyn tract_onnx::prelude::TypedOp>>>,
    #[cfg(feature = "onnx-tract")]
    hift_infer: Arc<tract_core::plan::SimplePlan<tract_onnx::prelude::TypedFact, Box<dyn tract_onnx::prelude::TypedOp>>>,
}

impl CosyVoiceEngine {
    pub fn load(pack: CosyVoicePack) -> Result<Self, TtsError> {
        let pack = Arc::new(pack);

        #[cfg(not(feature = "onnx-tract"))]
        {
            let _ = pack;
            return Err(TtsError::NotImplemented(
                "CosyVoiceEngine requires feature `onnx-tract` (fallback backend) for now",
            ));
        }

        #[cfg(feature = "onnx-tract")]
        {
            let llm_prefill = load_onnx_plan(pack.path_llm_prefill())?;
            let llm_decode = load_onnx_plan(pack.path_llm_decode())?;
            let flow_infer = load_onnx_plan(pack.path_flow_infer())?;
            let hift_infer = load_onnx_plan(pack.path_hift_infer())?;

            Ok(Self {
                pack,
                llm_prefill,
                llm_decode,
                flow_infer,
                hift_infer,
            })
        }
    }

    pub fn pack(&self) -> &CosyVoicePack {
        &self.pack
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

        let resolved = resolve_tts_text_basic(
            &params.text,
            &params.prompt_text,
            params.prompt_strategy,
            &params.guide_sep,
            params.text_frontend,
        )?;

        let mut input_ids: Vec<i64> = Vec::new();
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

        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(TtsError::Canceled);
            }
        }

        // LLM: autoregressively sample speech tokens until stop token.
        let mut rng = ChaCha20Rng::seed_from_u64(params.seed);
        let speech_tokens = self.llm_generate(
            &input_ids,
            spoken_text_len,
            &params.sampling,
            &mut rng,
            cancel,
        )?;

        if speech_tokens.is_empty() {
            return Err(TtsError::Onnx("LLM produced no speech tokens".into()));
        }

        // Flow: tokens -> mel
        let spk = self.pack.spk2info.get(params.spk_id.trim()).expect("checked");
        let mel = self.flow_tokens_to_mel(&speech_tokens, &spk.embedding, cancel)?;

        // Speed change: only for non-stream mode. We apply linear interpolation on mel time axis.
        let mel = if (params.speed - 1.0).abs() > f32::EPSILON {
            time_scale_mel_linear(&mel, 80, params.speed)?
        } else {
            mel
        };

        // HiFT: mel -> waveform f32
        let wav_f32 = self.hift_mel_to_wav(&mel, cancel)?;

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
    ) -> Result<Vec<i64>, TtsError> {
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

        #[cfg(feature = "onnx-tract")]
        {
            use tract_onnx::prelude::*;

            let input = tensor1(input_ids)
                .into_shape(&[1, input_ids.len()])
                .map_err(|e| TtsError::Onnx(format!("reshape input_ids failed: {e}")))?;
            let prefill_out = self
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
                let out = self
                    .llm_decode
                    .run(inputs)
                    .map_err(|e| TtsError::Onnx(format!("llm_decode run failed: {e}")))?;
                if out.is_empty() {
                    return Err(TtsError::Onnx("llm_decode returned no outputs".into()));
                }
                last_logits = extract_last_logits(&out[0])?;
                past = out.iter().skip(1).cloned().collect();
            }
            Ok(decoded)
        }

        #[cfg(not(feature = "onnx-tract"))]
        {
            let _ = (input_ids, spoken_text_len, sampling, rng, cancel, min_len, max_len, stop_start);
            Err(TtsError::NotImplemented("onnx backend is not enabled"))
        }
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

        #[cfg(feature = "onnx-tract")]
        {
            use tract_onnx::prelude::*;
            let token_t = tensor1(speech_tokens)
                .into_shape(&[1, speech_tokens.len()])
                .map_err(|e| TtsError::Onnx(format!("reshape speech_tokens failed: {e}")))?;
            let emb_t = tensor1(spk_embedding)
                .into_shape(&[1, spk_embedding.len()])
                .map_err(|e| TtsError::Onnx(format!("reshape spk_embedding failed: {e}")))?;
            let out = self
                .flow_infer
                .run(tvec!(token_t.into(), emb_t.into()))
                .map_err(|e| TtsError::Onnx(format!("flow_infer run failed: {e}")))?;
            if out.is_empty() {
                return Err(TtsError::Onnx("flow_infer returned no outputs".into()));
            }
            let mel = out[0]
                .to_array_view::<f32>()
                .map_err(|e| TtsError::Onnx(format!("flow output is not f32: {e}")))?;
            Ok(mel.iter().copied().collect())
        }

        #[cfg(not(feature = "onnx-tract"))]
        {
            let _ = (speech_tokens, spk_embedding);
            Err(TtsError::NotImplemented("onnx backend is not enabled"))
        }
    }

    fn hift_mel_to_wav(&self, mel: &[f32], cancel: Option<&AtomicBool>) -> Result<Vec<f32>, TtsError> {
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

        #[cfg(feature = "onnx-tract")]
        {
            use tract_onnx::prelude::*;
            let mel_t = tensor1(mel)
                .into_shape(&[1, 80, t])
                .map_err(|e| TtsError::Onnx(format!("reshape mel failed: {e}")))?;
            let out = self
                .hift_infer
                .run(tvec!(mel_t.into()))
                .map_err(|e| TtsError::Onnx(format!("hift_infer run failed: {e}")))?;
            if out.is_empty() {
                return Err(TtsError::Onnx("hift_infer returned no outputs".into()));
            }
            let wav = out[0]
                .to_array_view::<f32>()
                .map_err(|e| TtsError::Onnx(format!("hift output is not f32: {e}")))?;
            Ok(wav.iter().copied().collect())
        }

        #[cfg(not(feature = "onnx-tract"))]
        {
            let _ = (mel, t);
            Err(TtsError::NotImplemented("onnx backend is not enabled"))
        }
    }
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
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|e| TtsError::Onnx(format!("failed to load onnx {path:?}: {e}")))?;
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

    let mut out = vec![0.0f32; channels * new_t];
    for ch in 0..channels {
        for i in 0..new_t {
            let src_pos = (i as f32) * (t.saturating_sub(1) as f32)
                / (new_t.saturating_sub(1).max(1) as f32);
            let lo = src_pos.floor() as usize;
            let hi = src_pos.ceil() as usize;
            let a = src_pos - (lo as f32);
            let lo_v = mel[ch * t + lo];
            let hi_v = mel[ch * t + hi.min(t - 1)];
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
