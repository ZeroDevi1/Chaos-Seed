use std::cell::RefCell;
use std::rc::Rc;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ModelRc, SharedPixelBuffer, VecModel};

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

fn dark_pixel_count_bottom_half(buffer: &SharedPixelBuffer<slint::Rgb8Pixel>) -> usize {
    let w = buffer.width() as usize;
    let h = buffer.height() as usize;
    let pixels = buffer.as_slice();

    let mut count = 0usize;
    for y in (h / 2)..h {
        let row = &pixels[(y * w)..((y + 1) * w)];
        for p in row {
            if p.r < 80 && p.g < 80 && p.b < 80 {
                count += 1;
            }
        }
    }
    count
}

slint::slint! {
import { DanmakuView } from "ui/views/danmaku.slint";
import { DanmakuRow } from "ui/models.slint";
import { AppTheme } from "ui/theme.slint";

export component TestWindow inherits Window {
    background: AppTheme.app_bg;

    in-out property <string> input;
    in-out property <string> status;
    in-out property <bool> connected;
    in property <[DanmakuRow]> messages;

    DanmakuView {
        width: parent.width;
        height: parent.height;
        input <=> root.input;
        status <=> root.status;
        connected <=> root.connected;
        messages: root.messages;
    }
}
}

#[test]
fn danmaku_messages_render_in_bottom_area_and_can_be_unbound() {
    slint::platform::set_platform(Box::new(TestPlatform::default()))
        .expect("set slint test platform");

    let ui = TestWindow::new().expect("create ui");
    let window = LAST_WINDOW
        .with(|cell| cell.borrow().clone())
        .expect("window adapter created");
    window.set_size(slint::PhysicalSize::new(1000, 700));

    let messages_model: Rc<VecModel<DanmakuRow>> = Rc::new(VecModel::default());
    let empty_model: Rc<VecModel<DanmakuRow>> = Rc::new(VecModel::default());
    ui.set_messages(ModelRc::from(messages_model.clone()));

    // Render with empty messages.
    ui.window().request_redraw();
    slint::platform::update_timers_and_animations();
    let mut empty = SharedPixelBuffer::<slint::Rgb8Pixel>::new(1000, 700);
    let stride = empty.width() as usize;
    window.draw_if_needed(|renderer| {
        let _ = renderer.render(empty.make_mut_slice(), stride);
    });
    let empty_dark = dark_pixel_count_bottom_half(&empty);

    // Fill messages.
    let rows = (0..25)
        .map(|i| DanmakuRow {
            id: i,
            user: format!("U{i}").into(),
            text: "hello world".into(),
            image_url: "".into(),
            image_w: 0.0.into(),
            image: slint::Image::default(),
            image_ready: true,
            start_ms: 0,
            end_ms: 0,
            lane: 0,
            width_est: 0.0.into(),
        })
        .collect::<Vec<_>>();
    messages_model.set_vec(rows);

    ui.window().request_redraw();
    slint::platform::update_timers_and_animations();
    let mut filled = SharedPixelBuffer::<slint::Rgb8Pixel>::new(1000, 700);
    let stride = filled.width() as usize;
    window.draw_if_needed(|renderer| {
        let _ = renderer.render(filled.make_mut_slice(), stride);
    });
    let filled_dark = dark_pixel_count_bottom_half(&filled);

    assert!(
        filled_dark > empty_dark + 200,
        "expected significantly more dark pixels with messages rendered, got empty={empty_dark}, filled={filled_dark}"
    );

    // Unbind (simulate switching to a floating window).
    ui.set_messages(ModelRc::from(empty_model));
    ui.window().request_redraw();
    slint::platform::update_timers_and_animations();
    let mut unbound = SharedPixelBuffer::<slint::Rgb8Pixel>::new(1000, 700);
    let stride = unbound.width() as usize;
    window.draw_if_needed(|renderer| {
        let _ = renderer.render(unbound.make_mut_slice(), stride);
    });
    let unbound_dark = dark_pixel_count_bottom_half(&unbound);

    assert!(
        unbound_dark + 200 < filled_dark,
        "expected significantly fewer dark pixels after unbinding, got filled={filled_dark}, unbound={unbound_dark}"
    );
}
