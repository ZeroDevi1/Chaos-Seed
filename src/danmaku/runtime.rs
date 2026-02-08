use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, VecModel};

use chaos_seed::danmaku::sim;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DanmakuStyle {
    Chat,
    Overlay,
}

impl DanmakuStyle {
    pub fn from_index(idx: i32) -> Self {
        match idx {
            0 => Self::Chat,
            1 => Self::Overlay,
            _ => Self::Overlay,
        }
    }
}

pub struct DanmakuRuntime {
    style: DanmakuStyle,
    running: Rc<Cell<bool>>,
    gen_timer: slint::Timer,
    tick_timer: slint::Timer,
    interaction_poll_timer: slint::Timer,
    start: Instant,

    rng: fastrand::Rng,
    sim: sim::DanmakuSim,
    lane_sched: sim::LaneScheduler,

    next_id: i32,
    speed_px_per_ms: f32,
    rows: Vec<crate::DanmakuRow>,
    model: Rc<VecModel<crate::DanmakuRow>>,

    overlay: Option<crate::DanmakuOverlayWindow>,
    chat: Option<crate::DanmakuChatWindow>,
    win_width_px: f32,
}

impl DanmakuRuntime {
    pub fn start(
        app: &crate::AppWindow,
        style: DanmakuStyle,
        always_on_top: bool,
    ) -> Result<Self, slint::PlatformError> {
        let running = Rc::new(Cell::new(true));
        let start = Instant::now();

        let model: Rc<VecModel<crate::DanmakuRow>> = Rc::new(VecModel::default());
        let model_rc: ModelRc<crate::DanmakuRow> = ModelRc::from(model.clone());

        let speed_px_per_ms = 0.18f32;

        let mut rt = Self {
            style,
            running: running.clone(),
            gen_timer: slint::Timer::default(),
            tick_timer: slint::Timer::default(),
            interaction_poll_timer: slint::Timer::default(),
            start,
            rng: fastrand::Rng::new(),
            sim: sim::DanmakuSim::default(),
            lane_sched: sim::LaneScheduler::new(sim::LaneSchedulerConfig {
                lane_count: 10,
                min_spacing_px: 40.0,
                speed_px_per_ms,
            }),
            next_id: 1,
            speed_px_per_ms,
            rows: Vec::new(),
            model,
            overlay: None,
            chat: None,
            win_width_px: 960.0,
        };

        match style {
            DanmakuStyle::Overlay => {
                let w = crate::DanmakuOverlayWindow::new()?;
                // Overlay defaults to always-on-top regardless of the launcher checkbox.
                w.set_pin_on_top(true);
                w.set_speed_per_ms(speed_px_per_ms);
                w.set_messages(model_rc);
                w.window().set_size(slint::LogicalSize::new(960.0, 320.0));

                // Keep global theme consistent with the main window.
                w.global::<crate::AppTheme>()
                    .set_dark_mode(app.get_dark_mode());
                w.global::<crate::Palette>().set_color_scheme(if app.get_dark_mode() {
                    slint::language::ColorScheme::Dark
                } else {
                    slint::language::ColorScheme::Light
                });

                rt.overlay = Some(w);
            }
            DanmakuStyle::Chat => {
                let w = crate::DanmakuChatWindow::new()?;
                w.set_pin_on_top(always_on_top);
                w.set_messages(model_rc);
                w.window().set_size(slint::LogicalSize::new(420.0, 640.0));
                w.global::<crate::AppTheme>()
                    .set_dark_mode(app.get_dark_mode());
                w.global::<crate::Palette>().set_color_scheme(if app.get_dark_mode() {
                    slint::language::ColorScheme::Dark
                } else {
                    slint::language::ColorScheme::Light
                });
                rt.chat = Some(w);
            }
        }

        // Show after wiring. Timers are started by `install_timers()`.
        if let Some(w) = rt.overlay.as_ref() {
            w.show()?;
        }
        if let Some(w) = rt.chat.as_ref() {
            w.show()?;
        }
        Ok(rt)
    }

    /// Pause timers during window move/resize to improve perceived dragging smoothness.
    ///
    /// On Windows we poll the left mouse button and resume once the user releases it.
    pub fn begin_user_interaction(this: &Rc<RefCell<Self>>) {
        {
            let rt = this.borrow_mut();
            if !rt.running.get() {
                return;
            }
            rt.gen_timer.stop();
            if rt.style == DanmakuStyle::Overlay {
                rt.tick_timer.stop();
            }
        }

        let weak_rt = Rc::downgrade(this);
        this.borrow().interaction_poll_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(rc) = weak_rt.upgrade() else {
                    return;
                };
                if !rc.borrow().running.get() {
                    return;
                }
                if is_left_button_down() {
                    return;
                }
                {
                    rc.borrow().interaction_poll_timer.stop();
                }
                // Resume timers (tick only for overlay).
                DanmakuRuntime::start_tick_timer(&rc);
                DanmakuRuntime::arm_next_gen(&rc);
            },
        );
    }

    pub fn stop(&mut self) {
        self.running.set(false);
        self.gen_timer.stop();
        self.tick_timer.stop();
        self.interaction_poll_timer.stop();
        if let Some(w) = self.overlay.as_ref() {
            let _ = w.hide();
        }
        if let Some(w) = self.chat.as_ref() {
            let _ = w.hide();
        }
    }

    pub fn overlay_window(&self) -> Option<&crate::DanmakuOverlayWindow> {
        self.overlay.as_ref()
    }

    pub fn chat_window(&self) -> Option<&crate::DanmakuChatWindow> {
        self.chat.as_ref()
    }

    pub fn install_timers(this: &Rc<RefCell<Self>>) {
        Self::start_tick_timer(this);
        Self::arm_next_gen(this);
    }

    fn now_ms(&self) -> i32 {
        self.start
            .elapsed()
            .as_millis()
            .min(u128::from(i32::MAX as u32)) as i32
    }

    fn start_tick_timer(this: &Rc<RefCell<Self>>) {
        let (running, overlay, start) = {
            let rt = this.borrow();
            if rt.style != DanmakuStyle::Overlay {
                return;
            }
            (
                rt.running.clone(),
                rt.overlay.as_ref().expect("overlay window").as_weak(),
                rt.start,
            )
        };

        let weak_rt = Rc::downgrade(this);
        this.borrow().tick_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                if !running.get() {
                    return;
                }
                // Keep the overlay time ticking even if the runtime is about to be dropped.
                if let Some(w) = overlay.upgrade() {
                    let ms = start.elapsed().as_millis().min(u128::from(i32::MAX as u32)) as i32;
                    w.set_now_ms(ms);
                }
                // If the runtime has been dropped, stop ticking.
                if weak_rt.upgrade().is_none() {
                    return;
                }
            },
        );
    }

    fn arm_next_gen(this: &Rc<RefCell<Self>>) {
        let (delay, running) = {
            let mut rt = this.borrow_mut();
            // Avoid borrowing two fields of a `RefMut` at once.
            let mut rng = std::mem::replace(&mut rt.rng, fastrand::Rng::new());
            let d = rt.sim.next_delay(&mut rng);
            rt.rng = rng;
            (d, rt.running.clone())
        };

        let weak_rt = Rc::downgrade(this);
        this.borrow()
            .gen_timer
            .start(slint::TimerMode::SingleShot, delay, move || {
                if !running.get() {
                    return;
                }
                let Some(rc) = weak_rt.upgrade() else {
                    return;
                };
                {
                    let mut rt = rc.borrow_mut();
                    rt.gen_tick();
                    if !rt.running.get() {
                        return;
                    }
                }
                DanmakuRuntime::arm_next_gen(&rc);
            });
    }

    pub fn gen_tick(&mut self) {
        if !self.running.get() {
            return;
        }
        let now_ms = self.now_ms();

        // Trim expired rows for overlay.
        if self.style == DanmakuStyle::Overlay {
            // Keep width up-to-date so new messages compute a correct end time after resizing.
            if let Some(w) = self.overlay.as_ref() {
                let sf = w.window().scale_factor();
                let size = w.window().size().to_logical(sf);
                self.win_width_px = size.width.max(1.0);
            }

            let before = self.rows.len();
            self.rows.retain(|r| r.end_ms > now_ms);
            if self.rows.len() != before {
                self.model.set_vec(self.rows.clone());
            }
        }

        // Cap total rows to keep UI responsive.
        const MAX_ROWS: usize = 400;
        if self.rows.len() > MAX_ROWS {
            let keep = self
                .rows
                .split_off(self.rows.len().saturating_sub(MAX_ROWS));
            self.rows = keep;
            self.model.set_vec(self.rows.clone());
        }

        let (user, text) = random_message(&mut self.rng);
        // Overlay doesn't show the username, so the scheduler should estimate based on message text only.
        let width_px = if self.style == DanmakuStyle::Overlay {
            sim::estimate_width_px("", &text)
        } else {
            sim::estimate_width_px(&user, &text)
        };

        let (lane, start_ms, end_ms) = if self.style == DanmakuStyle::Overlay {
            let lane = self.lane_sched.pick_lane(now_ms) as i32;
            let start_ms = now_ms;
            let end_ms =
                sim::compute_end_ms(start_ms, self.win_width_px, width_px, self.speed_px_per_ms);
            self.lane_sched.reserve(lane as usize, start_ms, width_px);
            (lane, start_ms, end_ms)
        } else {
            (0, now_ms, now_ms.saturating_add(60_000))
        };

        let row = crate::DanmakuRow {
            id: self.next_id,
            user: user.into(),
            text: text.into(),
            image_url: "".into(),
            image_w: 0.0.into(),
            image: slint::Image::default(),
            image_ready: true,
            start_ms,
            end_ms,
            lane,
            width_est: width_px,
        };
        self.next_id = self.next_id.saturating_add(1);

        self.rows.push(row.clone());
        self.model.push(row);

        // Chat is rendered bottom-up in Slint, no scroll adjustment needed.
    }
}

#[cfg(windows)]
fn is_left_button_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn is_left_button_down() -> bool {
    // Best-effort fallback: assume the interaction ended immediately.
    false
}

static TEXT_POOL: &[&str] = &[
    "哈哈哈",
    "绷不住了",
    "这也太强了",
    "？？？",
    "来了来了",
    "高能预警",
    "爷青回",
    "救命",
    "笑死",
    "名场面",
    "这段太顶",
    "稳住",
    "别急",
    "卧槽",
    "神了",
    "太帅了",
    "我裂开",
    "泪目",
    "好耶",
    "冲冲冲",
    "燃起来了",
    "一整个爱住",
    "这谁顶得住",
    "弹幕护体",
    "回放走起",
    "别眨眼",
    "有点东西",
    "你是懂的",
    "合理",
    "不合理",
    "这都行",
    "离谱",
    "确实",
    "我懂了",
    "不懂",
    "再来一次",
];

fn random_message(rng: &mut fastrand::Rng) -> (String, String) {
    let user = format!("U{:04}", rng.u32(0..10_000));
    let text = TEXT_POOL[rng.usize(0..TEXT_POOL.len())].to_string();
    (user, text)
}
