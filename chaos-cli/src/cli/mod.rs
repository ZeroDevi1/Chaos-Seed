use clap::{Parser, Subcommand};

pub mod danmaku;
pub mod play;
pub mod resolve;

#[derive(Parser)]
#[command(
    name = "chaos-cli",
    version,
    about = "CLI and TUI for chaos-seed - 直播源解析与弹幕工具",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 解析直播源地址
    Resolve(resolve::Args),

    /// 实时显示弹幕
    Danmaku(danmaku::Args),

    /// 解析并使用外部播放器播放
    Play(play::Args),

    /// 启动 TUI 界面
    Tui,
}

/// 输出格式选项
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// 表格形式（默认）
    #[default]
    Table,
    /// JSON 格式
    Json,
    /// 仅 URL
    Url,
    /// 纯文本
    Plain,
}
