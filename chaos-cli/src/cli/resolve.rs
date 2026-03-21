use crate::cli::OutputFormat;
use anyhow::{Context, Result};
use chaos_core::livestream::{LivestreamClient, ResolveOptions};
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    /// 直播间 URL 或房间号
    pub input: String,

    /// 输出格式
    #[arg(short, long, value_enum, default_value = "table")]
    pub format: OutputFormat,

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

    match args.format {
        OutputFormat::Json => print_json(&manifest)?,
        OutputFormat::Url => print_url(&manifest)?,
        OutputFormat::Plain => print_plain(&manifest)?,
        OutputFormat::Table => print_table(&manifest)?,
    }

    Ok(())
}

fn print_json(manifest: &chaos_core::livestream::model::LiveManifest) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest).context("Failed to serialize to JSON")?;
    println!("{}", json);
    Ok(())
}

fn print_url(manifest: &chaos_core::livestream::model::LiveManifest) -> Result<()> {
    if let Some(variant) = manifest.variants.first() {
        if let Some(url) = &variant.url {
            println!("{}", url);
        } else if !variant.backup_urls.is_empty() {
            println!("{}", variant.backup_urls[0]);
        } else {
            anyhow::bail!("No stream URL found");
        }
    } else {
        anyhow::bail!("No stream variants found");
    }
    Ok(())
}

fn print_plain(manifest: &chaos_core::livestream::model::LiveManifest) -> Result<()> {
    println!("平台: {}", manifest.site.as_str());
    println!("房间号: {}", manifest.room_id);
    println!("标题: {}", manifest.info.title);
    if let Some(name) = &manifest.info.name {
        println!("主播: {}", name);
    }
    if let Some(cover) = &manifest.info.cover {
        println!("封面: {}", cover);
    }
    println!("状态: {}", if manifest.info.is_living { "直播中" } else { "未开播" });
    println!();
    println!("可用画质:");

    for (i, variant) in manifest.variants.iter().enumerate() {
        let quality_str = if variant.quality > 0 {
            format!("{}P", variant.quality)
        } else {
            variant.label.clone()
        };

        let url_preview = variant
            .url
            .as_ref()
            .map(|u| &u[..u.len().min(50)])
            .unwrap_or("N/A");

        println!(
            "  [{}] {} (ID: {}) - {}{}",
            i + 1,
            quality_str,
            variant.id,
            url_preview,
            if variant.url.as_ref().map(|u| u.len()).unwrap_or(0) > 50 {
                "..."
            } else {
                ""
            }
        );

        if !variant.backup_urls.is_empty() {
            println!("      备用地址: {} 个", variant.backup_urls.len());
        }
    }

    Ok(())
}

fn print_table(manifest: &chaos_core::livestream::model::LiveManifest) -> Result<()> {
    // 打印直播间信息
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    直播间信息                              ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!(" 平台:  {}", manifest.site.as_str());
    println!(" 房间:  {}", manifest.room_id);
    println!(" 标题:  {}", manifest.info.title);
    if let Some(name) = &manifest.info.name {
        println!(" 主播:  {}", name);
    }
    println!(" 状态:  {}", if manifest.info.is_living { "🟢 直播中" } else { "🔴 未开播" });
    println!();

    // 打印画质选项表格
    println!("┌──────┬──────────┬─────────────────────┬──────────┬─────────────────────────────────────────┐");
    println!("│ {:<4} │ {:<8} │ {:<19} │ {:<8} │ {:<39} │", "序号", "画质", "ID", "码率", "主地址");
    println!("├──────┼──────────┼─────────────────────┼──────────┼─────────────────────────────────────────┤");
    
    for (i, v) in manifest.variants.iter().enumerate() {
        let quality = if v.quality > 0 {
            format!("{}P", v.quality)
        } else {
            v.label.clone()
        };
        let id = &v.id;
        let rate = v.rate.map(|r| format!("{}kbps", r)).unwrap_or_default();
        let url = v
            .url
            .as_ref()
            .map(|u| {
                if u.len() > 39 {
                    format!("{}...", &u[..36])
                } else {
                    u.clone()
                }
            })
            .unwrap_or_else(|| "N/A".to_string());
        
        println!("│ {:<4} │ {:<8} │ {:<19} │ {:<8} │ {:<39} │", 
            i + 1, 
            if quality.len() > 8 { &quality[..8] } else { &quality },
            if id.len() > 19 { &id[..19] } else { id },
            if rate.len() > 8 { &rate[..8] } else { &rate },
            url
        );
    }
    
    println!("└──────┴──────────┴─────────────────────┴──────────┴─────────────────────────────────────────┘");

    // 打印提示信息
    println!();
    println!("💡 提示: 使用 `chaos-cli play \"{}\"` 使用外部播放器播放", manifest.raw_input);

    Ok(())
}
