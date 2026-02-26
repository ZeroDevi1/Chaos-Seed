use std::io::Cursor;

use crate::TtsError;

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
        let mut w = hound::WavWriter::new(&mut buf, spec).map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
        for &s in pcm {
            w.write_sample(s).map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
        }
        w.finalize().map_err(|e| TtsError::Io(std::io::Error::other(e)))?;
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

pub fn duration_ms(sample_rate: u32, samples: usize) -> u64 {
    if sample_rate == 0 {
        return 0;
    }
    ((samples as u128) * 1000u128 / (sample_rate as u128)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

