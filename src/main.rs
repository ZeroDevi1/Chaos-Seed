#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel};
use tokio::sync::Mutex;

mod subtitle;

slint::include_modules!();

#[derive(Debug, Clone)]
struct RowState {
    selected: bool,
    item: subtitle::models::ThunderSubtitleItem,
}

#[derive(Default)]
struct AppState {
    rows: Vec<RowState>,
}

fn rows_to_ui(rows: &[RowState]) -> Vec<SubtitleRow> {
    rows.iter()
        .map(|r| {
            let item = &r.item;
            let langs = item
                .languages
                .iter()
                .filter(|x| !x.trim().is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(",");

            SubtitleRow {
                selected: r.selected,
                score: format!("[{:.2}]", item.score).into(),
                name: item.name.clone().into(),
                ext: (if item.ext.trim().is_empty() { "srt".to_string() } else { item.ext.clone() }).into(),
                languages: langs.into(),
                extra_name: item.extra_name.clone().into(),
            }
        })
        .collect()
}

fn set_results(app: &AppWindow, rows: &[RowState]) {
    let ui_rows = rows_to_ui(rows);
    let model = VecModel::from(ui_rows);
    let model_rc: ModelRc<SubtitleRow> = ModelRc::from(Rc::new(model));
    app.set_results(model_rc);
}

#[cfg(windows)]
fn pick_folder() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

#[cfg(not(windows))]
fn pick_folder() -> Option<std::path::PathBuf> {
    None
}

#[tokio::main]
async fn main() {
    let app = AppWindow::new().expect("failed to create AppWindow");
    app.set_version(env!("CARGO_PKG_VERSION").into());
    app.set_homepage("https://github.com/ZeroDevi1/Chaos-Seed".into());

    let state: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::default()));

    setup_handlers(&app, state.clone());

    app.run().expect("slint app failed");
}

fn setup_handlers(app: &AppWindow, state: Arc<Mutex<AppState>>) {
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
    let app_weak_search = app.as_weak();
    let state_search = state.clone();
    app.on_search_clicked(move |query, min_score, lang, limit| {
        let query = query.to_string();
        let min_score = min_score.to_string();
        let lang = lang.to_string();
        let limit = limit.to_string();

        let app_weak = app_weak_search.clone();
        let state = state_search.clone();

        tokio::spawn(async move {
            let query_trim = query.trim().to_string();
            if query_trim.is_empty() {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_status_text("请输入关键词。".into());
                    }
                });
                return;
            }

            let min_score_parsed = {
                let s = min_score.trim();
                if s.is_empty() {
                    None
                } else {
                    match s.parse::<f64>() {
                        Ok(v) => Some(v),
                        Err(_) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_status_text("min_score 不是合法数字。".into());
                                }
                            });
                            return;
                        }
                    }
                }
            };

            let limit_parsed = match limit.trim().parse::<usize>() {
                Ok(v) if v > 0 => v.min(200),
                _ => 20,
            };

            let lang_trim = lang.trim().to_string();
            let lang_opt = if lang_trim.is_empty() { None } else { Some(lang_trim.as_str()) };

            let _ = slint::invoke_from_event_loop({
                let app_weak = app_weak.clone();
                move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_busy(true);
                        app.set_status_text("正在搜索...".into());
                    }
                }
            });

            let result =
                subtitle::core::search_items(&query_trim, limit_parsed, min_score_parsed, lang_opt, Duration::from_secs(20))
                    .await;

            match result {
                Ok(items) => {
                    let mut st = state.lock().await;
                    st.rows = items
                        .into_iter()
                        .map(|it| RowState { selected: false, item: it })
                        .collect();
                    let rows = st.rows.clone();
                    drop(st);

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            set_results(&app, &rows);
                            app.set_status_text(format!("找到 {} 条结果。", rows.len()).into());
                            app.set_busy(false);
                        }
                    });
                }
                Err(e) => {
                    let msg = format!("搜索失败：{e}");
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_status_text(msg.into());
                            app.set_busy(false);
                        }
                    });
                }
            }
        });
    });

    // Toggle selection
    let app_weak = app.as_weak();
    let state_toggle = state.clone();
    app.on_result_toggled(move |idx, checked| {
        let idx = idx as usize;
        let app_weak = app_weak.clone();
        let state_toggle = state_toggle.clone();
        tokio::spawn(async move {
            let mut st = state_toggle.lock().await;
            if idx < st.rows.len() {
                st.rows[idx].selected = checked;
            }
            let rows = st.rows.clone();
            drop(st);

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak.upgrade() {
                    set_results(&app, &rows);
                }
            });
        });
    });

    // Pick folder (optional explicit button)
    // Download
    let app_weak = app.as_weak();
    let state_dl = state.clone();
    app.on_download_clicked(move || {
        let app_weak2 = app_weak.clone();
        let state_dl2 = state_dl.clone();

        let picked = pick_folder();

        if picked.is_none() {
            if let Some(app) = app_weak2.upgrade() {
                #[cfg(windows)]
                app.set_status_text("已取消下载。".into());
                #[cfg(not(windows))]
                app.set_status_text("目录选择仅在 Windows 构建可用。".into());
            }
            return;
        }

        let out_dir = picked.unwrap();
        if let Some(app) = app_weak2.upgrade() {
            app.set_out_dir(out_dir.display().to_string().into());
            app.set_busy(true);
            app.set_status_text("开始下载...".into());
        }

        tokio::spawn(async move {
            let selected: Vec<subtitle::models::ThunderSubtitleItem> = {
                let st = state_dl2.lock().await;
                st.rows
                    .iter()
                    .filter(|r| r.selected)
                    .map(|r| r.item.clone())
                    .collect()
            };

            if selected.is_empty() {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak2.upgrade() {
                        app.set_status_text("请先勾选要下载的字幕。".into());
                        app.set_busy(false);
                    }
                });
                return;
            }

            let total = selected.len();
            for (i, item) in selected.iter().enumerate() {
                let name = item.name.clone();
                let _ = slint::invoke_from_event_loop({
                    let app_weak = app_weak2.clone();
                    move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_status_text(format!("正在下载 {}/{}：{}", i + 1, total, name).into());
                        }
                    }
                });

                let res =
                    subtitle::core::download_item(item, &out_dir, Duration::from_secs(60), 2, false).await;

                if let Err(e) = res {
                    let msg = format!("下载失败：{e}");
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak2.upgrade() {
                            app.set_status_text(msg.into());
                            app.set_busy(false);
                        }
                    });
                    return;
                }
            }

            let msg = format!("下载完成：共 {} 个文件 -> {}", total, out_dir.display());
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak2.upgrade() {
                    app.set_status_text(msg.into());
                    app.set_busy(false);
                }
            });
        });
    });

    // Open URL
    app.on_open_url(|url| {
        let _ = open::that(url.as_str());
    });
}
