use std::time::Duration;

use chaos_core::subtitle;

fn usage() {
    eprintln!("subtitle search (Thunder)");
    eprintln!("usage:");
    eprintln!("  cargo run -p chaos-core --example subtitle_search -- \\");
    eprintln!(
        "    --query <QUERY> [--limit <N>] [--min-score <S>] [--lang <LANG>] [--timeout-ms <N>] [--dump-json]"
    );
    eprintln!();
    eprintln!("examples:");
    eprintln!("  cargo run -p chaos-core --example subtitle_search -- --query \"Dune\"");
    eprintln!(
        "  cargo run -p chaos-core --example subtitle_search -- --query \"Dune\" --lang zh --min-score 8.0 --dump-json"
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

    let query = get_flag_value(&args, &["--query"]).unwrap_or_default();
    if query.trim().is_empty() {
        usage();
        std::process::exit(2);
    }

    let limit = get_flag_value(&args, &["--limit"])
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10)
        .clamp(1, 50);
    let min_score = get_flag_value(&args, &["--min-score"])
        .and_then(|s| s.parse::<f64>().ok())
        .and_then(|v| (v >= 0.0).then_some(v));
    let lang = get_flag_value(&args, &["--lang"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let timeout_ms = get_flag_value(&args, &["--timeout-ms", "--timeout"])
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(8000)
        .max(1);
    let dump_json = has_flag(&args, &["--dump-json", "--dump"]);

    let items = subtitle::core::search_items(
        query.trim(),
        limit,
        min_score,
        lang.as_deref(),
        Duration::from_millis(timeout_ms),
    )
    .await?;

    if dump_json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    eprintln!("OK: {} results (timeout={}ms)", items.len(), timeout_ms);
    for (i, it) in items.iter().enumerate() {
        eprintln!(
            "[{}] score={:.3} ext={} langs={:?} name={} url={}",
            i, it.score, it.ext, it.languages, it.name, it.url
        );
    }
    Ok(())
}
