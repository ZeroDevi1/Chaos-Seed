use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, Tabs, Wrap,
    },
    Frame, Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{error, info};

mod resolver_tab;
use resolver_tab::ResolverTab;

mod danmaku_tab;
use danmaku_tab::DanmakuTab;

/// TUI 应用状态
pub struct App {
    /// 当前选中的标签页
    current_tab: usize,
    /// 标签页标题
    tabs: Vec<&'static str>,
    /// 解析器标签页
    resolver: ResolverTab,
    /// 弹幕标签页
    danmaku: DanmakuTab,
    /// 是否退出
    should_quit: bool,
    /// 状态消息
    status_message: Option<String>,
    /// 状态消息过期时间
    status_expires: Option<Instant>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            current_tab: 0,
            tabs: vec!["解析直播", "弹幕监控", "帮助"],
            resolver: ResolverTab::new(),
            danmaku: DanmakuTab::new(),
            should_quit: false,
            status_message: None,
            status_expires: None,
        }
    }
}

impl App {
    fn set_status(&mut self, msg: impl Into<String>, duration: Duration) {
        self.status_message = Some(msg.into());
        self.status_expires = Some(Instant::now() + duration);
    }

    fn clear_expired_status(&mut self) {
        if let Some(expires) = self.status_expires {
            if Instant::now() > expires {
                self.status_message = None;
                self.status_expires = None;
            }
        }
    }
}

pub async fn run() -> Result<()> {
    info!("Starting TUI...");

    // 设置终端
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // 创建应用状态
    let mut app = App::default();

    // 运行主循环
    let res = run_app(&mut terminal, &mut app).await;

    // 恢复终端
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    res
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // 绘制界面
        terminal.draw(|f| ui(f, app))?;

        // 处理输入和定时器
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if app.current_tab == 2 {
                                // 在帮助页面按 q 退出
                                app.should_quit = true;
                            } else {
                                // 其他页面切换到帮助页面
                                app.current_tab = 2;
                            }
                        }
                        KeyCode::Char('1') => app.current_tab = 0,
                        KeyCode::Char('2') => app.current_tab = 1,
                        KeyCode::Char('3') | KeyCode::Char('?') => app.current_tab = 2,
                        KeyCode::Tab => {
                            app.current_tab = (app.current_tab + 1) % app.tabs.len();
                        }
                        KeyCode::BackTab => {
                            app.current_tab =
                                (app.current_tab + app.tabs.len() - 1) % app.tabs.len();
                        }
                        _ => {
                            // 将按键传递给当前标签页
                            match app.current_tab {
                                0 => app.resolver.handle_key(key.code).await,
                                1 => app.danmaku.handle_key(key.code).await,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // 定时更新
        if last_tick.elapsed() >= tick_rate {
            app.clear_expired_status();
            app.danmaku.update().await;
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    // 标题栏
    render_header(f, chunks[0], app);

    // 主体内容
    match app.current_tab {
        0 => app.resolver.render(f, chunks[1]),
        1 => app.danmaku.render(f, chunks[1]),
        _ => render_help(f, chunks[1]),
    }

    // 状态栏
    render_status_bar(f, chunks[2], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let title = Paragraph::new("Chaos-Seed CLI")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));

    f.render_widget(title, area);

    // 标签页
    let tab_titles: Vec<Line> = app.tabs.iter().cloned().map(|t| Line::from(t)).collect();
    let tabs = Tabs::new(tab_titles)
        .select(app.current_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider(Span::raw(" | "));

    let tab_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: 1,
    };

    f.render_widget(tabs, tab_area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let help_text = Text::from(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("键盘快捷键:", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from("  1          - 切换到 解析直播 标签页"),
        Line::from("  2          - 切换到 弹幕监控 标签页"),
        Line::from("  3 / ?      - 切换到 帮助 标签页"),
        Line::from("  Tab        - 下一个标签页"),
        Line::from("  Shift+Tab  - 上一个标签页"),
        Line::from("  q / Esc    - 退出程序"),
        Line::from(""),
        Line::from(vec![
            Span::styled("解析直播 页面:", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  Enter      - 开始解析输入的 URL"),
        Line::from("  p          - 使用外部播放器播放"),
        Line::from("  ↑/↓        - 选择画质"),
        Line::from("  c          - 复制选中画质的 URL"),
        Line::from(""),
        Line::from(vec![
            Span::styled("弹幕监控 页面:", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  Enter      - 连接/断开弹幕"),
        Line::from("  Space      - 暂停/继续滚动"),
        Line::from("  f          - 打开过滤对话框"),
        Line::from("  s          - 保存弹幕到文件"),
        Line::from(""),
        Line::from(vec![
            Span::styled("命令行使用:", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  chaos-cli resolve <url>     - 解析直播源"),
        Line::from("  chaos-cli danmaku <url>     - 显示弹幕"),
        Line::from("  chaos-cli play <url>        - 使用外部播放器播放"),
    ]);

    let help = Paragraph::new(help_text).wrap(Wrap { trim: true }).block(
        Block::default()
            .title("帮助")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(help, area);
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let status = if let Some(ref msg) = app.status_message {
        Line::from(vec![
            Span::styled("  状态: ", Style::default().fg(Color::Gray)),
            Span::styled(msg, Style::default().fg(Color::Yellow)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  按 ", Style::default().fg(Color::Gray)),
            Span::styled("?", Style::default().fg(Color::Yellow)),
            Span::styled(" 查看帮助, ", Style::default().fg(Color::Gray)),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::styled(" 退出", Style::default().fg(Color::Gray)),
        ])
    };

    let status_bar = Paragraph::new(status);
    f.render_widget(status_bar, area);
}
