use rand::Rng;

use crate::tts::TtsError;

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
    let scores: Vec<f32> = weighted_scores.iter().map(|x| x / cfg.temperature).collect();

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
        let mut scores2 = scores;
        if let Some(s) = scores2.get_mut(top_id as usize) {
            *s = f32::NEG_INFINITY;
        }
        return random_sampling(&scores2, rng);
    }

    Ok(top_id)
}

fn random_sampling(scores: &[f32], rng: &mut impl Rng) -> Result<u32, TtsError> {
    // 直接从 softmax 分布采样：避免分配 `Vec<f32>`（V≈6761，逐 token 分配会很慢）。
    sample_from_softmax(scores, rng)
}

fn nucleus_sampling(
    scores: &[f32],
    top_p: f32,
    top_k: usize,
    rng: &mut impl Rng,
) -> Result<u32, TtsError> {
    if scores.is_empty() {
        return Err(TtsError::InvalidArg("scores is empty".into()));
    }
    if top_k == 0 {
        return Err(TtsError::InvalidArg("top_k must be > 0".into()));
    }

    // top_k=1 时等价于 argmax（并且老实现也会退化为永远选第一个最大项）。
    // 这里直接走 argmax，避免 softmax + 排序开销；对齐 tie-break：索引更小者优先。
    if top_k == 1 {
        let mut best_i = 0usize;
        let mut best_v = f32::NEG_INFINITY;
        for (i, &x) in scores.iter().enumerate() {
            if x.is_finite() {
                if x > best_v || (x == best_v && i < best_i) {
                    best_v = x;
                    best_i = i;
                }
            }
        }
        // 兼容性：参考实现即使 top_k=1 也会走一次 `sample_from_probs` 消耗 RNG。
        // 这里同样消耗一次随机数，保证在同一 seed 下整体 token 序列可对齐。
        let _ = sample_from_probs(&[1.0], rng)?;
        // 若全部为 -inf/NaN：老实现 softmax->uniform，再 top_k=1 会固定选 index=0。
        return Ok(best_i as u32);
    }

    let v = scores.len();
    let k = top_k.min(v);

    // 1) 找 max（稳定 softmax）；若全是 -inf，则退化为 uniform。
    let mut max = f32::NEG_INFINITY;
    for &x in scores {
        if x.is_finite() && x > max {
            max = x;
        }
    }
    if !max.is_finite() {
        // 全 -inf -> uniform。老实现会按 index 升序选出若干 token（受 top_p/top_k 影响），再在其中随机采样。
        // 这里复刻：chosen = [0,1,2,...]，直到 cum >= top_p 或 len==k（均匀分布时 cum= len/v）。
        let mut chosen_len = 0usize;
        let mut cum = 0.0f32;
        while chosen_len < k && cum < top_p {
            chosen_len += 1;
            cum += 1.0 / (v as f32);
        }
        let chosen_len = chosen_len.max(1);
        // 从 [0..chosen_len) 均匀采样（用同一套 `sample_from_probs`，保证与参考实现完全一致）。
        let probs = vec![1.0f32 / (chosen_len as f32); chosen_len];
        let pick = sample_from_probs(&probs, rng)? as usize;
        return Ok(pick.min(chosen_len - 1) as u32);
    }

    // 2) 计算 softmax 分母（sum_exp）并同时维护 top-k 候选（按 score 降序、index 升序稳定排序）。
    let mut sum_exp = 0.0f64;
    // top-k 候选：按「可参与 softmax 的分数」排序；非有限值在参考实现中会被当作 0 概率，因此这里当作 -inf 参与比较。
    let mut top: Vec<(usize, f32)> = Vec::with_capacity(k);
    for (i, &x) in scores.iter().enumerate() {
        let e = if x.is_finite() { (x - max).exp() as f64 } else { 0.0 };
        sum_exp += e;

        let rank = if x.is_finite() { x } else { f32::NEG_INFINITY };

        // 稳定 top-k：O(V*K)，但 K 很小（默认 20），比全量 sort 快很多。
        if top.len() < k {
            top.push((i, rank));
            // 插入后做一次局部冒泡，维持排序。
            let mut j = top.len() - 1;
            while j > 0 {
                let (pi, pv) = top[j - 1];
                let (ci, cv) = top[j];
                let better = cv > pv || (cv == pv && ci < pi);
                if better {
                    top.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        } else {
            let (wi, wv) = top[k - 1];
            let better = rank > wv || (rank == wv && i < wi);
            if better {
                top[k - 1] = (i, rank);
                // 往前冒泡修复顺序。
                let mut j = k - 1;
                while j > 0 {
                    let (pi, pv) = top[j - 1];
                    let (ci, cv) = top[j];
                    let better2 = cv > pv || (cv == pv && ci < pi);
                    if better2 {
                        top.swap(j - 1, j);
                        j -= 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    if sum_exp == 0.0 {
        // 极端：max 有限但 sum_exp=0（例如全是极小值溢出成 0）。退回 uniform。
        return random_sampling(scores, rng);
    }

    // 3) 在 top-k 中做 nucleus（按概率降序 === 按 logit 降序），累积到 top_p。
    let mut chosen: Vec<(usize, f32)> = Vec::with_capacity(k);
    let mut cum = 0.0f32;
    for (i, _rank) in top {
        if chosen.len() >= k || cum >= top_p {
            break;
        }
        let x = scores[i];
        let p = if x.is_finite() {
            (((x - max).exp() as f64) / sum_exp) as f32
        } else {
            0.0
        };
        chosen.push((i, p));
        cum += p;
    }
    if chosen.is_empty() {
        // Degenerate: fall back to full distribution.
        return random_sampling(scores, rng);
    }

    // 4) 在 chosen 内按概率重归一后采样。
    let sum: f32 = chosen.iter().map(|(_, p)| *p).sum();
    if sum <= 0.0 {
        // 全 0 -> 退化为均匀。
        let probs = vec![1.0f32 / (chosen.len() as f32); chosen.len()];
        let pick = sample_from_probs(&probs, rng)? as usize;
        return Ok(chosen[pick.min(chosen.len() - 1)].0 as u32);
    }
    let mut c_probs: Vec<f32> = chosen.iter().map(|(_, p)| *p / sum).collect();
    // 理论上 sum>0 时这里已经归一了；但为了避免浮点累计误差，做一次归一化（与参考实现一致）。
    let sum2: f32 = c_probs.iter().sum();
    if sum2 > 0.0 {
        for p in &mut c_probs {
            *p /= sum2;
        }
    }
    let pick = sample_from_probs(&c_probs, rng)? as usize;
    Ok(chosen[pick.min(chosen.len() - 1)].0 as u32)
}

#[cfg(test)]
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

fn sample_from_softmax(scores: &[f32], rng: &mut impl Rng) -> Result<u32, TtsError> {
    if scores.is_empty() {
        return Err(TtsError::InvalidArg("scores is empty".into()));
    }

    // 1) max
    let mut max = f32::NEG_INFINITY;
    for &x in scores {
        if x.is_finite() && x > max {
            max = x;
        }
    }

    // 全 -inf -> uniform
    if !max.is_finite() {
        let mut r: f32 = rng.r#gen::<f32>();
        if r >= 1.0 {
            r = 0.99999994;
        }
        let pick = (r * (scores.len() as f32)).floor() as usize;
        return Ok(pick.min(scores.len() - 1) as u32);
    }

    // 2) sum_exp
    let mut sum_exp = 0.0f64;
    for &x in scores {
        if x.is_finite() {
            sum_exp += (x - max).exp() as f64;
        }
    }
    if sum_exp == 0.0 {
        // 退化：按均匀采样。
        let mut r: f32 = rng.r#gen::<f32>();
        if r >= 1.0 {
            r = 0.99999994;
        }
        let pick = (r * (scores.len() as f32)).floor() as usize;
        return Ok(pick.min(scores.len() - 1) as u32);
    }

    // 3) 采样：等价于在 [0, sum_exp) 上采样并落到累计区间。
    let mut r: f32 = rng.r#gen::<f32>();
    if r >= 1.0 {
        r = 0.99999994;
    }
    let target = (r as f64) * sum_exp;
    let mut cum = 0.0f64;
    for (i, &x) in scores.iter().enumerate() {
        if x.is_finite() {
            cum += (x - max).exp() as f64;
        }
        if target < cum {
            return Ok(i as u32);
        }
    }
    Ok((scores.len() - 1) as u32)
}

fn sample_from_probs(probs: &[f32], rng: &mut impl Rng) -> Result<u32, TtsError> {
    if probs.is_empty() {
        return Err(TtsError::InvalidArg("probs is empty".into()));
    }
    // Rust 2024: `gen` is a reserved keyword, use raw identifier.
    let mut r: f32 = rng.r#gen::<f32>();
    // Clamp in case RNG returns 1.0 exactly.
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
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

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
        // Make tokens 2 repeat many times.
        let scores = [0.0, 0.1, 50.0, -3.0];
        let mut rng = ChaCha20Rng::seed_from_u64(1986);
        let cfg = SamplingConfig {
            temperature: 1.0,
            top_p: 0.9,
            top_k: 10,
            win_size: 4,
            tau_r: 0.75,
        };
        let decoded = vec![2u32, 2, 2, 2];
        let id = sample_ras_next(&scores, &decoded, &cfg, &mut rng).unwrap();
        assert_ne!(id, 2);
    }

    fn nucleus_sampling_reference(
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
        // Renormalize.
        let sum: f32 = c_probs.iter().sum();
        if sum > 0.0 {
            for p in &mut c_probs {
                *p /= sum;
            }
        }

        let pick = sample_from_probs(&c_probs, rng)? as usize;
        Ok(chosen[pick].0 as u32)
    }

    #[test]
    fn nucleus_sampling_fast_matches_reference() {
        let mut rng1 = ChaCha20Rng::seed_from_u64(1986);
        let mut rng2 = ChaCha20Rng::seed_from_u64(1986);

        // 覆盖：常规、极端、以及包含 NaN/inf 的情况。
        let cases: Vec<Vec<f32>> = vec![
            vec![0.0, 0.1, 50.0, -3.0],
            vec![f32::NEG_INFINITY; 32],
            {
                let mut v = vec![0.0f32; 128];
                v[3] = 10.0;
                v[5] = 9.0;
                v
            },
            {
                let mut v = vec![0.0f32; 64];
                v[10] = f32::NAN;
                v[11] = f32::INFINITY;
                v[12] = f32::NEG_INFINITY;
                v[13] = 5.0;
                v
            },
        ];

        let params = [
            (1.0, 1),
            (1.0, 20),
            (0.9, 20),
            (0.75, 20),
            (0.6, 10),
        ];

        for scores in cases {
            for (top_p, top_k) in params {
                let a = nucleus_sampling_reference(&scores, top_p, top_k, &mut rng1).unwrap();
                let b = nucleus_sampling(&scores, top_p, top_k, &mut rng2).unwrap();
                assert_eq!(
                    a, b,
                    "mismatch: top_p={top_p} top_k={top_k} len={}",
                    scores.len()
                );
            }
        }
    }
}

