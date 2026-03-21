use crate::player::Player;
use anyhow::{Context, Result};
use chaos_core::livestream::{LivestreamClient, ResolveOptions};
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    /// 直播间 URL 或房间号
    pub input: String,

    /// 指定播放器（auto/iina/potplayer/vlc）
    #[arg(short, long, default_value = "auto")]
    pub player: String,

    /// 选择画质（默认最高画质）
    #[arg(short, long)]
    pub quality: Option<String>,
}

pub async fn run(args: Args) -> Result<()> {
    let client = LivestreamClient::new().context("Failed to create livestream client")?;

    let options = ResolveOptions::default();

    let manifest = client
        .decode_manifest(&args.input, options)
        .await
        .context("Failed to decode manifest")?;

    // 获取最高画质（第一个通常是最高画质）
    let variant = manifest
        .variants
        .first()
        .context("No stream variants found")?;

    let stream_url = variant
        .url
        .as_ref()
        .or(variant.backup_urls.first())
        .context("No stream URL found")?;

    println!("🎥 正在启动播放器...");
    println!("   平台: {}", manifest.site.as_str());
    println!("   房间: {}", manifest.room_id);
    println!("   画质: {}", if variant.quality > 0 {
        format!("{}P", variant.quality)
    } else {
        variant.label.clone()
    });

    // 选择播放器
    let player = if args.player.eq_ignore_ascii_case("auto") {
        Player::detect()
    } else {
        match args.player.to_lowercase().as_str() {
            "iina" | "iina+" => Player::IINAPlus,
            "potplayer" | "pot" => Player::PotPlayer,
            "vlc" => Player::VLC,
            _ => Player::detect(),
        }
    };

    // 启动播放器
    player
        .play(stream_url, &manifest)
        .context("Failed to launch player")?;

    Ok(())
}
