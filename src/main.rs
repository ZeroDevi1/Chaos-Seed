#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel};

use chaos_seed::subtitle;

slint::include_modules!();

#[path = "danmaku/gui.rs"]
mod danmaku_gui;

#[derive(Default)]
struct AppState {
    items: Vec<subtitle::models::ThunderSubtitleItem>,
}

fn items_to_ui(items: &[subtitle::models::ThunderSubtitleItem]) -> Vec<SubtitleRow> {
    items
        .iter()
        .map(|item| {
            let langs = item
                .languages
                .iter()
                .filter(|x| !x.trim().is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(",");

            SubtitleRow {
                score: format!("[{:.2}]", item.score).into(),
                name: item.name.clone().into(),
                ext: (if item.ext.trim().is_empty() {
                    "srt".to_string()
                } else {
                    item.ext.clone()
                })
                .into(),
                languages: langs.into(),
                extra_name: item.extra_name.clone().into(),
            }
        })
        .collect()
}

#[cfg(windows)]
fn pick_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

#[cfg(not(windows))]
fn pick_folder() -> Option<PathBuf> {
    None
}

enum TaskMsg {
    Search {
        query: String,
        min_score: Option<f64>,
        lang: Option<String>,
        limit: usize,
    },
    DownloadOne {
        item: subtitle::models::ThunderSubtitleItem,
        out_dir: PathBuf,
    },
    DanmakuConnect {
        input: String,
    },
    DanmakuDisconnect,
    DanmakuLoadImage {
        row_id: i32,
        url: String,
    },
}

enum UiMsg {
    Busy(bool),
    Status(String),
    Results(Vec<subtitle::models::ThunderSubtitleItem>),
    DanmakuConnected {
        site: String,
        room_id: String,
    },
    DanmakuDisconnected,
    DanmakuEvent(chaos_seed::danmaku::model::DanmakuEvent),
    DanmakuImage {
        row_id: i32,
        w: u32,
        h: u32,
        pixels: Vec<slint::Rgba8Pixel>,
    },
    DanmakuError(String),
}

fn install_panic_hook() {
    // Intentionally disabled: we no longer persist logs/panic info to files.
}

fn log_line(msg: &str) {
    let _ = msg;
    // Intentionally disabled: we no longer write runtime logs to files.
}

fn spawn_runtime_thread(
    mut task_rx: tokio::sync::mpsc::UnboundedReceiver<TaskMsg>,
    ui_tx: std::sync::mpsc::Sender<UiMsg>,
) {
    std::thread::spawn(move || {
        #[derive(Clone)]
        struct CachedImage {
            w: u32,
            h: u32,
            pixels: std::sync::Arc<Vec<slint::Rgba8Pixel>>,
        }

        #[derive(Default)]
        struct ImageLoaderState {
            cache: std::collections::HashMap<String, CachedImage>,
            inflight: std::collections::HashMap<String, Vec<i32>>,
        }

        struct ActiveDanmaku {
            session: chaos_seed::danmaku::model::DanmakuSession,
            reader_task: tokio::task::JoinHandle<()>,
        }

        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = ui_tx.send(UiMsg::Status(format!("后台运行时初始化失败：{e}")));
                return;
            }
        };

        rt.block_on(async move {
            let image_state: std::sync::Arc<tokio::sync::Mutex<ImageLoaderState>> =
                std::sync::Arc::new(tokio::sync::Mutex::new(ImageLoaderState::default()));
            let image_sem = std::sync::Arc::new(tokio::sync::Semaphore::new(4));

            let http = reqwest::Client::builder()
                .user_agent("chaos-seed/0.1")
                .build()
                .expect("http client");

            let mut danmaku_active: Option<ActiveDanmaku> = None;

            while let Some(msg) = task_rx.recv().await {
                match msg {
                    TaskMsg::Search {
                        query,
                        min_score,
                        lang,
                        limit,
                    } => {
                        log_line(&format!(
                            "task=search start query={query:?} min_score={min_score:?} lang={lang:?} limit={limit}"
                        ));
                        let _ = ui_tx.send(UiMsg::Busy(true));
                        let _ = ui_tx.send(UiMsg::Status("正在搜索...".to_string()));

                        let lang_opt = lang.as_deref();
                        let res = subtitle::core::search_items(
                            &query,
                            limit,
                            min_score,
                            lang_opt,
                            Duration::from_secs(20),
                        )
                        .await;

                        match res {
                            Ok(items) => {
                                log_line(&format!("task=search ok items={}", items.len()));

                                // Dump the returned items so we can verify the API actually returned data.
                                // Limit lines to avoid huge logs when `limit` is large.
                                let max_dump = items.len().min(50);
                                for (i, item) in items.iter().take(max_dump).enumerate() {
                                    let ext = if item.ext.trim().is_empty() {
                                        "srt"
                                    } else {
                                        item.ext.as_str()
                                    };
                                    let langs = item
                                        .languages
                                        .iter()
                                        .filter(|x| !x.trim().is_empty())
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join(",");

                                    log_line(&format!(
                                        "task=search item[{i}] score={:.2} name={:?} ext={:?} languages={:?} extra_name={:?} url={:?}",
                                        item.score,
                                        item.name,
                                        ext,
                                        langs,
                                        item.extra_name,
                                        item.url
                                    ));
                                }
                                if items.len() > max_dump {
                                    log_line(&format!(
                                        "task=search items truncated: showing {} of {}",
                                        max_dump,
                                        items.len()
                                    ));
                                }

                                let _ = ui_tx.send(UiMsg::Results(items));
                            }
                            Err(e) => {
                                log_line(&format!("task=search err {e}"));
                                let _ = ui_tx.send(UiMsg::Status(format!("搜索失败：{e}")));
                            }
                        }
                        let _ = ui_tx.send(UiMsg::Busy(false));
                    }
                    TaskMsg::DownloadOne { item, out_dir } => {
                        log_line(&format!(
                            "task=download start name={:?} out_dir={}",
                            item.name,
                            out_dir.display()
                        ));
                        let _ = ui_tx.send(UiMsg::Busy(true));
                        let _ = ui_tx.send(UiMsg::Status(format!("开始下载：{}", item.name)));

                        let res = subtitle::core::download_item(
                            &item,
                            &out_dir,
                            Duration::from_secs(60),
                            2,
                            false,
                        )
                        .await;

                        match res {
                            Ok(path) => {
                                log_line(&format!(
                                    "task=download ok name={:?} path={}",
                                    item.name,
                                    path.display()
                                ));
                                let _ = ui_tx.send(UiMsg::Status(format!(
                                    "下载完成：{} -> {}",
                                    item.name,
                                    path.display()
                                )));
                            }
                            Err(e) => {
                                log_line(&format!(
                                    "task=download err name={:?} err={e}",
                                    item.name
                                ));
                                let _ = ui_tx.send(UiMsg::Status(format!("下载失败：{e}")));
                            }
                        }
                        let _ = ui_tx.send(UiMsg::Busy(false));
                    }

                    TaskMsg::DanmakuConnect { input } => {
                        // Best-effort: stop any previous session.
                        if let Some(active) = danmaku_active.take() {
                            active.reader_task.abort();
                            active.session.stop().await;
                        }

                        let input = input.trim().to_string();
                        if input.is_empty() {
                            let _ = ui_tx.send(UiMsg::DanmakuError("请输入直播间地址。".to_string()));
                            let _ = ui_tx.send(UiMsg::DanmakuDisconnected);
                            continue;
                        }

                        let client = match chaos_seed::danmaku::client::DanmakuClient::new() {
                            Ok(c) => c,
                            Err(e) => {
                                let _ = ui_tx.send(UiMsg::DanmakuError(format!("{e}")));
                                let _ = ui_tx.send(UiMsg::DanmakuDisconnected);
                                continue;
                            }
                        };

                        let target = match client.resolve(&input).await {
                            Ok(t) => t,
                            Err(e) => {
                                let _ = ui_tx.send(UiMsg::DanmakuError(format!("{e}")));
                                let _ = ui_tx.send(UiMsg::DanmakuDisconnected);
                                continue;
                            }
                        };

                        let _ = ui_tx.send(UiMsg::DanmakuConnected {
                            site: target.site.as_str().to_string(),
                            room_id: target.room_id.clone(),
                        });

                        let (session, mut rx) = match client
                            .connect_resolved(target, chaos_seed::danmaku::model::ConnectOptions::default())
                            .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = ui_tx.send(UiMsg::DanmakuError(format!("{e}")));
                                let _ = ui_tx.send(UiMsg::DanmakuDisconnected);
                                continue;
                            }
                        };

                        let ui_tx2 = ui_tx.clone();
                        let reader_task = tokio::spawn(async move {
                            while let Some(ev) = rx.recv().await {
                                let _ = ui_tx2.send(UiMsg::DanmakuEvent(ev));
                            }
                            let _ = ui_tx2.send(UiMsg::DanmakuDisconnected);
                        });

                        danmaku_active = Some(ActiveDanmaku { session, reader_task });
                    }

                    TaskMsg::DanmakuDisconnect => {
                        if let Some(active) = danmaku_active.take() {
                            active.reader_task.abort();
                            active.session.stop().await;
                        }
                        let _ = ui_tx.send(UiMsg::DanmakuDisconnected);
                    }

                    TaskMsg::DanmakuLoadImage { row_id, url } => {
                        let url = url.trim().to_string();
                        if url.is_empty() {
                            continue;
                        }

                        let ui_tx2 = ui_tx.clone();
                        let http2 = http.clone();
                        let image_state = image_state.clone();
                        let sem = image_sem.clone();

                        tokio::spawn(async move {
                            fn pick_referer(url: &str) -> &'static str {
                                match url::Url::parse(url).ok().and_then(|u| u.host_str().map(|s| s.to_ascii_lowercase())) {
                                    Some(h) if h.ends_with("hdslb.com") || h.ends_with("bilibili.com") => "https://live.bilibili.com/",
                                    Some(h) if h.ends_with("douyucdn.cn") || h.ends_with("douyu.com") => "https://www.douyu.com/",
                                    Some(h) if h.ends_with("huya.com") => "https://www.huya.com/",
                                    _ => "",
                                }
                            }

                            const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

                            // Fast path: cache hit / inflight merge.
                            {
                                let mut st = image_state.lock().await;
                                if let Some(c) = st.cache.get(&url) {
                                    let _ = ui_tx2.send(UiMsg::DanmakuImage {
                                        row_id,
                                        w: c.w,
                                        h: c.h,
                                        pixels: (*c.pixels).clone(),
                                    });
                                    return;
                                }
                                if let Some(waiters) = st.inflight.get_mut(&url) {
                                    waiters.push(row_id);
                                    return;
                                }
                                st.inflight.insert(url.clone(), vec![row_id]);
                            }

                            let _permit = sem.acquire().await.expect("semaphore permit");

                            let mut req = http2.get(&url).header(reqwest::header::USER_AGENT, BROWSER_UA);
                            let referer = pick_referer(&url);
                            if !referer.is_empty() {
                                req = req.header(reqwest::header::REFERER, referer);
                            }

                            let bytes = match req.send().await {
                                Ok(resp) => match resp.error_for_status() {
                                    Ok(resp) => match resp.bytes().await {
                                        Ok(b) => b,
                                        Err(_) => {
                                            let mut st = image_state.lock().await;
                                            let _ = st.inflight.remove(&url);
                                            return;
                                        }
                                    },
                                    Err(_) => {
                                        let mut st = image_state.lock().await;
                                        let _ = st.inflight.remove(&url);
                                        return;
                                    }
                                },
                                Err(_) => {
                                    let mut st = image_state.lock().await;
                                    let _ = st.inflight.remove(&url);
                                    return;
                                }
                            };

                            let dynimg = match image::load_from_memory(&bytes) {
                                Ok(i) => i,
                                Err(_) => {
                                    let mut st = image_state.lock().await;
                                    let _ = st.inflight.remove(&url);
                                    return;
                                }
                            };

                            // Downscale aggressively for chat thumbnails to keep memory reasonable.
                            let dynimg = dynimg.resize(96, 96, image::imageops::FilterType::Triangle);
                            let rgba = dynimg.to_rgba8();
                            let (w, h) = rgba.dimensions();
                            let raw = rgba.into_raw();

                            let mut pixels = Vec::with_capacity((w as usize) * (h as usize));
                            for px in raw.chunks_exact(4) {
                                pixels.push(slint::Rgba8Pixel {
                                    r: px[0],
                                    g: px[1],
                                    b: px[2],
                                    a: px[3],
                                });
                            }

                            let (waiters, cached) = {
                                let mut st = image_state.lock().await;
                                let waiters = st.inflight.remove(&url).unwrap_or_default();
                                let cached = CachedImage {
                                    w,
                                    h,
                                    pixels: std::sync::Arc::new(pixels),
                                };
                                st.cache.insert(url.clone(), cached.clone());
                                (waiters, cached)
                            };

                            for rid in waiters {
                                let _ = ui_tx2.send(UiMsg::DanmakuImage {
                                    row_id: rid,
                                    w: cached.w,
                                    h: cached.h,
                                    pixels: (*cached.pixels).clone(),
                                });
                            }
                        });
                    }
                }
            }

            // If the channel is closed, stop any active danmaku tasks.
            if let Some(active) = danmaku_active.take() {
                active.reader_task.abort();
                active.session.stop().await;
            }
        });
    });
}

fn parse_min_score(s: &str) -> Result<Option<f64>, &'static str> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<f64>()
        .map(Some)
        .map_err(|_| "min_score 不是合法数字。")
}

fn parse_limit(s: &str) -> usize {
    match s.trim().parse::<usize>() {
        Ok(v) if v > 0 => v.min(200),
        _ => 20,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    install_panic_hook();
    log_line("app=start");

    let app = AppWindow::new()?;
    app.set_version(env!("CARGO_PKG_VERSION").into());
    app.set_homepage("https://github.com/ZeroDevi1/Chaos-Seed".into());

    let state: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::default()));

    // Keep a stable model instance and only mutate its contents. This avoids subtle UI update
    // issues where replacing the model pointer doesn't refresh ListView delegates.
    let results_model: Rc<VecModel<SubtitleRow>> = Rc::new(VecModel::default());
    let results_model_rc: ModelRc<SubtitleRow> = ModelRc::from(results_model.clone());
    app.set_results(results_model_rc);

    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<UiMsg>();
    let (task_tx, task_rx) = tokio::sync::mpsc::unbounded_channel::<TaskMsg>();

    spawn_runtime_thread(task_rx, ui_tx.clone());

    // Danmaku models: keep stable instances and only mutate their contents.
    let danmaku_model: Rc<VecModel<DanmakuRow>> = Rc::new(VecModel::default());
    app.set_danmaku_messages(ModelRc::from(danmaku_model.clone()));
    let danmaku_empty_model: Rc<VecModel<DanmakuRow>> = Rc::new(VecModel::default());

    let danmaku_ctrl: Rc<RefCell<danmaku_gui::DanmakuUiController>> =
        Rc::new(RefCell::new(danmaku_gui::DanmakuUiController::new(
            app.as_weak(),
            task_tx.clone(),
            danmaku_model,
            danmaku_empty_model,
        )));

    setup_handlers(
        &app,
        state.clone(),
        task_tx,
        ui_tx.clone(),
        danmaku_ctrl.clone(),
    );
    start_ui_msg_pump(
        &app,
        state.clone(),
        ui_rx,
        results_model.clone(),
        danmaku_ctrl,
    );

    app.run()
}

fn start_ui_msg_pump(
    app: &AppWindow,
    state: Arc<Mutex<AppState>>,
    ui_rx: std::sync::mpsc::Receiver<UiMsg>,
    results_model: Rc<VecModel<SubtitleRow>>,
    danmaku_ctrl: Rc<RefCell<danmaku_gui::DanmakuUiController>>,
) {
    let app_weak = app.as_weak();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(30),
        move || {
            if let Some(app) = app_weak.upgrade() {
                while let Ok(msg) = ui_rx.try_recv() {
                    match msg {
                        UiMsg::Busy(b) => app.set_busy(b),
                        UiMsg::Status(s) => app.set_status_text(s.into()),
                        UiMsg::Results(items) => {
                            if let Ok(mut st) = state.lock() {
                                // Update raw items for download.
                                st.items = items;

                                // Update the UI model in-place.
                                let ui_rows = items_to_ui(&st.items);
                                results_model.set_vec(ui_rows);

                                if st.items.is_empty() {
                                    app.set_status_text("未找到结果。".into());
                                } else {
                                    app.set_status_text(
                                        format!("找到 {} 条结果。", st.items.len()).into(),
                                    );
                                }
                                log_line(&format!("ui=results_update count={}", st.items.len()));
                            }
                        }
                        UiMsg::DanmakuConnected { site, room_id } => {
                            app.set_danmaku_connected(true);
                            app.set_danmaku_status(format!("已连接：{site} / {room_id}").into());
                        }
                        UiMsg::DanmakuDisconnected => {
                            app.set_danmaku_connected(false);
                            app.set_danmaku_status("已断开。".into());
                            danmaku_gui::DanmakuUiController::return_to_main(&danmaku_ctrl);
                        }
                        UiMsg::DanmakuError(s) => {
                            app.set_danmaku_connected(false);
                            app.set_danmaku_status(format!("连接失败：{s}").into());
                        }
                        UiMsg::DanmakuEvent(ev) => {
                            if ev.text == "error" {
                                app.set_danmaku_connected(false);
                                app.set_danmaku_status("连接异常：已断开。".into());
                                continue;
                            }
                            danmaku_ctrl.borrow_mut().handle_event(ev);
                        }
                        UiMsg::DanmakuImage {
                            row_id,
                            w,
                            h,
                            pixels,
                        } => {
                            danmaku_ctrl.borrow_mut().apply_image(row_id, w, h, pixels);
                        }
                    }
                }
            }
        },
    );

    // Keep the timer alive for the entire app lifecycle.
    std::mem::forget(timer);
}

fn setup_handlers(
    app: &AppWindow,
    state: Arc<Mutex<AppState>>,
    task_tx: tokio::sync::mpsc::UnboundedSender<TaskMsg>,
    ui_tx: std::sync::mpsc::Sender<UiMsg>,
    danmaku_ctrl: Rc<RefCell<danmaku_gui::DanmakuUiController>>,
) {
    fn sync_std_widgets_palette(app: &AppWindow) {
        // std-widgets (Fluent) maintains its own palette and does not automatically follow our
        // custom AppTheme toggle. Keep them in sync, otherwise light UI + dark widgets becomes
        // unreadable.
        let scheme = if app.get_dark_mode() {
            slint::language::ColorScheme::Dark
        } else {
            slint::language::ColorScheme::Light
        };
        app.global::<Palette>().set_color_scheme(scheme);
    }

    // Theme: drive the Slint global theme from the UI toggle.
    // (Slint doesn't support binding-to-global in .slint syntax.)
    app.global::<AppTheme>().set_dark_mode(app.get_dark_mode());
    sync_std_widgets_palette(app);
    {
        let app_weak = app.as_weak();
        app.on_dark_mode_changed(move |dark| {
            if let Some(app) = app_weak.upgrade() {
                app.global::<AppTheme>().set_dark_mode(dark);
                sync_std_widgets_palette(&app);
            }
        });
    }

    // Search
    {
        let task_tx = task_tx.clone();
        let ui_tx = ui_tx.clone();
        app.on_search_clicked(move |query, min_score, lang, limit| {
            let query_trim = query.trim().to_string();
            if query_trim.is_empty() {
                let _ = ui_tx.send(UiMsg::Status("请输入关键词。".to_string()));
                return;
            }

            let min_score_parsed = match parse_min_score(min_score.as_str()) {
                Ok(v) => v,
                Err(msg) => {
                    let _ = ui_tx.send(UiMsg::Status(msg.to_string()));
                    return;
                }
            };

            let limit_parsed = parse_limit(limit.as_str());
            let lang_trim = lang.trim().to_string();
            let lang_opt = if lang_trim.is_empty() {
                None
            } else {
                Some(lang_trim)
            };

            log_line(&format!(
                "ui=search_clicked query={query_trim:?} min_score={min_score_parsed:?} lang={lang_opt:?} limit={limit_parsed}"
            ));
            let _ = task_tx.send(TaskMsg::Search {
                query: query_trim,
                min_score: min_score_parsed,
                lang: lang_opt,
                limit: limit_parsed,
            });
        });
    }

    // Download one item (pick folder each time)
    {
        let task_tx = task_tx.clone();
        let ui_tx = ui_tx.clone();
        app.on_download_one(move |idx| {
            let idx = idx as usize;
            let item = {
                let st = match state.lock() {
                    Ok(s) => s,
                    Err(_) => {
                        let _ = ui_tx.send(UiMsg::Status(
                            "内部状态错误：无法读取搜索结果。".to_string(),
                        ));
                        return;
                    }
                };
                st.items.get(idx).cloned()
            };

            let Some(item) = item else {
                let _ = ui_tx.send(UiMsg::Status("请选择有效的下载条目。".to_string()));
                return;
            };

            let picked = pick_folder();
            if picked.is_none() {
                #[cfg(windows)]
                let _ = ui_tx.send(UiMsg::Status("已取消下载。".to_string()));
                #[cfg(not(windows))]
                let _ = ui_tx.send(UiMsg::Status("目录选择仅在 Windows 构建可用。".to_string()));
                return;
            }

            let out_dir = picked.unwrap();
            let _ = task_tx.send(TaskMsg::DownloadOne { item, out_dir });
        });
    }

    // Open URL
    app.on_open_url(|url| {
        let _ = open::that(url.as_str());
    });

    // Danmaku (real connectors)
    {
        let task_tx = task_tx.clone();
        let ctrl = danmaku_ctrl.clone();
        let app_weak = app.as_weak();
        app.on_danmaku_connect(move |input| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let input_trim = input.trim().to_string();
            if input_trim.is_empty() {
                app.set_danmaku_status("请输入直播间地址。".into());
                return;
            }

            ctrl.borrow_mut().reset_for_new_session();
            danmaku_gui::DanmakuUiController::return_to_main(&ctrl);

            app.set_danmaku_connected(false);
            app.set_danmaku_status("连接中...".into());
            let _ = task_tx.send(TaskMsg::DanmakuConnect { input: input_trim });
        });
    }

    {
        let task_tx = task_tx.clone();
        let ctrl = danmaku_ctrl.clone();
        let app_weak = app.as_weak();
        app.on_danmaku_disconnect(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            danmaku_gui::DanmakuUiController::return_to_main(&ctrl);
            app.set_danmaku_connected(false);
            app.set_danmaku_status("正在断开...".into());
            let _ = task_tx.send(TaskMsg::DanmakuDisconnect);
        });
    }

    {
        let ctrl = danmaku_ctrl.clone();
        app.on_danmaku_open_chat_window(move || {
            danmaku_gui::DanmakuUiController::open_chat_window(&ctrl);
        });
    }

    {
        let ctrl = danmaku_ctrl.clone();
        app.on_danmaku_open_overlay_window(move || {
            danmaku_gui::DanmakuUiController::open_overlay_window(&ctrl);
        });
    }
}

#[cfg(windows)]
fn hwnd_from_slint_window(window: &slint::Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    // `slint::Window` does not implement `HasWindowHandle`, but `slint::WindowHandle` does.
    // Note: the raw handle is only available after the window has been shown by the backend.
    let handle = window.window_handle();
    let raw = handle.window_handle().ok()?.as_raw();
    match raw {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as windows_sys::Win32::Foundation::HWND),
        _ => None,
    }
}

fn begin_native_move(_window: &slint::Window) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{POINT, POINTS};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetCursorPos, HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN,
        };
        if let Some(hwnd) = hwnd_from_slint_window(_window) {
            unsafe {
                // Let Windows handle the drag; this is much smoother than manual set_position().
                //
                // Use `PostMessageW` (not `SendMessageW`) to avoid blocking the Slint event loop
                // during the modal move/resize loop. Blocking here can leave the backend in a
                // stale pointer/hit-test state until the next resize.
                let mut pos: POINT = std::mem::zeroed();
                let _ = GetCursorPos(&mut pos);
                let pts = POINTS {
                    x: pos.x as i16,
                    y: pos.y as i16,
                };
                let lparam = ((pts.y as u16 as u32) << 16) | (pts.x as u16 as u32);

                ReleaseCapture();
                let _ = PostMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, lparam as isize);
            }
        }
    }
}
