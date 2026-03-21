use chaos_core::danmaku::{client::DanmakuClient, model::ConnectOptions};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;

const MAX_MESSAGES: usize = 1000;

pub struct DanmakuTab {
    /// 输入框内容
    input: String,
    /// 是否正在输入
    is_inputting: bool,
    /// 是否已连接
    is_connected: bool,
    /// 弹幕消息队列
    messages: VecDeque<DanmakuMessage>,
    /// 滚动位置
    scroll: usize,
    /// 是否暂停滚动
    is_paused: bool,
    /// 弹幕接收通道
    rx: Option<mpsc::UnboundedReceiver<chaos_core::danmaku::model::DanmakuEvent>>,
    /// 会话（用于断开连接）
    _session: Option<chaos_core::danmaku::model::DanmakuSession>,
    /// 错误信息
    error: Option<String>,
    /// 过滤正则
    filter_regex: Option<regex::Regex>,
    /// 是否显示过滤对话框
    show_filter_dialog: bool,
    /// 过滤输入
    filter_input: String,
}

struct DanmakuMessage {
    timestamp: chrono::DateTime<chrono::Local>,
    site: String,
    user: String,
    text: String,
    color: Color,
}

impl DanmakuTab {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            is_inputting: true,
            is_connected: false,
            messages: VecDeque::with_capacity(MAX_MESSAGES),
            scroll: 0,
            is_paused: false,
            rx: None,
            _session: None,
            error: None,
            filter_regex: None,
            show_filter_dialog: false,
            filter_input: String::new(),
        }
    }

    pub async fn handle_key(&mut self, key: KeyCode) {
        if self.show_filter_dialog {
            match key {
                KeyCode::Char(c) => self.filter_input.push(c),
                KeyCode::Backspace => {
                    self.filter_input.pop();
                }
                KeyCode::Enter => {
                    if self.filter_input.is_empty() {
                        self.filter_regex = None;
                    } else if let Ok(regex) = regex::Regex::new(&self.filter_input) {
                        self.filter_regex = Some(regex);
                    }
                    self.show_filter_dialog = false;
                }
                KeyCode::Esc => {
                    self.show_filter_dialog = false;
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char(c) if self.is_inputting && !self.is_connected => {
                self.input.push(c);
            }
            KeyCode::Backspace if self.is_inputting && !self.is_connected => {
                self.input.pop();
            }
            KeyCode::Enter => {
                if self.is_connected {
                    self.disconnect().await;
                } else if !self.input.is_empty() {
                    self.connect().await;
                }
            }
            KeyCode::Char(' ') => {
                self.is_paused = !self.is_paused;
            }
            KeyCode::Char('f') => {
                self.show_filter_dialog = true;
            }
            KeyCode::Char('s') => {
                self.save_to_file().await;
            }
            KeyCode::Up => {
                if self.scroll > 0 {
                    self.scroll -= 1;
                }
            }
            KeyCode::Down => {
                let max_scroll = self.messages.len().saturating_sub(1);
                if self.scroll < max_scroll {
                    self.scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let max_scroll = self.messages.len().saturating_sub(1);
                self.scroll = (self.scroll + 10).min(max_scroll);
            }
            KeyCode::Home => {
                self.scroll = 0;
            }
            KeyCode::End => {
                self.scroll = self.messages.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    async fn connect(&mut self) {
        self.error = None;

        let client = match DanmakuClient::new() {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(format!("Failed to create client: {}", e));
                return;
            }
        };

        match client.connect(&self.input, ConnectOptions::default()).await {
            Ok((session, rx)) => {
                self.is_connected = true;
                self._session = Some(session);
                self.rx = Some(rx);
                self.messages.clear();
                self.scroll = 0;
            }
            Err(e) => {
                self.error = Some(format!("连接失败: {}", e));
            }
        }
    }

    async fn disconnect(&mut self) {
        self.is_connected = false;
        self._session = None;
        self.rx = None;
    }

    async fn save_to_file(&self) {
        use std::io::Write;

        let filename = format!(
            "danmaku_{}.jsonl",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );

        if let Ok(mut file) = std::fs::File::create(&filename) {
            for msg in &self.messages {
                let json = serde_json::json!({
                    "timestamp": msg.timestamp.to_rfc3339(),
                    "site": msg.site,
                    "user": msg.user,
                    "text": msg.text,
                });
                let _ = writeln!(file, "{}", json);
            }
        }
    }

    pub async fn update(&mut self) {
        if let Some(ref mut rx) = self.rx {
            // 尝试接收所有待处理的消息
            loop {
                match rx.try_recv() {
                    Ok(event) => {
                        // 应用过滤
                        if let Some(ref regex) = self.filter_regex {
                            if !regex.is_match(&event.text) {
                                continue;
                            }
                        }

                        let msg = DanmakuMessage {
                            timestamp: chrono::Local::now(),
                            site: event.site.as_str().to_string(),
                            user: if event.user.is_empty() { "未知用户".to_string() } else { event.user.clone() },
                            text: event.text.clone(),
                            color: Self::get_random_color(),
                        };

                        self.messages.push_back(msg);

                        // 限制消息数量
                        if self.messages.len() > MAX_MESSAGES {
                            self.messages.pop_front();
                        }

                        // 自动滚动到底部（如果没有暂停）
                        if !self.is_paused {
                            self.scroll = self.messages.len().saturating_sub(1);
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    fn get_random_color() -> Color {
        use rand::Rng;
        let colors = [
            Color::White,
            Color::Yellow,
            Color::Cyan,
            Color::Green,
            Color::Magenta,
            Color::Blue,
        ];
        let mut rng = rand::thread_rng();
        colors[rng.gen_range(0..colors.len())]
    }

    pub fn render(&self,
        f: &mut Frame,
        area: Rect,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        // 输入框
        let input_block = Block::default()
            .title(if self.is_connected {
                "已连接（按 Enter 断开）"
            } else {
                "输入直播间 URL"
            })
            .borders(Borders::ALL)
            .border_style(if self.is_connected {
                Style::default().fg(Color::Green)
            } else if self.is_inputting {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            });

        let input = Paragraph::new(self.input.clone()).block(input_block);
        f.render_widget(input, chunks[0]);

        // 弹幕列表
        let messages: Vec<Line> = self
            .messages
            .iter()
            .skip(self.scroll)
            .take(area.height as usize)
            .map(|msg| {
                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", msg.timestamp.format("%H:%M:%S")),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("[{}] ", msg.site), Style::default().fg(Color::Blue)),
                    Span::styled(format!("{}: ", msg.user), Style::default().fg(msg.color)),
                    Span::raw(&msg.text),
                ])
            })
            .collect();

        let status_text = if self.is_paused {
            "⏸ 已暂停 (Space 继续)"
        } else {
            "▶ 实时更新"
        };

        let danmaku_block = Block::default()
            .title(format!("弹幕消息 ({}) - {}", self.messages.len(), status_text))
            .borders(Borders::ALL);

        let danmaku = Paragraph::new(Text::from(messages))
            .block(danmaku_block)
            .wrap(Wrap { trim: true });

        f.render_widget(danmaku, chunks[1]);

        // 滚动条
        if self.messages.len() > 0 {
            let mut scrollbar_state = ScrollbarState::new(self.messages.len())
                .position(self.scroll)
                .viewport_content_length(area.height as usize);

            f.render_stateful_widget(
                Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓")),
                chunks[1],
                &mut scrollbar_state,
            );
        }

        // 底部提示
        let hint = if let Some(ref error) = self.error {
            Line::from(vec![Span::styled(
                format!("❌ {}", error),
                Style::default().fg(Color::Red),
            )])
        } else {
            Line::from(vec![
                Span::styled("Space", Style::default().fg(Color::Yellow)),
                Span::raw(" 暂停 "),
                Span::styled("f", Style::default().fg(Color::Yellow)),
                Span::raw(" 过滤 "),
                Span::styled("s", Style::default().fg(Color::Yellow)),
                Span::raw(" 保存 "),
                Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
                Span::raw(" 滚动"),
            ])
        };

        let hint_widget = Paragraph::new(hint);
        f.render_widget(hint_widget, chunks[2]);

        // 过滤对话框
        if self.show_filter_dialog {
            self.render_filter_dialog(f, area);
        }
    }

    fn render_filter_dialog(
        &self,
        f: &mut Frame,
        _area: Rect,
    ) {
        let popup_area = Rect {
            x: _area.width / 4,
            y: _area.height / 3,
            width: _area.width / 2,
            height: 5,
        };

        let block = Block::default()
            .title("过滤弹幕 (正则表达式)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let input = Paragraph::new(self.filter_input.clone()).block(block);

        f.render_widget(ratatui::widgets::Clear, popup_area);
        f.render_widget(input, popup_area);
    }
}
