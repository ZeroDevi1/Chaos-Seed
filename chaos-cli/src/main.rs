mod cli;
mod player;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use std::process;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    info!("chaos-cli started");

    match cli.command {
        Commands::Resolve(args) => cli::resolve::run(args).await,
        Commands::Danmaku(args) => cli::danmaku::run(args).await,
        Commands::Play(args) => cli::play::run(args).await,
        Commands::Tui => tui::run().await,
    }
    .map_err(|e| {
        error!("Error: {}", e);
        e
    })?;

    Ok(())
}
