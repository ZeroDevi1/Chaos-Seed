use std::str::FromStr;

use chaos_core::lyrics;

fn usage() {
    eprintln!("lyrics search (BetterLyrics-style multi-provider)");
    eprintln!("usage:");
    eprintln!("  cargo run -p chaos-core --example lyrics_search -- \\");
    eprintln!("    --title <TITLE> [--artist <ARTIST>] [--album <ALBUM>] [--duration-ms <N>]");
    eprintln!("    [--limit <N>] [--services <csv>] [--timeout-ms <N>] [--strict] [--dump-json]");
    eprintln!();
    eprintln!("examples:");
    eprintln!(
        "  cargo run -p chaos-core --example lyrics_search -- --title \"Hello\" --artist \"Adele\""
    );
    eprintln!(
        "  cargo run -p chaos-core --example lyrics_search -- --title \"Hello\" --artist \"Adele\" --services qq,netease,lrclib --dump-json"
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if has_flag(&args, &["-h", "--help"]) {
        usage();
        return Ok(());
    }

    let title = get_flag_value(&args, &["--title"]).unwrap_or_default();
    if title.trim().is_empty() {
        usage();
        std::process::exit(2);
    }

    let artist = get_flag_value(&args, &["--artist"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let album = get_flag_value(&args, &["--album"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let duration_ms = get_flag_value(&args, &["--duration-ms", "--duration"])
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|v| *v > 0);
    let limit = get_flag_value(&args, &["--limit"])
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10)
        .clamp(1, 50);
    let services_csv =
        get_flag_value(&args, &["--services"]).unwrap_or_else(|| "qq,netease,lrclib".to_string());
    let timeout_ms = get_flag_value(&args, &["--timeout-ms", "--timeout"])
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(8000)
        .max(1);
    let strict = has_flag(&args, &["--strict"]);
    let dump_json = has_flag(&args, &["--dump-json", "--dump"]);

    let term = match artist {
        Some(artist) => lyrics::model::LyricsSearchTerm::Info {
            title: title.trim().to_string(),
            artist,
            album,
        },
        None => lyrics::model::LyricsSearchTerm::Keyword {
            keyword: title.trim().to_string(),
        },
    };
    let mut req = lyrics::model::LyricsSearchRequest::new(term);
    req.duration_ms = duration_ms;
    req.limit = limit;

    let mut opt = lyrics::model::LyricsSearchOptions::default();
    opt.timeout_ms = timeout_ms;
    opt.strict_match = strict;
    opt.services = services_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| lyrics::model::LyricsService::from_str(s))
        .collect::<Result<Vec<_>, _>>()?;

    let items = lyrics::core::search(&req, opt).await?;

    if dump_json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    eprintln!("OK: {} results (timeout={}ms)", items.len(), timeout_ms);
    for (i, it) in items.iter().enumerate() {
        eprintln!(
            "[{}] service={} match={} quality={:.4} title={} artist={} album={} token={}",
            i,
            it.service,
            it.match_percentage,
            it.quality,
            it.title.as_deref().unwrap_or("-"),
            it.artist.as_deref().unwrap_or("-"),
            it.album.as_deref().unwrap_or("-"),
            it.service_token
        );
    }
    Ok(())
}
