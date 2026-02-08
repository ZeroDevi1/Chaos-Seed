use chaos_core::danmaku::sim::*;

#[test]
fn normal_mode_produces_longer_delays() {
    let cfg = SimConfig::default();
    let mut sim = DanmakuSim::new(cfg);
    let mut rng = fastrand::Rng::with_seed(1);

    for _ in 0..200 {
        sim.mode = SimMode::Normal;
        let d = sim.next_delay(&mut rng);
        assert!(
            (600..=1500).contains(&d.as_millis().try_into().unwrap()),
            "delay_ms={}",
            d.as_millis()
        );
    }
}

#[test]
fn hype_mode_produces_short_delays_in_hype() {
    let cfg = SimConfig::default();
    let mut sim = DanmakuSim::new(cfg);
    let mut rng = fastrand::Rng::with_seed(2);

    sim.mode = SimMode::Hype { remaining: 10 };
    for _ in 0..10 {
        let d = sim.next_delay(&mut rng);
        assert!(
            (40..=220).contains(&d.as_millis().try_into().unwrap()),
            "delay_ms={}",
            d.as_millis()
        );
    }
}

#[test]
fn lane_scheduler_prefers_free_lane() {
    let mut sched = LaneScheduler::new(LaneSchedulerConfig {
        lane_count: 3,
        min_spacing_px: 40.0,
        speed_px_per_ms: 0.2,
    });

    // Reserve lane 0 and 1 into the future, lane 2 stays free.
    sched.reserve(0, 0, 500.0);
    sched.reserve(1, 0, 500.0);

    let lane = sched.pick_lane(10);
    assert_eq!(lane, 2);
}

#[test]
fn end_ms_is_after_start_ms_and_scales_with_width() {
    let start = 1000;
    let win_w = 960.0;
    let speed = 0.18;

    let end_small = compute_end_ms(start, win_w, 100.0, speed);
    let end_big = compute_end_ms(start, win_w, 1000.0, speed);
    assert!(end_small > start);
    assert!(end_big > end_small);
}

#[test]
fn cleanup_removes_expired_messages() {
    let mut rows = vec![
        OverlayRowMeta {
            id: 1,
            start_ms: 0,
            end_ms: 10,
        },
        OverlayRowMeta {
            id: 2,
            start_ms: 0,
            end_ms: 11,
        },
        OverlayRowMeta {
            id: 3,
            start_ms: 0,
            end_ms: 20,
        },
    ];
    cleanup_expired(&mut rows, 11);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 3);
}
