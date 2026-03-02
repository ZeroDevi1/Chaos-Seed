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

#[derive(Debug, Clone, Copy)]
pub struct WavMeta {
    pub sample_rate: u32,
    pub channels: u16,
    /// 每声道样本数（即帧数）。
    pub samples: u32,
}

/// 从 WAV bytes 中读取最基本的元信息（不解码完整 PCM）。
pub fn read_wav_meta_from_bytes(wav_bytes: &[u8]) -> Result<WavMeta, TtsError> {
    let reader = hound::WavReader::new(Cursor::new(wav_bytes))
        .map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
    let spec = reader.spec();
    let total_samples = reader.duration(); // 总样本数（所有声道）
    let ch = spec.channels.max(1) as u32;
    let samples_per_ch = (total_samples / ch).min(u32::MAX as u32);
    Ok(WavMeta {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        samples: samples_per_ch,
    })
}

/// 将 WAV bytes 解码为单声道 PCM16。
///
/// 说明：
/// - VoiceLab 的 infer_sft.py 典型输出是 PCM16 单声道，但这里做一些兼容兜底（float32/int32）。
/// - 若遇到多声道输出，当前直接报错（避免“错误混音”导致听感异常）。
pub fn decode_wav_bytes_to_pcm16_mono(wav_bytes: &[u8]) -> Result<TtsPcm16Result, TtsError> {
    let mut reader = hound::WavReader::new(Cursor::new(wav_bytes))
        .map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
    let spec = reader.spec();
    if spec.channels != 1 {
        return Err(TtsError::NotImplemented("only mono wav is supported"));
    }

    let sample_rate = spec.sample_rate;
    if sample_rate == 0 {
        return Err(TtsError::InvalidArg("wav sample_rate must be > 0".into()));
    }

    let pcm16: Vec<i16> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .map(|s| s.map_err(|e| TtsError::Io(std::io::Error::other(e))))
            .collect::<Result<Vec<_>, _>>()?,
        (hound::SampleFormat::Int, 32) => {
            let x: Vec<i32> = reader
                .samples::<i32>()
                .map(|s| s.map_err(|e| TtsError::Io(std::io::Error::other(e))))
                .collect::<Result<Vec<_>, _>>()?;
            x.into_iter()
                .map(|v| (v / 65536).clamp(i16::MIN as i32, i16::MAX as i32) as i16)
                .collect()
        }
        (hound::SampleFormat::Float, 32) => {
            let x: Vec<f32> = reader
                .samples::<f32>()
                .map(|s| s.map_err(|e| TtsError::Io(std::io::Error::other(e))))
                .collect::<Result<Vec<_>, _>>()?;
            f32_to_pcm16_mono(&x)
        }
        _ => {
            return Err(TtsError::NotImplemented(
                "unsupported wav sample format (expected int16/int32/float32)",
            ));
        }
    };

    let duration_ms = duration_ms(sample_rate, pcm16.len());
    Ok(TtsPcm16Result {
        sample_rate,
        channels: 1,
        pcm16,
        duration_ms,
    })
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
