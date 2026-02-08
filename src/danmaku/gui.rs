use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, VecModel};

use chaos_seed::danmaku::model::{DanmakuComment, DanmakuEvent};
use chaos_seed::danmaku::sim;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayTarget {
    Main,
    Chat,
    Overlay,
}

pub struct DanmakuUiController {
    app: slint::Weak<crate::AppWindow>,
    task_tx: tokio::sync::mpsc::UnboundedSender<crate::TaskMsg>,

    // Keep a stable model instance; switch the app binding between `messages_model` and `empty_model`
    // to truly stop rendering on the main page when a floating window is active.
    messages_model: Rc<VecModel<crate::DanmakuRow>>,
    messages_model_rc: ModelRc<crate::DanmakuRow>,
    empty_model_rc: ModelRc<crate::DanmakuRow>,

    rows: Vec<crate::DanmakuRow>,
    row_index: HashMap<i32, usize>,
    next_row_id: i32,

    display: DisplayTarget,
    chat: Option<crate::DanmakuChatWindow>,
    overlay: Option<crate::DanmakuOverlayWindow>,

    // Overlay timing / lane scheduling (re-used from the simulator helpers).
    start: Instant,
    speed_px_per_ms: f32,
    win_width_px: f32,
    lane_sched: sim::LaneScheduler,
    overlay_tick_timer: slint::Timer,
    interaction_poll_timer: slint::Timer,
}

impl DanmakuUiController {
    pub fn new(
        app: slint::Weak<crate::AppWindow>,
        task_tx: tokio::sync::mpsc::UnboundedSender<crate::TaskMsg>,
        messages_model: Rc<VecModel<crate::DanmakuRow>>,
        empty_model: Rc<VecModel<crate::DanmakuRow>>,
    ) -> Self {
        let messages_model_rc = ModelRc::from(messages_model.clone());
        let empty_model_rc = ModelRc::from(empty_model.clone());

        Self {
            app,
            task_tx,
            messages_model,
            messages_model_rc,
            empty_model_rc,
            rows: Vec::new(),
            row_index: HashMap::new(),
            next_row_id: 1,
            display: DisplayTarget::Main,
            chat: None,
            overlay: None,
            start: Instant::now(),
            speed_px_per_ms: 0.18,
            win_width_px: 960.0,
            lane_sched: sim::LaneScheduler::new(sim::LaneSchedulerConfig {
                lane_count: 10,
                min_spacing_px: 40.0,
                speed_px_per_ms: 0.18,
            }),
            overlay_tick_timer: slint::Timer::default(),
            interaction_poll_timer: slint::Timer::default(),
        }
    }

    pub fn reset_for_new_session(&mut self) {
        self.rows.clear();
        self.row_index.clear();
        self.messages_model.clear();
        self.next_row_id = 1;

        self.start = Instant::now();
        self.win_width_px = 960.0;
        self.lane_sched = sim::LaneScheduler::new(sim::LaneSchedulerConfig {
            lane_count: 10,
            min_spacing_px: 40.0,
            speed_px_per_ms: self.speed_px_per_ms,
        });
    }

    fn now_ms(&self) -> i32 {
        self.start
            .elapsed()
            .as_millis()
            .min(u128::from(i32::MAX as u32)) as i32
    }

    fn overlay_window_width_px(&mut self) -> f32 {
        let Some(w) = self.overlay.as_ref() else {
            return self.win_width_px;
        };
        let sf = w.window().scale_factor();
        let size = w.window().size().to_logical(sf);
        self.win_width_px = size.width.max(1.0);
        self.win_width_px
    }

    fn clamp_thumb_width_px(width: Option<u32>) -> f32 {
        let Some(w) = width else {
            return 0.0;
        };
        if w == 0 {
            return 0.0;
        }
        (w as f32).clamp(18.0, 80.0)
    }

    fn default_thumb_width_px() -> f32 {
        // Keep thumbnails visible even when the platform doesn't provide width metadata.
        // This matches the default row height used in the .slint chat views.
        26.0
    }

    fn rebuild_index(&mut self) {
        self.row_index.clear();
        for (i, r) in self.rows.iter().enumerate() {
            self.row_index.insert(r.id, i);
        }
    }

    pub fn handle_event(&mut self, ev: DanmakuEvent) {
        // Handshake events: ok = empty payload, error is handled by the UI layer.
        if ev.text.is_empty() && ev.dms.is_none() {
            return;
        }

        let user = if ev.user.trim().is_empty() {
            "？".to_string()
        } else {
            ev.user
        };

        let mut comments: Vec<DanmakuComment> = Vec::new();
        if let Some(dms) = ev.dms {
            comments.extend(dms);
        } else if !ev.text.trim().is_empty() {
            comments.push(DanmakuComment::text(ev.text));
        }

        if comments.is_empty() {
            return;
        }

        let now_ms = self.now_ms();
        let win_w = self.overlay_window_width_px();

        for c in comments {
            let image_url = c.image_url.clone().unwrap_or_default();
            let mut text = c.text.clone();
            if text.trim().is_empty() && !image_url.is_empty() {
                text = "[图片]".to_string();
            }

            let overlay_text = if text.trim().is_empty() {
                "[弹幕]".to_string()
            } else {
                text.clone()
            };

            let width_px = sim::estimate_width_px("", &overlay_text);
            let lane = self.lane_sched.pick_lane(now_ms) as i32;
            let start_ms = now_ms;
            let end_ms = sim::compute_end_ms(start_ms, win_w, width_px, self.speed_px_per_ms);
            self.lane_sched.reserve(lane as usize, start_ms, width_px);

            let id = self.next_row_id;
            self.next_row_id = self.next_row_id.saturating_add(1);

            let row = crate::DanmakuRow {
                id,
                user: user.clone().into(),
                text: text.into(),
                image_url: image_url.clone().into(),
                image_w: (if image_url.is_empty() {
                    0.0
                } else {
                    Self::clamp_thumb_width_px(c.image_width).max(Self::default_thumb_width_px())
                })
                .into(),
                image: slint::Image::default(),
                image_ready: image_url.is_empty(),
                start_ms,
                end_ms,
                lane,
                width_est: width_px.into(),
            };

            self.row_index.insert(id, self.rows.len());
            self.rows.push(row.clone());
            self.messages_model.push(row);

            if !image_url.is_empty() {
                let _ = self.task_tx.send(crate::TaskMsg::DanmakuLoadImage {
                    row_id: id,
                    url: image_url,
                });
            }
        }

        const MAX_ROWS: usize = 400;
        if self.rows.len() > MAX_ROWS {
            let keep = self
                .rows
                .split_off(self.rows.len().saturating_sub(MAX_ROWS));
            self.rows = keep;
            self.messages_model.set_vec(self.rows.clone());
            self.rebuild_index();
        }
    }

    pub fn apply_image(&mut self, row_id: i32, w: u32, h: u32, pixels: Vec<slint::Rgba8Pixel>) {
        let Some(&idx) = self.row_index.get(&row_id) else {
            return;
        };

        let mut buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
        buffer.make_mut_slice().copy_from_slice(&pixels);
        let img = slint::Image::from_rgba8(buffer);

        let mut row = match self.rows.get(idx).cloned() {
            Some(r) => r,
            None => return,
        };
        row.image = img;
        row.image_ready = true;

        // Update thumbnail width from actual aspect ratio (helps platforms that don't provide width metadata).
        if !row.image_url.trim().is_empty() && h > 0 {
            let row_h = Self::default_thumb_width_px();
            let aspect = (w as f32) / (h as f32);
            let w2 = (aspect * row_h).clamp(18.0, 80.0);
            row.image_w = w2.into();
        }
        self.rows[idx] = row.clone();

        // VecModel implements the Model trait, so we need it in scope for `set_row_data`.
        use slint::Model as _;
        self.messages_model.set_row_data(idx, row);
    }

    pub fn open_chat_window(this: &Rc<RefCell<Self>>) {
        let (app, msg_model) = {
            let ctrl = this.borrow();
            (ctrl.app.clone(), ctrl.messages_model_rc.clone())
        };
        let Some(app) = app.upgrade() else {
            return;
        };

        // Ensure overlay is not active.
        Self::return_to_main(this);

        let (wweak, empty_model_rc) = {
            let mut ctrl = this.borrow_mut();
            if ctrl.chat.is_none() {
                let Ok(w) = crate::DanmakuChatWindow::new() else {
                    return;
                };
                // Workaround: on some backends, frameless window hit-testing is unreliable until
                // the window receives a size update. Force an initial size here.
                w.window().set_size(slint::LogicalSize::new(420.0, 640.0));
                ctrl.chat = Some(w);
            }
            ctrl.display = DisplayTarget::Chat;
            (
                ctrl.chat.as_ref().expect("chat window").as_weak(),
                ctrl.empty_model_rc.clone(),
            )
        };

        if let Some(w) = wweak.upgrade() {
            w.set_messages(msg_model);

            // Theme sync with the main window.
            w.global::<crate::AppTheme>()
                .set_dark_mode(app.get_dark_mode());
            w.global::<crate::Palette>()
                .set_color_scheme(if app.get_dark_mode() {
                    slint::language::ColorScheme::Dark
                } else {
                    slint::language::ColorScheme::Light
                });

            // Window handlers.
            Self::install_chat_handlers(this, &w);

            let _ = w.show();

            // Additional hit-test warmup: re-apply current size in the next tick.
            // This mimics the "manual resize" that users reported as making buttons clickable.
            let w2 = w.as_weak();
            slint::Timer::single_shot(Duration::from_millis(0), move || {
                force_hit_test_refresh(w2.clone());
            });
        }

        // Bind main view to empty model to stop rendering.
        app.set_danmaku_messages(empty_model_rc);
    }

    pub fn open_overlay_window(this: &Rc<RefCell<Self>>) {
        let (app, msg_model, speed) = {
            let ctrl = this.borrow();
            (
                ctrl.app.clone(),
                ctrl.messages_model_rc.clone(),
                ctrl.speed_px_per_ms,
            )
        };
        let Some(app) = app.upgrade() else {
            return;
        };

        // Ensure chat is not active.
        Self::return_to_main(this);

        let (wweak, empty_model_rc) = {
            let mut ctrl = this.borrow_mut();
            if ctrl.overlay.is_none() {
                let Ok(w) = crate::DanmakuOverlayWindow::new() else {
                    return;
                };
                w.window().set_size(slint::LogicalSize::new(960.0, 320.0));
                ctrl.overlay = Some(w);
            }
            ctrl.display = DisplayTarget::Overlay;
            (
                ctrl.overlay.as_ref().expect("overlay window").as_weak(),
                ctrl.empty_model_rc.clone(),
            )
        };

        if let Some(w) = wweak.upgrade() {
            w.set_speed_per_ms(speed);
            w.set_messages(msg_model);

            // Theme sync with the main window (even if overlay is transparent).
            w.global::<crate::AppTheme>()
                .set_dark_mode(app.get_dark_mode());
            w.global::<crate::Palette>()
                .set_color_scheme(if app.get_dark_mode() {
                    slint::language::ColorScheme::Dark
                } else {
                    slint::language::ColorScheme::Light
                });

            // Window handlers.
            Self::install_overlay_handlers(this, &w);

            let _ = w.show();

            let w2 = w.as_weak();
            slint::Timer::single_shot(Duration::from_millis(0), move || {
                force_hit_test_refresh(w2.clone());
            });
        }

        // Bind main view to empty model to stop rendering.
        app.set_danmaku_messages(empty_model_rc);

        // Start ticking the overlay time.
        this.borrow_mut().start_overlay_tick(this);
    }

    fn start_overlay_tick(&mut self, this: &Rc<RefCell<Self>>) {
        let Some(w) = self.overlay.as_ref() else {
            return;
        };
        let wweak = w.as_weak();
        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);

        self.overlay_tick_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(ctrl_rc) = ctrl_weak.upgrade() else {
                    return;
                };
                let now_ms = ctrl_rc.borrow().now_ms();

                if let Some(w) = wweak.upgrade() {
                    w.set_now_ms(now_ms);
                } else {
                    ctrl_rc.borrow_mut().overlay_tick_timer.stop();
                }
            },
        );
    }

    fn install_chat_handlers(this: &Rc<RefCell<Self>>, w: &crate::DanmakuChatWindow) {
        let wweak = w.as_weak();
        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        w.on_begin_drag(move || {
            if let Some(w) = wweak.upgrade() {
                // When we hand over to a native window move loop, the backend may not forward the
                // corresponding left-button release back to Slint. That can leave the initial
                // TouchArea in a stuck "pressed/captured" state, causing subsequent clicks to be
                // interpreted as drags until some other event (right-click/resize) resets it.
                cancel_left_button_in_slint(&w.window());
                crate::begin_native_move(&w.window());
            }
            if let Some(rc) = ctrl_weak.upgrade() {
                DanmakuUiController::poll_until_mouse_released_then_refresh_hit_test(
                    &rc,
                    wweak.clone(),
                );
            }
        });

        // Workaround: toggling always-on-top can also leave frameless hit-testing stale until a resize.
        let wweak = w.as_weak();
        w.on_pin_toggled(move || {
            let w2 = wweak.clone();
            slint::Timer::single_shot(Duration::from_millis(0), move || {
                force_hit_test_refresh(w2.clone());
            });
        });

        w.on_open_url(|url| {
            let _ = open::that(url.as_str());
        });

        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        w.on_close_clicked(move || {
            if let Some(rc) = ctrl_weak.upgrade() {
                DanmakuUiController::schedule_return_to_main(&rc);
            }
        });

        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        w.window().on_close_requested(move || {
            if let Some(rc) = ctrl_weak.upgrade() {
                DanmakuUiController::schedule_return_to_main(&rc);
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    fn install_overlay_handlers(this: &Rc<RefCell<Self>>, w: &crate::DanmakuOverlayWindow) {
        let wweak = w.as_weak();
        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        w.on_begin_drag(move || {
            if let Some(w) = wweak.upgrade() {
                cancel_left_button_in_slint(&w.window());
                crate::begin_native_move(&w.window());
            }
            if let Some(rc) = ctrl_weak.upgrade() {
                DanmakuUiController::poll_until_mouse_released_then_refresh_hit_test(
                    &rc,
                    wweak.clone(),
                );
            }
        });

        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        w.on_close_clicked(move || {
            if let Some(rc) = ctrl_weak.upgrade() {
                DanmakuUiController::schedule_return_to_main(&rc);
            }
        });

        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        w.window().on_close_requested(move || {
            if let Some(rc) = ctrl_weak.upgrade() {
                DanmakuUiController::schedule_return_to_main(&rc);
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    fn schedule_return_to_main(this: &Rc<RefCell<Self>>) {
        // Defer the cleanup to avoid dropping/hiding a window component from within its own callback.
        let weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        slint::Timer::single_shot(Duration::from_millis(0), move || {
            if let Some(rc) = weak.upgrade() {
                DanmakuUiController::return_to_main(&rc);
            }
        });
    }

    pub fn return_to_main(this: &Rc<RefCell<Self>>) {
        let Some(app) = this.borrow().app.upgrade() else {
            return;
        };

        // Stop overlay ticking if we were in overlay mode.
        {
            let mut ctrl = this.borrow_mut();
            if ctrl.display == DisplayTarget::Overlay {
                ctrl.overlay_tick_timer.stop();
            }
            ctrl.display = DisplayTarget::Main;
        }

        // Hide windows if present.
        if let Some(w) = this.borrow().chat.as_ref() {
            let _ = w.hide();
        }
        if let Some(w) = this.borrow().overlay.as_ref() {
            let _ = w.hide();
        }

        // Restore main messages.
        let msg_model = this.borrow().messages_model_rc.clone();
        app.set_danmaku_messages(msg_model);
    }

    fn poll_until_mouse_released_then_refresh_hit_test<T>(
        this: &Rc<RefCell<Self>>,
        win: slint::Weak<T>,
    ) where
        T: ComponentHandle + 'static,
    {
        // Stop any previous interaction poll.
        this.borrow_mut().interaction_poll_timer.stop();

        // Fast-path: if the button is already released (e.g. native move call was blocking),
        // refresh immediately instead of waiting for the first timer tick.
        if !is_left_button_down() {
            if let Some(w) = win.upgrade() {
                force_hit_test_refresh(w.as_weak());
            }
            return;
        }

        let ctrl_weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        this.borrow_mut().interaction_poll_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(ctrl) = ctrl_weak.upgrade() else {
                    return;
                };

                // Wait until the user releases the left mouse button (end of move/drag).
                if is_left_button_down() {
                    return;
                }

                ctrl.borrow_mut().interaction_poll_timer.stop();

                // Workaround: some backends don't refresh hit-testing after native move / always-on-top
                // changes until a resize event occurs. Re-apply the current size to force a refresh.
                if let Some(w) = win.upgrade() {
                    force_hit_test_refresh(w.as_weak());
                }
            },
        );
    }
}

fn cancel_left_button_in_slint(window: &slint::Window) {
    // Synthetic release to clear any pressed/captured TouchArea state before native move starts.
    // Use a position inside the window; if a pointer grab exists, it'll receive the event anyway.
    window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: slint::LogicalPosition::new(1.0, 1.0),
        button: slint::platform::PointerEventButton::Left,
    });
    window.dispatch_event(slint::platform::WindowEvent::PointerExited);
}

fn force_hit_test_refresh<T>(win: slint::Weak<T>)
where
    T: ComponentHandle + 'static,
{
    // Some backend/platform combos (notably frameless windows on Windows) can end up with stale
    // hit-testing after a native move (and sometimes after always-on-top changes). A manual resize
    // fixes it, so we emulate that by nudging the window size by 1px and restoring it.
    //
    // Important: do the restore in the next tick; setting size twice synchronously may be coalesced.
    let Some(w) = win.upgrade() else {
        return;
    };
    let window = w.window();
    let sf = window.scale_factor();
    let size_px = window.size();
    let size = size_px.to_logical(sf);

    window.set_size(slint::LogicalSize::new(size.width + 1.0, size.height));
    let win2 = win.clone();
    slint::Timer::single_shot(Duration::from_millis(0), move || {
        if let Some(w) = win2.upgrade() {
            w.window().set_size(size);
            w.window().request_redraw();
        }
    });
}

#[cfg(windows)]
fn is_left_button_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn is_left_button_down() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::force_hit_test_refresh;
    use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
    use slint::platform::{Platform, PlatformError, WindowAdapter};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Once;

    thread_local! {
        static LAST_WINDOW: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(None) };
    }

    #[derive(Default)]
    struct TestPlatform;

    impl Platform for TestPlatform {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
            LAST_WINDOW.with(|cell| {
                *cell.borrow_mut() = Some(window.clone());
            });
            Ok(window)
        }
    }

    slint::slint! {
        export component TestWindow inherits Window { }
    }

    #[test]
    fn force_hit_test_refresh_restores_size() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            slint::platform::set_platform(Box::new(TestPlatform::default()))
                .expect("set slint test platform");
        });

        let ui = TestWindow::new().expect("create ui");
        let adapter = LAST_WINDOW
            .with(|cell| cell.borrow().clone())
            .expect("window adapter created");

        adapter.set_size(slint::PhysicalSize::new(420, 360));
        let before = ui.window().size();

        force_hit_test_refresh(ui.as_weak());
        slint::platform::update_timers_and_animations();

        let after = ui.window().size();
        assert_eq!(after.width, before.width);
        assert_eq!(after.height, before.height);
    }
}
