use chaos_core::danmaku::model::Site;
use chaos_core::livestream::{LivestreamClient, ResolveOptions};

fn usage() {
    eprintln!("livestream manifest dump (BiliLive/Douyu/Huya)");
    eprintln!("usage:");
    eprintln!("  cargo run -p chaos-core --example livestream_dump -- \\");
    eprintln!(
        "    --input <URL_OR_HINT> [--drop-inaccessible-high-qualities <0|1>] [--resolve <variant_id>] [--dump-json]"
    );
    eprintln!();
    eprintln!("examples:");
    eprintln!(
        "  cargo run -p chaos-core --example livestream_dump -- --input \"https://live.bilibili.com/<RID>\""
    );
    eprintln!(
        "  cargo run -p chaos-core --example livestream_dump -- --input \"douyu:<RID>\" --dump-json"
    );
}

fn has_flag(args: &[String], names: &[&str]) -> bool {
    args.iter().any(|a| names.iter().any(|n| *n == a))
}

fn get_flag_value(args: &[String], names: &[&str]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if names.iter().any(|n| *n == a) && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn site_to_str(site: Site) -> &'static str {
    match site {
        Site::BiliLive => "bilibili",
        Site::Douyu => "douyu",
        Site::Huya => "huya",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if has_flag(&args, &["-h", "--help"]) {
        usage();
        return Ok(());
    }

    let input = get_flag_value(&args, &["--input"]).unwrap_or_default();
    if input.trim().is_empty() {
        usage();
        std::process::exit(2);
    }

    let drop_inaccessible_high_qualities = get_flag_value(
        &args,
        &["--drop-inaccessible-high-qualities", "--drop-high"],
    )
    .and_then(|s| s.parse::<u8>().ok())
    .map(|v| v != 0)
    .unwrap_or(true);
    let resolve = get_flag_value(&args, &["--resolve"]);
    let dump_json = has_flag(&args, &["--dump-json", "--dump"]);

    let client = LivestreamClient::new()?;
    let manifest = client
        .decode_manifest(
            input.trim(),
            ResolveOptions {
                drop_inaccessible_high_qualities,
            },
        )
        .await?;

    if dump_json {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
    } else {
        eprintln!(
            "OK: site={} room_id={} living={} title={} variants={}",
            site_to_str(manifest.site),
            manifest.room_id,
            manifest.info.is_living,
            manifest.info.title,
            manifest.variants.len()
        );
        for (i, v) in manifest.variants.iter().enumerate() {
            let url_present = v
                .url
                .as_deref()
                .map(|u| !u.trim().is_empty())
                .unwrap_or(false);
            eprintln!(
                "  VAR[{}] id={} label={} quality={} url_present={}",
                i, v.id, v.label, v.quality, url_present
            );
        }
    }

    if let Some(variant_id) = resolve {
        let v = client
            .resolve_variant(manifest.site, &manifest.room_id, variant_id.trim())
            .await?;
        if dump_json {
            println!("{}", serde_json::to_string_pretty(&v)?);
        } else {
            eprintln!(
                "RESOLVED: id={} label={} quality={} url={} backups={}",
                v.id,
                v.label,
                v.quality,
                v.url.as_deref().unwrap_or("-"),
                v.backup_urls.len()
            );
        }
    }

    Ok(())
}
