#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel};

use chaos_seed::subtitle;

slint::include_modules!();

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

fn set_results(app: &AppWindow, items: &[subtitle::models::ThunderSubtitleItem]) {
    let ui_rows = items_to_ui(items);
    let model = VecModel::from(ui_rows);
    let model_rc: ModelRc<SubtitleRow> = ModelRc::from(Rc::new(model));
    app.set_results(model_rc);
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
}

enum UiMsg {
    Busy(bool),
    Status(String),
    Results(Vec<subtitle::models::ThunderSubtitleItem>),
}

fn install_panic_hook() {
    // Release builds hide the console window. Persist panic information so we can debug crashes.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::create_dir_all("logs");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = format!("logs/panic_{ts}.log");
        let msg = format!("{info}\n");
        let _ = std::fs::write(&path, msg);
        default_hook(info);
    }));
}

fn log_line(msg: &str) {
    let _ = std::fs::create_dir_all("logs");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = format!("[{ts}] {msg}\n");
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/app.log")
    {
        let _ = f.write_all(line.as_bytes());
    }
}

fn spawn_runtime_thread(
    mut task_rx: tokio::sync::mpsc::UnboundedReceiver<TaskMsg>,
    ui_tx: std::sync::mpsc::Sender<UiMsg>,
) {
    std::thread::spawn(move || {
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
                }
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

    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<UiMsg>();
    let (task_tx, task_rx) = tokio::sync::mpsc::unbounded_channel::<TaskMsg>();

    spawn_runtime_thread(task_rx, ui_tx.clone());

    setup_handlers(&app, state.clone(), task_tx, ui_tx.clone());
    start_ui_msg_pump(&app, state.clone(), ui_rx);

    app.run()
}

fn start_ui_msg_pump(
    app: &AppWindow,
    state: Arc<Mutex<AppState>>,
    ui_rx: std::sync::mpsc::Receiver<UiMsg>,
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
                                st.items = items;
                                set_results(&app, &st.items);
                                if st.items.is_empty() {
                                    app.set_status_text("未找到结果。".into());
                                } else {
                                    app.set_status_text(
                                        format!("找到 {} 条结果。", st.items.len()).into(),
                                    );
                                }
                                log_line(&format!(
                                    "ui=results_update count={}",
                                    st.items.len()
                                ));
                            }
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
) {
    // Theme: drive the Slint global theme from the UI toggle.
    // (Slint doesn't support binding-to-global in .slint syntax.)
    app.global::<AppTheme>().set_dark_mode(app.get_dark_mode());
    {
        let app_weak = app.as_weak();
        app.on_dark_mode_changed(move |dark| {
            if let Some(app) = app_weak.upgrade() {
                app.global::<AppTheme>().set_dark_mode(dark);
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
}
