fn usage() {
    eprintln!("now playing live integration check (harness=false)");
    eprintln!("usage:");
    eprintln!("  cargo test -p chaos-ffi --features live-tests --test now_playing_live -- \\");
    eprintln!(
        "    [--no-thumbnail] [--max-thumbnail-bytes <N>] [--max-sessions <N>] [--repeat <N>]"
    );
    eprintln!("    [--dump-json] [--no-list-sessions]");
    eprintln!();
    eprintln!("notes:");
    eprintln!("  - on non-Windows platforms, this test prints SKIP and exits 0");
    eprintln!("  - on Windows, it queries GSMTC sessions and prints a summary per iteration");
}

#[cfg(not(windows))]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        usage();
        return;
    }
    eprintln!("SKIP: not on Windows.");
}

#[cfg(windows)]
mod windows_impl {
    use std::ffi::CStr;
    use std::process;

    use chaos_ffi::{
        chaos_ffi_last_error_json, chaos_ffi_string_free, chaos_now_playing_snapshot_json,
    };

    fn take_last_error_string() -> Option<String> {
        let p = chaos_ffi_last_error_json();
        if p.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(p) }
            .to_str()
            .ok()
            .map(|s| s.to_string());
        chaos_ffi_string_free(p);
        s
    }

    fn get_flag_value(args: &[String], names: &[&str]) -> Option<String> {
        let mut i = 0;
        while i < args.len() {
            let a = args[i].as_str();
            if names.iter().any(|n| *n == a) {
                if i + 1 < args.len() {
                    return Some(args[i + 1].clone());
                }
            }
            i += 1;
        }
        None
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "-h" || a == "--help") {
            super::usage();
            return;
        }

        let include_thumbnail = !args
            .iter()
            .any(|a| a == "--no-thumbnail" || a == "--no-thumb");
        let list_sessions = !args
            .iter()
            .any(|a| a == "--no-list-sessions" || a == "--no-list");
        let max_thumbnail_bytes =
            get_flag_value(&args, &["--max-thumbnail-bytes", "--thumb-bytes"])
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(262_144);
        let max_sessions = get_flag_value(&args, &["--max-sessions", "--sessions"])
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(32);
        let repeat = get_flag_value(&args, &["--repeat", "--n"])
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .max(1)
            .min(200);
        let dump_json = args.iter().any(|a| a == "--dump-json" || a == "--dump");

        let mut failed = false;
        for i in 0..repeat {
            let p = chaos_now_playing_snapshot_json(
                if include_thumbnail { 1 } else { 0 },
                max_thumbnail_bytes,
                max_sessions,
            );
            if p.is_null() {
                let err = take_last_error_string().unwrap_or_else(|| "unknown error".to_string());
                eprintln!("FAIL: snapshot error: {err}");
                failed = true;
                break;
            }
            let s = unsafe { CStr::from_ptr(p) }
                .to_str()
                .unwrap_or("")
                .to_string();
            chaos_ffi_string_free(p);

            let v: serde_json::Value = match serde_json::from_str(&s) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("FAIL: invalid json: {e}");
                    failed = true;
                    break;
                }
            };

            let supported = v
                .get("supported")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let sessions = v
                .get("sessions")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let sessions_len = sessions.len();
            let np = v.get("now_playing");

            let (app_id, title, artist, cover_bytes) = if let Some(np) = np {
                let app_id = np.get("app_id").and_then(|v| v.as_str()).unwrap_or("");
                let title = np.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let artist = np.get("artist").and_then(|v| v.as_str()).unwrap_or("");
                let cover_bytes = np
                    .pointer("/thumbnail/base64")
                    .and_then(|v| v.as_str())
                    .map(|b64| b64.len())
                    .unwrap_or(0);
                (
                    app_id.to_string(),
                    title.to_string(),
                    artist.to_string(),
                    cover_bytes,
                )
            } else {
                (String::new(), String::new(), String::new(), 0usize)
            };

            eprintln!(
                "OK[{}/{}]: supported={} sessions={} now_playing_app_id={} title={} artist={} cover_b64_len={}",
                i + 1,
                repeat,
                supported,
                sessions_len,
                app_id,
                title,
                artist,
                cover_bytes
            );

            if list_sessions && !sessions.is_empty() {
                for (idx, s) in sessions.iter().enumerate() {
                    let app_id = s.get("app_id").and_then(|v| v.as_str()).unwrap_or("");
                    let status = s
                        .get("playback_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let title = s.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let artist = s.get("artist").and_then(|v| v.as_str()).unwrap_or("");
                    eprintln!(
                        "  SESSION[{}]: app_id={} status={} title={} artist={}",
                        idx, app_id, status, title, artist
                    );
                }
            }

            if dump_json {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            }

            if i + 1 < repeat {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }

        if failed {
            process::exit(1);
        }
    }
}

#[cfg(windows)]
fn main() {
    windows_impl::run();
}
