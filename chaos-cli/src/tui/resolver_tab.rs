use chaos_core::livestream::{LivestreamClient, ResolveOptions};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, Wrap,
    },
    Frame,
};
use std::io::Write;

pub struct ResolverTab {
    /// 输入框内容
    input: String,
    /// 是否正在输入
    is_inputting: bool,
    /// 解析结果
    manifest: Option<chaos_core::livestream::model::LiveManifest>,
    /// 错误信息
    error: Option<String>,
    /// 选中的画质索引
    selected_variant: usize,
    /// 是否正在解析中
    is_loading: bool,
}

impl ResolverTab {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            is_inputting: true,
            manifest: None,
            error: None,
            selected_variant: 0,
            is_loading: false,
        }
    }

    pub async fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) if self.is_inputting => {
                self.input.push(c);
            }
            KeyCode::Backspace if self.is_inputting => {
                self.input.pop();
            }
            KeyCode::Enter => {
                if self.is_inputting && !self.input.is_empty() {
                    self.resolve().await;
                }
            }
            KeyCode::Up => {
                if self.selected_variant > 0 {
                    self.selected_variant -= 1;
                }
            }
            KeyCode::Down => {
                if let Some(ref manifest) = self.manifest {
                    if self.selected_variant < manifest.variants.len().saturating_sub(1) {
                        self.selected_variant += 1;
                    }
                }
            }
            KeyCode::Char('p') => {
                if self.manifest.is_some() {
                    self.play_with_player().await;
                }
            }
            KeyCode::Char('c') => {
                if let Some(ref manifest) = self.manifest {
                    if let Some(variant) = manifest.variants.get(self.selected_variant) {
                        if let Some(url) = &variant.url {
                            // 复制到剪贴板
                            #[cfg(target_os = "macos")]
                            {
                                let _ = std::process::Command::new("pbcopy")
                                    .arg(url)
                                    .spawn();
                            }
                            #[cfg(target_os = "linux")]
                            {
                                let _ = std::process::Command::new("xclip")
                                    .arg("-selection")
                                    .arg("clipboard")
                                    .stdin(std::process::Stdio::piped())
                                    .spawn()
                                    .and_then(|mut c| {
                                        c.stdin.take().map(|mut s| s.write_all(url.as_bytes()));
                                        Ok(())
                                    });
                            }
                            #[cfg(target_os = "windows")]
                            {
                                let _ = std::process::Command::new("clip")
                                    .stdin(std::process::Stdio::piped())
                                    .spawn()
                                    .and_then(|mut c| {
                                        c.stdin.take().map(|mut s| s.write_all(url.as_bytes()));
                                        Ok(())
                                    });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    async fn resolve(&mut self) {
        self.is_loading = true;
        self.error = None;
        self.manifest = None;
        self.selected_variant = 0;

        let client = match LivestreamClient::new() {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(format!("Failed to create client: {}", e));
                self.is_loading = false;
                return;
            }
        };

        let options = ResolveOptions::default();

        match client.decode_manifest(&self.input, options).await {
            Ok(manifest) => {
                self.manifest = Some(manifest);
            }
            Err(e) => {
                self.error = Some(format!("解析失败: {}", e));
            }
        }

        self.is_loading = false;
    }

    async fn play_with_player(&self) {
        if let Some(ref manifest) = self.manifest {
            if let Some(variant) = manifest.variants.get(self.selected_variant) {
                if let Some(url) = &variant.url {
                    let _ = crate::player::Player::detect().play(url, manifest);
                }
            }
        }
    }

    pub fn render(&self,
        f: &mut Frame,
        area: Rect,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // 输入框
        let input_block = Block::default()
            .title("输入直播间 URL")
            .borders(Borders::ALL)
            .border_style(if self.is_inputting {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            });

        let input = Paragraph::new(self.input.clone()).block(input_block);
        f.render_widget(input, chunks[0]);

        // 结果显示区域
        let result_area = chunks[1];

        if self.is_loading {
            let loading = Paragraph::new("⏳ 正在解析...")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(loading, result_area);
        } else if let Some(ref error) = self.error {
            let error_widget = Paragraph::new(format!("❌ {}", error))
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(error_widget, result_area);
        } else if let Some(ref manifest) = self.manifest {
            self.render_manifest(f, result_area, manifest);
        } else {
            let hint = Paragraph::new("输入直播间 URL 并按 Enter 开始解析")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(hint, result_area);
        }
    }

    fn render_manifest(
        &self,
        f: &mut Frame,
        area: Rect,
        manifest: &chaos_core::livestream::model::LiveManifest,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Min(0)])
            .split(area);

        // 直播间信息
        let info_text = Text::from(vec![
            Line::from(vec![
                Span::styled("平台: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(manifest.site.as_str()),
            ]),
            Line::from(vec![
                Span::styled("房间: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&manifest.room_id),
            ]),
            Line::from(vec![
                Span::styled("标题: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&manifest.info.title),
            ]),
            Line::from(vec![
                Span::styled("主播: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(manifest.info.name.as_deref().unwrap_or("未知")),
            ]),
            Line::from(vec![
                Span::styled("状态: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    if manifest.info.is_living {
                        "🟢 直播中"
                    } else {
                        "🔴 未开播"
                    },
                    if manifest.info.is_living {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Red)
                    },
                ),
            ]),
        ]);

        let info_block = Block::default()
            .title("直播间信息")
            .borders(Borders::ALL);
        let info = Paragraph::new(info_text).block(info_block);
        f.render_widget(info, chunks[0]);

        // 画质列表
        let header = Row::new(vec!["选择", "画质", "ID", "码率"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1);

        let rows: Vec<Row> = manifest
            .variants
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let cells = vec![
                    Cell::from(if i == self.selected_variant { "▶" } else { "" }),
                    Cell::from(if v.quality > 0 {
                        format!("{}P", v.quality)
                    } else {
                        v.label.clone()
                    }),
                    Cell::from(v.id.clone()),
                    Cell::from(v.rate.map(|r| format!("{}kbps", r)).unwrap_or_default()),
                ];

                let style = if i == self.selected_variant {
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(cells).style(style)
            })
            .collect();

        let table = Table::new(rows, vec![
            Constraint::Length(4),
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Length(10),
        ])
        .header(header)
        .block(Block::default().title("可用画质 (↑/↓ 选择, p 播放, c 复制 URL)").borders(Borders::ALL));

        f.render_widget(table, chunks[1]);
    }
}
