use crate::tts::TtsError;

#[derive(Debug, thiserror::Error)]
pub enum VadError {
    #[error("invalid argument: {0}")]
    InvalidArg(String),
    #[error("vad backend error: {0}")]
    Backend(String),
}

impl From<VadError> for TtsError {
    fn from(e: VadError) -> Self {
        TtsError::Vad(e.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VadSegment {
    pub start_sample: usize,
    pub end_sample: usize,
}

#[derive(Debug, Clone)]
pub struct VadConfig {
    pub frame_ms: u32,
    pub threshold: f32,
    pub min_speech_ms: u32,
    pub min_silence_ms: u32,
    pub pad_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            frame_ms: 30,
            threshold: 0.015,
            min_speech_ms: 200,
            min_silence_ms: 150,
            pad_ms: 80,
        }
    }
}

pub trait VadEngine: Send + Sync {
    fn detect_segments(
        &self,
        pcm16: &[i16],
        sample_rate: u32,
        cfg: &VadConfig,
    ) -> Result<Vec<VadSegment>, VadError>;
}

/// A tiny pure-Rust VAD fallback based on short-time energy (RMS).
///
/// This is NOT as good as Silero, but it's deterministic, dependency-light, and enough to
/// remove obvious leading/trailing silence and many prompt-text leaks in practice.
#[derive(Debug, Default)]
pub struct EnergyVad;

impl VadEngine for EnergyVad {
    fn detect_segments(
        &self,
        pcm16: &[i16],
        sample_rate: u32,
        cfg: &VadConfig,
    ) -> Result<Vec<VadSegment>, VadError> {
        if sample_rate == 0 {
            return Err(VadError::InvalidArg("sample_rate must be > 0".into()));
        }
        if cfg.frame_ms == 0 {
            return Err(VadError::InvalidArg("frame_ms must be > 0".into()));
        }
        if pcm16.is_empty() {
            return Ok(vec![]);
        }

        let frame = (sample_rate as u64 * cfg.frame_ms as u64 / 1000).max(1) as usize;
        let min_speech = (sample_rate as u64 * cfg.min_speech_ms as u64 / 1000) as usize;
        let min_silence = (sample_rate as u64 * cfg.min_silence_ms as u64 / 1000) as usize;

        let mut speech = vec![false; (pcm16.len() + frame - 1) / frame];
        for (i, chunk) in pcm16.chunks(frame).enumerate() {
            let rms = rms_i16(chunk);
            speech[i] = rms >= cfg.threshold;
        }

        // Merge into segments with a simple hangover rule based on min_silence.
        let silence_frames = (min_silence + frame - 1) / frame;
        let mut segs = Vec::new();
        let mut i = 0usize;
        while i < speech.len() {
            if !speech[i] {
                i += 1;
                continue;
            }
            let start_f = i;
            let mut end_f = i;
            let mut silence_run = 0usize;
            i += 1;
            while i < speech.len() {
                if speech[i] {
                    end_f = i;
                    silence_run = 0;
                } else {
                    silence_run += 1;
                    if silence_run >= silence_frames {
                        break;
                    }
                }
                i += 1;
            }

            let start_sample = start_f * frame;
            let end_sample = ((end_f + 1) * frame).min(pcm16.len());
            if end_sample.saturating_sub(start_sample) >= min_speech {
                segs.push(VadSegment {
                    start_sample,
                    end_sample,
                });
            }
        }

        Ok(segs)
    }
}

fn rms_i16(pcm: &[i16]) -> f32 {
    if pcm.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0f64;
    for &s in pcm {
        let x = (s as f64) / 32768.0;
        sum += x * x;
    }
    ((sum / (pcm.len() as f64)).sqrt()) as f32
}

// -----------------------------
// Silero VAD (optional feature)
// -----------------------------

#[cfg(feature = "silero-vad")]
pub struct SileroVad {
    model: std::sync::Mutex<silero_vad_rs::SileroVAD>,
}

#[cfg(feature = "silero-vad")]
impl std::fmt::Debug for SileroVad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SileroVad").finish_non_exhaustive()
    }
}

#[cfg(feature = "silero-vad")]
impl SileroVad {
    pub fn new(model_path: impl AsRef<std::path::Path>) -> Result<Self, VadError> {
        let p = model_path.as_ref();
        if !p.exists() {
            // 避免运行时偷偷下载模型，分发/离线场景更可控。
            return Err(VadError::Backend(format!(
                "silero vad model not found: {}",
                p.display()
            )));
        }
        let model = silero_vad_rs::SileroVAD::new(p)
            .map_err(|e| VadError::Backend(format!("silero vad init failed: {e}")))?;
        Ok(Self {
            model: std::sync::Mutex::new(model),
        })
    }
}

#[cfg(feature = "silero-vad")]
impl VadEngine for SileroVad {
    fn detect_segments(
        &self,
        pcm16: &[i16],
        sample_rate: u32,
        cfg: &VadConfig,
    ) -> Result<Vec<VadSegment>, VadError> {
        if sample_rate == 0 {
            return Err(VadError::InvalidArg("sample_rate must be > 0".into()));
        }
        if pcm16.is_empty() {
            return Ok(vec![]);
        }
        if cfg.threshold <= 0.0 {
            return Err(VadError::InvalidArg("threshold must be > 0".into()));
        }

        // SileroVAD 模型仅支持 16kHz、512 samples 一帧；这里做一次简易重采样。
        let sr_vad = 16_000u32;
        let x16k = if sample_rate == sr_vad {
            pcm16
                .iter()
                .map(|&s| (s as f32) / 32768.0)
                .collect::<Vec<f32>>()
        } else {
            resample_linear_i16_to_f32(pcm16, sample_rate, sr_vad)
        };

        if x16k.len() < 512 {
            return Ok(vec![]);
        }

        let frame = 512usize;
        let min_speech = (sr_vad as u64 * cfg.min_speech_ms as u64 / 1000) as usize;
        let min_silence = (sr_vad as u64 * cfg.min_silence_ms as u64 / 1000) as usize;
        let silence_frames = (min_silence + frame - 1) / frame;

        let mut speech = vec![false; x16k.len() / frame];
        {
            let mut model = self
                .model
                .lock()
                .map_err(|_| VadError::Backend("silero vad mutex poisoned".into()))?;
            model.reset_states(1);
            for i in 0..speech.len() {
                let off = i * frame;
                let slice = &x16k[off..off + frame];
                let view = ndarray::ArrayView1::from(slice);
                let prob = model
                    .process_chunk(&view, sr_vad)
                    .map_err(|e| VadError::Backend(format!("silero vad run failed: {e}")))?;
                speech[i] = prob[0] >= cfg.threshold;
            }
        }

        let mut segs_16k = Vec::new();
        let mut i = 0usize;
        while i < speech.len() {
            if !speech[i] {
                i += 1;
                continue;
            }
            let start_f = i;
            let mut end_f = i;
            let mut silence_run = 0usize;
            i += 1;
            while i < speech.len() {
                if speech[i] {
                    end_f = i;
                    silence_run = 0;
                } else {
                    silence_run += 1;
                    if silence_run >= silence_frames {
                        break;
                    }
                }
                i += 1;
            }

            let start_sample = start_f * frame;
            let end_sample = ((end_f + 1) * frame).min(x16k.len());
            if end_sample.saturating_sub(start_sample) >= min_speech {
                segs_16k.push(VadSegment {
                    start_sample,
                    end_sample,
                });
            }
        }

        // 映射回原采样率的 sample index（用于裁剪）。
        let mut segs = Vec::with_capacity(segs_16k.len());
        for s in segs_16k {
            let start =
                ((s.start_sample as f64) * (sample_rate as f64) / (sr_vad as f64)).round() as usize;
            let end =
                ((s.end_sample as f64) * (sample_rate as f64) / (sr_vad as f64)).round() as usize;
            if start < end {
                segs.push(VadSegment {
                    start_sample: start.min(pcm16.len()),
                    end_sample: end.min(pcm16.len()),
                });
            }
        }
        Ok(segs)
    }
}

#[cfg(feature = "silero-vad")]
fn resample_linear_i16_to_f32(pcm16: &[i16], sr_in: u32, sr_out: u32) -> Vec<f32> {
    if sr_in == 0 || sr_out == 0 || pcm16.is_empty() {
        return Vec::new();
    }
    let out_len = ((pcm16.len() as u64) * (sr_out as u64) / (sr_in as u64)).max(1) as usize;
    let mut out = Vec::with_capacity(out_len);
    let scale = (sr_in as f64) / (sr_out as f64);
    for i in 0..out_len {
        let src_pos = (i as f64) * scale;
        let idx0 = src_pos.floor() as isize;
        let idx1 = idx0 + 1;
        let t = (src_pos - (idx0 as f64)) as f32;
        let s0 = pcm16.get(idx0.max(0) as usize).copied().unwrap_or(0) as f32 / 32768.0;
        let s1 = pcm16.get(idx1.max(0) as usize).copied().unwrap_or(0) as f32 / 32768.0;
        out.push(s0 + (s1 - s0) * t);
    }
    out
}
