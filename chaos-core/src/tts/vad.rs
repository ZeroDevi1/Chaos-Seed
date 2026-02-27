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
    fn detect_segments(&self, pcm16: &[i16], sample_rate: u32, cfg: &VadConfig) -> Result<Vec<VadSegment>, VadError>;
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

