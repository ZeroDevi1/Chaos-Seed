use crate::tts::TtsError;
use crate::tts::vad::{EnergyVad, VadConfig, VadEngine};

#[derive(Debug, Clone)]
pub struct TrimConfig {
    /// Trim a deterministic prefix (e.g. prompt/guide) by ratio of total samples.
    pub enable_prompt_trim: bool,
    /// If set, we drop `ratio * total_samples` from the front.
    ///
    /// This is intentionally caller-provided so the caller can compute it from tokenizer
    /// lengths (guide_tokens / total_tokens) without requiring text/tokenizer at this layer.
    pub prompt_prefix_ratio: Option<f32>,

    /// Trim leading/trailing silence using VAD.
    pub enable_vad_trim: bool,
    pub vad: VadConfig,

    /// If VAD finds speech, we pad both sides by this many milliseconds (clamped to bounds).
    pub pad_ms: u32,
}

impl Default for TrimConfig {
    fn default() -> Self {
        Self {
            enable_prompt_trim: false,
            prompt_prefix_ratio: None,
            enable_vad_trim: true,
            vad: VadConfig::default(),
            pad_ms: 80,
        }
    }
}

/// Trim TTS output PCM16 (mono) in a deterministic, config-driven way.
///
/// V1 behavior:
/// - Prompt-trim: optional ratio-based prefix removal (caller must provide ratio).
/// - VAD-trim: caller-provided VAD backend (default: pure-Rust energy VAD).
pub fn trim_output_pcm16(
    pcm16: &[i16],
    sample_rate: u32,
    cfg: &TrimConfig,
) -> Result<Vec<i16>, TtsError> {
    let vad = EnergyVad::default();
    trim_output_pcm16_with_engine(pcm16, sample_rate, cfg, &vad)
}

/// Same as [`trim_output_pcm16`], but allows the caller to choose a VAD backend
/// (e.g. Energy VAD vs Silero VAD).
pub fn trim_output_pcm16_with_engine(
    pcm16: &[i16],
    sample_rate: u32,
    cfg: &TrimConfig,
    vad: &dyn VadEngine,
) -> Result<Vec<i16>, TtsError> {
    if sample_rate == 0 {
        return Err(TtsError::InvalidArg("sample_rate must be > 0".into()));
    }
    if pcm16.is_empty() {
        return Ok(Vec::new());
    }

    let mut start = 0usize;
    let mut end = pcm16.len();

    if cfg.enable_prompt_trim {
        if let Some(r) = cfg.prompt_prefix_ratio {
            if r.is_finite() && r > 0.0 {
                let drop = ((pcm16.len() as f32) * r)
                    .round()
                    .clamp(0.0, pcm16.len() as f32) as usize;
                start = start.max(drop.min(end));
            }
        }
    }

    if cfg.enable_vad_trim && start < end {
        let mut vad_cfg = cfg.vad.clone();
        vad_cfg.pad_ms = cfg.pad_ms;
        let segs = vad.detect_segments(&pcm16[start..end], sample_rate, &vad_cfg)?;
        if let (Some(first), Some(last)) = (segs.first(), segs.last()) {
            let pad = (sample_rate as u64 * cfg.pad_ms as u64 / 1000) as isize;
            let s = (first.start_sample as isize - pad).max(0) as usize;
            let e = (last.end_sample as isize + pad).min((end - start) as isize) as usize;
            start += s.min(end - start);
            end = start + (e.saturating_sub(s)).min(pcm16.len().saturating_sub(start));
        }
    }

    if start >= end {
        return Ok(Vec::new());
    }
    Ok(pcm16[start..end].to_vec())
}
