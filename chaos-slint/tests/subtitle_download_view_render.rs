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
            // Count only "real dark" pixels so muted hint text doesn't skew the baseline.
            if p.r < 80 && p.g < 80 && p.b < 80 {
                count += 1;
            }
        }
    }
    count
}

slint::slint! {
import { SubtitleDownloadView } from "ui/views/subtitle_download.slint";
import { SubtitleRow } from "ui/models.slint";
import { AppTheme } from "ui/theme.slint";

export component TestWindow inherits Window {
    background: AppTheme.app_bg;
    in property <[SubtitleRow]> results;

    SubtitleDownloadView {
        width: parent.width;
        height: parent.height;
        results: root.results;
        status_text: "";
        busy: false;
    }
}
}

#[test]
fn subtitle_results_render_in_bottom_area() {
    // Use a headless software-renderer platform so we can render + assert pixels in CI.
    slint::platform::set_platform(Box::new(TestPlatform)).expect("set slint test platform");

    let ui = TestWindow::new().expect("create ui");
    let window = LAST_WINDOW
        .with(|cell| cell.borrow().clone())
        .expect("window adapter created");
    window.set_size(slint::PhysicalSize::new(1000, 700));

    let results_model: Rc<VecModel<SubtitleRow>> = Rc::new(VecModel::default());
    ui.set_results(ModelRc::from(results_model.clone()));

    // Render with empty results.
    ui.window().request_redraw();
    slint::platform::update_timers_and_animations();
    let mut empty = SharedPixelBuffer::<slint::Rgb8Pixel>::new(1000, 700);
    let stride = empty.width() as usize;
    window.draw_if_needed(|renderer| {
        let _ = renderer.render(empty.make_mut_slice(), stride);
    });
    let empty_dark = dark_pixel_count_bottom_half(&empty);

    // Render with non-empty results.
    let rows = (0..10)
        .map(|i| SubtitleRow {
            score: format!("[{i}.00]").into(),
            name: format!("Row {i}").into(),
            ext: "srt".into(),
            languages: "zh,en".into(),
            extra_name: "".into(),
        })
        .collect::<Vec<_>>();
    results_model.set_vec(rows);

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
        "expected significantly more dark pixels with results rendered, got empty={empty_dark}, filled={filled_dark}"
    );
}
