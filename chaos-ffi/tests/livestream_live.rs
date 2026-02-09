use std::ffi::{CStr, CString};
use std::process;

use chaos_ffi::{
    chaos_ffi_last_error_json, chaos_ffi_string_free, chaos_livestream_decode_manifest_json,
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

fn decode_manifest(input: &str) -> Result<serde_json::Value, String> {
    let c = CString::new(input).map_err(|_| "input contains NUL".to_string())?;
    let p = chaos_livestream_decode_manifest_json(c.as_ptr(), 1);
    if p.is_null() {
        let err = take_last_error_string().unwrap_or_else(|| "unknown error".to_string());
        return Err(err);
    }
    let s = unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
    chaos_ffi_string_free(p);
    Ok(v)
}

fn usage() {
    eprintln!("livestream live integration check (harness=false)");
    eprintln!("usage:");
    eprintln!("  cargo test -p chaos-ffi --features live-tests --test livestream_live -- \\");
    eprintln!("    --bili-url <URL> --huya-url <URL>");
    eprintln!("    [--dump-json]");
    eprintln!();
    eprintln!("notes:");
    eprintln!("  - pass any subset of urls; omitted ones are skipped");
    eprintln!("  - no real URLs are stored in the repository (you pass them at runtime)");
    eprintln!("  - use --dump-json to print the decoded JSON payload");
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

fn check_site(v: &serde_json::Value, expected_site: &str) -> Result<(), String> {
    let site = v
        .get("site")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if site != expected_site {
        return Err(format!(
            "unexpected site: got={site}, expected={expected_site}"
        ));
    }
    let rid = v.get("room_id").and_then(|v| v.as_str()).unwrap_or("");
    if rid.trim().is_empty() {
        return Err("room_id is empty".to_string());
    }

    let is_living = v
        .pointer("/info/is_living")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let empty: Vec<serde_json::Value> = vec![];
    let variants = v
        .get("variants")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    if is_living {
        if variants.is_empty() {
            return Err("is_living=true but variants is empty".to_string());
        }
        let has_url = variants.iter().any(|vv| {
            vv.get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .trim()
                .len()
                > 0
        });
        if !has_url {
            return Err("is_living=true but no variant has url".to_string());
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        usage();
        return;
    }

    let bili = get_flag_value(&args, &["--bili-url", "--bili"]);
    let huya = get_flag_value(&args, &["--huya-url", "--huya"]);
    let dump_json = args.iter().any(|a| a == "--dump-json" || a == "--dump");

    if bili.is_none() && huya.is_none() {
        eprintln!("SKIP: no urls provided.");
        usage();
        return;
    }

    let mut failed = false;

    if let Some(url) = bili {
        eprintln!("checking BiliLive: {url}");
        match decode_manifest(&url) {
            Ok(v) => {
                if let Err(e) = check_site(&v, "BiliLive") {
                    eprintln!("FAIL: {e}");
                    failed = true;
                } else {
                    let title = v
                        .pointer("/info/title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let living = v
                        .pointer("/info/is_living")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let vars = v
                        .get("variants")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    eprintln!(
                        "OK: site=BiliLive room_id={} is_living={} variants={} title={}",
                        v.get("room_id").and_then(|v| v.as_str()).unwrap_or(""),
                        living,
                        vars,
                        title
                    );
                    if dump_json {
                        println!("{}", serde_json::to_string_pretty(&v).unwrap());
                    }
                }
            }
            Err(e) => {
                eprintln!("FAIL: decode_manifest error: {e}");
                failed = true;
            }
        }
    }

    if let Some(url) = huya {
        eprintln!("checking Huya: {url}");
        match decode_manifest(&url) {
            Ok(v) => {
                if let Err(e) = check_site(&v, "Huya") {
                    eprintln!("FAIL: {e}");
                    failed = true;
                } else {
                    let title = v
                        .pointer("/info/title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let living = v
                        .pointer("/info/is_living")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let vars = v
                        .get("variants")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    eprintln!(
                        "OK: site=Huya room_id={} is_living={} variants={} title={}",
                        v.get("room_id").and_then(|v| v.as_str()).unwrap_or(""),
                        living,
                        vars,
                        title
                    );
                    if dump_json {
                        println!("{}", serde_json::to_string_pretty(&v).unwrap());
                    }
                }
            }
            Err(e) => {
                eprintln!("FAIL: decode_manifest error: {e}");
                failed = true;
            }
        }
    }

    if failed {
        process::exit(1);
    }
}
