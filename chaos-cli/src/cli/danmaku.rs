use crate::cli::OutputFormat;
use anyhow::{Context, Result};
use chaos_core::danmaku::{client::DanmakuClient, model::ConnectOptions};
use clap::Parser;
use std::io::{self, Write};
use tokio::signal;
use tracing::{error, info};

#[derive(Parser)]
pub struct Args {
    /// 直播间 URL 或房间号
    pub input: String,

    /// 输出格式
    #[arg(short, long, value_enum, default_value = "plain")]
    pub format: OutputFormat,

    /// 输出到文件（JSONL 格式）
    #[arg(short, long)]
    pub output: Option<std::path::PathBuf>,

    /// 使用正则表达式过滤弹幕
    #[arg(long = "filter")]
    pub filter_regex: Option<String>,
}

pub async fn run(args: Args) -> Result<()> {
    let client = DanmakuClient::new().context("Failed to create danmaku client")?;

    let target = client
        .resolve(&args.input)
        .await
        .context("Failed to resolve danmaku target")?;

    info!(
        "Connecting to danmaku server: site={} room_id={}",
        target.site.as_str(),
        target.room_id
    );

    let (_session, mut rx) = client
        .connect_resolved(target.clone(), ConnectOptions::default())
        .await
        .context("Failed to connect to danmaku server")?;

    println!("✅ 已连接到 {} 直播间: {}", target.site.as_str(), target.room_id);
    println!("按 Ctrl+C 退出...");
    println!();

    let filter = args
        .filter_regex
        .as_ref()
        .map(|r| regex::Regex::new(r).ok())
        .flatten();

    let mut file: Option<std::fs::File> = None;
    if let Some(path) = &args.output {
        file = Some(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .context("Failed to open output file")?,
        );
    }

    let mut stdout = io::stdout();

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                let user = if event.user.is_empty() { "未知用户" } else { &event.user };
                let text = &event.text;

                // 应用过滤
                if let Some(ref regex) = filter {
                    if !regex.is_match(text) {
                        continue;
                    }
                }

                match args.format {
                    OutputFormat::Plain | OutputFormat::Table => {
                        let output = format!("[{}] {}: {}",
                            event.site.as_str(),
                            user,
                            text
                        );
                        writeln!(stdout, "{}", output)?;
                        stdout.flush()?;
                    }
                    OutputFormat::Json => {
                        let json = serde_json::to_string(&event)
                            .context("Failed to serialize event")?;
                        writeln!(stdout, "{}", json)?;
                        stdout.flush()?;

                        // 写入文件（JSONL 格式）
                        if let Some(ref mut f) = file {
                            writeln!(f, "{}", json)?;
                        }
                    }
                    OutputFormat::Url => {
                        // 不适用于弹幕
                    }
                }
            }

            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down...");
                println!("\n👋 再见!");
                break;
            }
        }
    }

    Ok(())
}
