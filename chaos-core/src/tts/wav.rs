use std::io::Cursor;

use crate::tts::TtsError;

#[derive(Debug, Clone)]
pub struct TtsPcm16Result {
    pub sample_rate: u32,
    pub channels: u16,
    pub pcm16: Vec<i16>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TtsWavResult {
    pub sample_rate: u32,
    pub channels: u16,
    pub wav_bytes: Vec<u8>,
    pub duration_ms: u64,
}

pub fn encode_wav_pcm16_mono(sample_rate: u32, pcm: &[i16]) -> Result<Vec<u8>, TtsError> {
    if sample_rate == 0 {
        return Err(TtsError::InvalidArg("sample_rate must be > 0".into()));
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::<u8>::new());
    {
        let mut w = hound::WavWriter::new(&mut buf, spec)
            .map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
        for &s in pcm {
            w.write_sample(s)
                .map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
        }
        w.finalize()
            .map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
    }
    Ok(buf.into_inner())
}

pub fn f32_to_pcm16_mono(pcm_f32: &[f32]) -> Vec<i16> {
    pcm_f32
        .iter()
        .map(|&x| {
            let x = x.clamp(-1.0, 1.0);
            // Symmetric mapping: -1.0 -> -32768, +1.0 -> 32767
            if x >= 0.0 {
                (x * 32767.0).round() as i16
            } else {
                (x * 32768.0).round() as i16
            }
        })
        .collect()
}

/// 对单声道 PCM f32 做 IIR notch（陷波）滤波，常用于去除窄带“高频嗡嗡/啸叫”。
///
/// - `freq_hz`：陷波中心频率（Hz）
/// - `q`：品质因子，越大陷波越窄（建议 10~50）。
///
/// 说明：这是一个很轻量的后处理兜底；如果模型/导出正确，原则上不应依赖它。
pub fn notch_filter_f32_mono_inplace(
    pcm: &mut [f32],
    sample_rate: u32,
    freq_hz: f32,
    q: f32,
) -> Result<(), TtsError> {
    if sample_rate == 0 {
        return Err(TtsError::InvalidArg("sample_rate must be > 0".into()));
    }
    if pcm.is_empty() {
        return Ok(());
    }
    if !(freq_hz > 0.0) {
        return Err(TtsError::InvalidArg("freq_hz must be > 0".into()));
    }
    if !(q > 0.0) {
        return Err(TtsError::InvalidArg("q must be > 0".into()));
    }

    let sr = sample_rate as f64;
    let f = freq_hz as f64;
    let q = q as f64;
    let nyq = sr * 0.5;
    if f >= nyq {
        return Err(TtsError::InvalidArg(format!(
            "freq_hz must be < Nyquist ({}), got {freq_hz}",
            nyq
        )));
    }

    // RBJ audio EQ cookbook: notch filter biquad.
    let w0 = 2.0 * std::f64::consts::PI * f / sr;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = 1.0;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;

    let b0 = b0 / a0;
    let b1 = b1 / a0;
    let b2 = b2 / a0;
    let a1 = a1 / a0;
    let a2 = a2 / a0;

    // Direct Form II (transposed).
    let mut z1 = 0.0f64;
    let mut z2 = 0.0f64;
    for x in pcm.iter_mut() {
        let x0 = (*x as f64).clamp(-1.5, 1.5); // 稳定性兜底（正常输出应在 [-1,1]）
        let y0 = b0 * x0 + z1;
        z1 = b1 * x0 - a1 * y0 + z2;
        z2 = b2 * x0 - a2 * y0;
        *x = y0 as f32;
    }

    Ok(())
}

pub fn duration_ms(sample_rate: u32, samples: usize) -> u64 {
    if sample_rate == 0 {
        return 0;
    }
    ((samples as u128) * 1000u128 / (sample_rate as u128)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        if x.is_empty() {
            return 0.0;
        }
        let mut sum = 0.0f64;
        for &v in x {
            let v = v as f64;
            sum += v * v;
        }
        ((sum / (x.len() as f64)).sqrt()) as f32
    }

    #[test]
    fn wav_roundtrip_is_readable() {
        let pcm = [0i16, 1000, -1000, 0, 32767, -32768];
        let bytes = encode_wav_pcm16_mono(24000, &pcm).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 24000);
        let decoded: Vec<i16> = reader.samples::<i16>().map(|x| x.unwrap()).collect();
        assert_eq!(decoded, pcm);
    }

    #[test]
    fn f32_to_pcm16_clamps() {
        let pcm = f32_to_pcm16_mono(&[-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0]);
        assert_eq!(pcm[0], -32768);
        assert_eq!(pcm[1], -32768);
        assert_eq!(pcm[3], 0);
        assert_eq!(pcm[5], 32767);
        assert_eq!(pcm[6], 32767);
    }

    #[test]
    fn notch_filter_reduces_pure_tone_energy() {
        let sr = 24_000u32;
        let f = 6_000.0f32;
        let n = sr as usize; // 1s
        let mut x = vec![0.0f32; n];
        for i in 0..n {
            let t = (i as f32) / (sr as f32);
            x[i] = (2.0 * std::f32::consts::PI * f * t).sin() * 0.5;
        }
        let before = rms(&x);
        notch_filter_f32_mono_inplace(&mut x, sr, f, 30.0).unwrap();
        let after = rms(&x);
        assert!(after < before * 0.5, "before={before} after={after}");
    }
}

