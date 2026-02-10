use std::str::FromStr;

use chaos_core::lyrics::model::{LyricsSearchOptions, LyricsSearchRequest, LyricsSearchTerm, LyricsService};

fn usage() -> String {
    [
        "usage:",
        "  cargo run -- test \"<title>\" \"<album>\" \"<artist>\"",
        "  cargo run -- test --title \"<title>\" --artist \"<artist>\" [--album \"<album>\"]",
        "",
        "flags:",
        "  --timeout-ms <n>     request timeout per provider (default 10000)",
        "  --limit <n>          max results (default 6)",
        "  --duration-ms <n>    expected duration (optional)",
        "  --strict             enable strict match filter",
        "  --services <csv>     e.g. netease,qq,kugou",
        "  --dump-json          print full json array",
    ]
    .join("\n")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("test") {
        args.remove(0);
    }
    if args.is_empty() {
        eprintln!("{}", usage());
        std::process::exit(2);
    }

    let is_flag_mode = args.iter().any(|a| a.starts_with("--"));

    let mut title: Option<String> = None;
    let mut artist: Option<String> = None;
    let mut album: Option<String> = None;
    let mut timeout_ms: u64 = 10_000;
    let mut limit: usize = 6;
    let mut duration_ms: Option<u64> = None;
    let mut strict_match = false;
    let mut services: Option<Vec<LyricsService>> = None;
    let mut dump_json = false;

    if !is_flag_mode {
        // positional: <title> <album?> <artist?>
        title = args.get(0).cloned().filter(|s| !s.trim().is_empty());
        album = args.get(1).cloned().filter(|s| !s.trim().is_empty());
        artist = args.get(2).cloned().filter(|s| !s.trim().is_empty());
    } else {
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--title" => {
                    i += 1;
                    title = args.get(i).cloned();
                }
                "--artist" => {
                    i += 1;
                    artist = args.get(i).cloned();
                }
                "--album" => {
                    i += 1;
                    album = args.get(i).cloned();
                }
                "--timeout-ms" => {
                    i += 1;
                    timeout_ms = args
                        .get(i)
                        .ok_or("--timeout-ms requires a value")?
                        .parse()?;
                }
                "--limit" => {
                    i += 1;
                    limit = args.get(i).ok_or("--limit requires a value")?.parse()?;
                }
                "--duration-ms" => {
                    i += 1;
                    duration_ms = Some(args.get(i).ok_or("--duration-ms requires a value")?.parse()?);
                }
                "--strict" => strict_match = true,
                "--services" => {
                    i += 1;
                    let csv = args.get(i).ok_or("--services requires a value")?;
                    let mut out = Vec::new();
                    for part in csv.split(',') {
                        if part.trim().is_empty() {
                            continue;
                        }
                        out.push(LyricsService::from_str(part)?);
                    }
                    services = Some(out);
                }
                "--dump-json" => dump_json = true,
                "-h" | "--help" => {
                    println!("{}", usage());
                    return Ok(());
                }
                other => return Err(format!("unknown arg: {other}\n\n{}", usage()).into()),
            }
            i += 1;
        }
    }

    let title = title.unwrap_or_default().trim().to_string();
    if title.is_empty() {
        return Err("missing title".into());
    }
    let artist = artist.unwrap_or_default().trim().to_string();
    let album = album.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    let term = LyricsSearchTerm::Info { title, artist, album };
    let mut req = LyricsSearchRequest::new(term);
    req.limit = limit.max(1);
    req.duration_ms = duration_ms.filter(|v| *v > 0);

    let mut opt = LyricsSearchOptions::default();
    opt.timeout_ms = timeout_ms.max(1);
    opt.strict_match = strict_match;
    if let Some(s) = services {
        opt.services = s;
    }

    let items = chaos_core::lyrics::core::search(&req, opt).await?;

    if dump_json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    for (i, it) in items.iter().enumerate() {
        println!(
            "#{i} quality={:.4} service={} title={:?} artist={:?} album={:?} matched={}",
            it.quality, it.service, it.title, it.artist, it.album, it.matched
        );
    }
    if let Some(best) = items.first() {
        let text = if best.lyrics_original.len() > 4000 {
            &best.lyrics_original[..4000]
        } else {
            &best.lyrics_original
        };
        println!("\n--- best lyrics (truncated) ---\n{text}");
    }

    Ok(())
}

