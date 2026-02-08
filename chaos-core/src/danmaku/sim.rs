use std::time::Duration;

/// Controls how "human-like" simulated messages are produced.
///
/// The simulator alternates between:
/// - Normal mode: moderate frequency
/// - Hype mode: bursty high-frequency sequences
#[derive(Debug, Clone)]
pub struct SimConfig {
    pub normal_delay_ms: (u64, u64),
    pub hype_delay_ms: (u64, u64),
    pub hype_remaining: (u32, u32),
    pub hype_probability: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            normal_delay_ms: (600, 1500),
            hype_delay_ms: (40, 220),
            hype_remaining: (20, 80),
            hype_probability: 0.03,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimMode {
    Normal,
    Hype { remaining: u32 },
}

#[derive(Debug, Clone)]
pub struct DanmakuSim {
    pub config: SimConfig,
    pub mode: SimMode,
}

impl Default for DanmakuSim {
    fn default() -> Self {
        Self {
            config: SimConfig::default(),
            mode: SimMode::Normal,
        }
    }
}

impl DanmakuSim {
    pub fn new(config: SimConfig) -> Self {
        Self {
            config,
            mode: SimMode::Normal,
        }
    }

    /// Decide the delay until the next message should be generated.
    pub fn next_delay(&mut self, rng: &mut fastrand::Rng) -> Duration {
        match self.mode {
            SimMode::Normal => {
                let (lo, hi) = self.config.normal_delay_ms;
                let ms = rng.u64(lo..=hi);
                if rng.f32() < self.config.hype_probability {
                    let (rlo, rhi) = self.config.hype_remaining;
                    let remaining = rng.u32(rlo..=rhi);
                    self.mode = SimMode::Hype { remaining };
                }
                Duration::from_millis(ms)
            }
            SimMode::Hype { remaining } => {
                let (lo, hi) = self.config.hype_delay_ms;
                let ms = rng.u64(lo..=hi);
                if remaining <= 1 {
                    self.mode = SimMode::Normal;
                } else {
                    self.mode = SimMode::Hype {
                        remaining: remaining - 1,
                    };
                }
                Duration::from_millis(ms)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LaneSchedulerConfig {
    pub lane_count: usize,
    pub min_spacing_px: f32,
    pub speed_px_per_ms: f32,
}

impl Default for LaneSchedulerConfig {
    fn default() -> Self {
        Self {
            lane_count: 10,
            min_spacing_px: 40.0,
            speed_px_per_ms: 0.18,
        }
    }
}

/// A simple lane scheduler that tries to reduce overlaps.
#[derive(Debug, Clone)]
pub struct LaneScheduler {
    pub cfg: LaneSchedulerConfig,
    lane_next_free_ms: Vec<i32>,
}

impl LaneScheduler {
    pub fn new(cfg: LaneSchedulerConfig) -> Self {
        Self {
            lane_next_free_ms: vec![0; cfg.lane_count],
            cfg,
        }
    }

    pub fn lane_next_free_ms(&self) -> &[i32] {
        &self.lane_next_free_ms
    }

    pub fn pick_lane(&self, now_ms: i32) -> usize {
        // Prefer lanes already free (next_free <= now) and among them the earliest free.
        let mut best_free: Option<(usize, i32)> = None;
        let mut best_any: Option<(usize, i32)> = None;
        for (idx, &t) in self.lane_next_free_ms.iter().enumerate() {
            best_any = match best_any {
                None => Some((idx, t)),
                Some((bi, bt)) => Some(if t < bt { (idx, t) } else { (bi, bt) }),
            };
            if t <= now_ms {
                best_free = match best_free {
                    None => Some((idx, t)),
                    Some((bi, bt)) => Some(if t < bt { (idx, t) } else { (bi, bt) }),
                };
            }
        }
        best_free.or(best_any).map(|(i, _)| i).unwrap_or(0)
    }

    pub fn reserve(&mut self, lane: usize, start_ms: i32, width_est_px: f32) {
        let gap_ms = ((width_est_px + self.cfg.min_spacing_px) / self.cfg.speed_px_per_ms).ceil();
        let gap_ms = gap_ms.clamp(0.0, 1_000_000.0) as i32;
        if lane < self.lane_next_free_ms.len() {
            self.lane_next_free_ms[lane] = start_ms.saturating_add(gap_ms);
        }
    }
}

pub fn estimate_width_px(user: &str, text: &str) -> f32 {
    // Rough width estimate to feed the lane scheduler.
    let len = (user.chars().count() + 2 + text.chars().count()) as f32;
    12.0 * len * 0.6 + 24.0
}

pub fn compute_end_ms(
    start_ms: i32,
    win_width_px: f32,
    width_est_px: f32,
    speed_px_per_ms: f32,
) -> i32 {
    if speed_px_per_ms <= 0.0 {
        return start_ms.saturating_add(10_000);
    }
    let dur_ms = ((win_width_px + width_est_px) / speed_px_per_ms).ceil();
    start_ms.saturating_add(dur_ms.clamp(0.0, 10_000_000.0) as i32)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayRowMeta {
    pub id: i32,
    pub start_ms: i32,
    pub end_ms: i32,
}

pub fn cleanup_expired(rows: &mut Vec<OverlayRowMeta>, now_ms: i32) {
    rows.retain(|r| r.end_ms > now_ms);
}
