use rand::Rng;

use crate::TtsError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingConfig {
    /// Temperature scaling applied to logits before sampling (must be > 0).
    pub temperature: f32,
    /// Nucleus top-p in (0, 1].
    pub top_p: f32,
    /// Nucleus top-k (must be > 0).
    pub top_k: usize,
    /// RAS repetition window size (must be > 0).
    pub win_size: usize,
    /// RAS repetition threshold (must be >= 0).
    pub tau_r: f32,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            top_p: 0.6,
            top_k: 10,
            win_size: 10,
            tau_r: 1.0,
        }
    }
}

/// Implements CosyVoice `ras_sampling` + temperature scaling (as done in `infer_sft.py`).
///
/// `weighted_scores` can be logits or log-probabilities; we apply softmax internally.
pub fn sample_ras_next(
    weighted_scores: &[f32],
    decoded_tokens: &[u32],
    cfg: &SamplingConfig,
    rng: &mut impl Rng,
) -> Result<u32, TtsError> {
    if weighted_scores.is_empty() {
        return Err(TtsError::InvalidArg("weighted_scores is empty".into()));
    }
    if !(cfg.temperature > 0.0) {
        return Err(TtsError::InvalidArg("temperature must be > 0".into()));
    }
    if !(cfg.top_p > 0.0 && cfg.top_p <= 1.0) {
        return Err(TtsError::InvalidArg("top_p must be in (0, 1]".into()));
    }
    if cfg.top_k == 0 {
        return Err(TtsError::InvalidArg("top_k must be > 0".into()));
    }
    if cfg.win_size == 0 {
        return Err(TtsError::InvalidArg("win_size must be > 0".into()));
    }
    if cfg.tau_r < 0.0 {
        return Err(TtsError::InvalidArg("tau_r must be >= 0".into()));
    }

    // Temperature scaling: logits /= temperature.
    let mut scores: Vec<f32> = weighted_scores.iter().map(|x| x / cfg.temperature).collect();

    let top_id = nucleus_sampling(&scores, cfg.top_p, cfg.top_k, rng)?;

    // RAS repetition check.
    let rep_num = decoded_tokens
        .iter()
        .rev()
        .take(cfg.win_size)
        .filter(|&&t| t == top_id)
        .count() as f32;
    if rep_num >= (cfg.win_size as f32) * cfg.tau_r {
        // weighted_scores[top_id] = -inf; then random_sampling over softmax.
        if let Some(s) = scores.get_mut(top_id as usize) {
            *s = f32::NEG_INFINITY;
        }
        return random_sampling(&scores, rng);
    }

    Ok(top_id)
}

fn random_sampling(scores: &[f32], rng: &mut impl Rng) -> Result<u32, TtsError> {
    let probs = softmax(scores);
    sample_from_probs(&probs, rng)
}

fn nucleus_sampling(
    scores: &[f32],
    top_p: f32,
    top_k: usize,
    rng: &mut impl Rng,
) -> Result<u32, TtsError> {
    let probs = softmax(scores);

    // Sort by prob desc; stable tie-breaker by index asc.
    let mut pairs: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
    pairs.sort_by(|(ia, pa), (ib, pb)| {
        pb.partial_cmp(pa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| ia.cmp(ib))
    });

    let mut chosen: Vec<(usize, f32)> = Vec::new();
    let mut cum = 0.0f32;
    for (i, p) in pairs {
        if chosen.len() < top_k && cum < top_p {
            chosen.push((i, p));
            cum += p;
        } else {
            break;
        }
    }
    if chosen.is_empty() {
        // Degenerate: fall back to full distribution.
        return sample_from_probs(&probs, rng);
    }

    let mut c_probs: Vec<f32> = chosen.iter().map(|(_, p)| *p).collect();
    // Renormalize (Python keeps raw probs then multinomial; multinomial doesn't require sum=1 but we do).
    let sum: f32 = c_probs.iter().sum();
    if sum > 0.0 {
        for p in &mut c_probs {
            *p /= sum;
        }
    }

    let pick = sample_from_probs(&c_probs, rng)? as usize;
    Ok(chosen[pick].0 as u32)
}

fn softmax(scores: &[f32]) -> Vec<f32> {
    // Stable softmax.
    let mut max = f32::NEG_INFINITY;
    for &x in scores {
        if x.is_finite() && x > max {
            max = x;
        }
    }
    if !max.is_finite() {
        // All -inf -> uniform.
        return vec![1.0 / (scores.len() as f32); scores.len()];
    }

    let mut exps = Vec::with_capacity(scores.len());
    let mut sum = 0.0f32;
    for &x in scores {
        let e = if x.is_finite() { (x - max).exp() } else { 0.0 };
        exps.push(e);
        sum += e;
    }
    if sum == 0.0 {
        return vec![1.0 / (scores.len() as f32); scores.len()];
    }
    for e in &mut exps {
        *e /= sum;
    }
    exps
}

fn sample_from_probs(probs: &[f32], rng: &mut impl Rng) -> Result<u32, TtsError> {
    if probs.is_empty() {
        return Err(TtsError::InvalidArg("probs is empty".into()));
    }
    // Rust 2024: `gen` is a reserved keyword, use raw identifier.
    let mut r: f32 = rng.r#gen::<f32>();
    // Clamp in case RNG returns 1.0 exactly (rare but possible depending on impl).
    if r >= 1.0 {
        r = 0.99999994;
    }
    let mut cum = 0.0f32;
    for (i, &p) in probs.iter().enumerate() {
        cum += p.max(0.0);
        if r < cum {
            return Ok(i as u32);
        }
    }
    Ok((probs.len() - 1) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand::SeedableRng;

    #[test]
    fn sampling_picks_argmax_when_distribution_is_peaked() {
        let scores = [0.0, 0.1, 50.0, -3.0];
        let mut rng = ChaCha20Rng::seed_from_u64(1986);
        let cfg = SamplingConfig {
            temperature: 1.0,
            top_p: 0.9,
            top_k: 10,
            win_size: 10,
            tau_r: 1.0,
        };
        let id = sample_ras_next(&scores, &[], &cfg, &mut rng).unwrap();
        assert_eq!(id, 2);
    }

    #[test]
    fn ras_avoids_repeating_token_when_threshold_hit() {
        // Make token 0 the nucleus winner, but ensure we force it away via RAS.
        let scores = [10.0, 9.0, 8.0, 7.0];
        let decoded = vec![0u32; 10];
        let mut rng = ChaCha20Rng::seed_from_u64(1986);
        let cfg = SamplingConfig {
            temperature: 1.0,
            top_p: 0.9,
            top_k: 10,
            win_size: 10,
            tau_r: 1.0, // rep_num >= 10 * 1.0
        };
        let id = sample_ras_next(&scores, &decoded, &cfg, &mut rng).unwrap();
        assert_ne!(id, 0);
        assert!(id < scores.len() as u32);
    }

    #[test]
    fn temperature_must_be_positive() {
        let scores = [0.0, 1.0];
        let mut rng = ChaCha20Rng::seed_from_u64(1);
        let cfg = SamplingConfig {
            temperature: 0.0,
            ..Default::default()
        };
        let err = sample_ras_next(&scores, &[], &cfg, &mut rng).unwrap_err();
        assert!(matches!(err, TtsError::InvalidArg(_)));
    }
}
