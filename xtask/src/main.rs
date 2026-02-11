use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::{fs, path::Path};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        return Err(help());
    };

    match cmd.as_str() {
        "build-winui3" => {
            let mut release = false;
            for a in args {
                if a == "--release" {
                    release = true;
                }
            }
            build_winui3(release)
        }
        "-h" | "--help" | "help" => Err(help()),
        other => Err(format!("unknown xtask command: {other}\n\n{}", help())),
    }
}

fn help() -> String {
    [
        "xtask commands:",
        "  cargo xtask build-winui3 [--release]",
        "",
        "Notes:",
        "  - build-winui3 is intended to run on Windows (will call msbuild/dotnet).",
    ]
    .join("\n")
}

fn build_winui3(release: bool) -> Result<(), String> {
    let profile = if release { "release" } else { "debug" };

    let mut cargo = Command::new("cargo");
    cargo.arg("build").arg("-p").arg("chaos-daemon");
    if release {
        cargo.arg("--release");
    }
    run_cmd(cargo)?;

    let mut cargo_ffi = Command::new("cargo");
    cargo_ffi.arg("build").arg("-p").arg("chaos-ffi");
    if release {
        cargo_ffi.arg("--release");
    }
    run_cmd(cargo_ffi)?;

    if !cfg!(windows) {
        return Ok(());
    }

    let root = repo_root()?;
    let sln = root.join("chaos-winui3").join("ChaosSeed.WinUI3.sln");
    if !sln.exists() {
        return Err(format!("missing solution: {}", sln.display()));
    }

    ensure_winui3_ffmpeg(&root)?;

    let cfg = if release { "Release" } else { "Debug" };

    // Prefer MSBuild; fall back to dotnet build.
    let mut msbuild_cmd = Command::new("msbuild");
    msbuild_cmd
        .arg(&sln)
        .arg("/restore")
        .arg(format!("/p:Configuration={cfg}"))
        .arg("/p:Platform=x64");

    match run_cmd_allow_not_found(msbuild_cmd) {
        Ok(()) => return Ok(()),
        Err(RunErr::NotFound) => {}
        Err(RunErr::Failed(e)) => return Err(e),
    }

    let mut dotnet = Command::new("dotnet");
    dotnet
        .arg("build")
        .arg(&sln)
        .arg("-c")
        .arg(cfg)
        .arg("-p:Platform=x64");
    run_cmd(dotnet)?;

    // Verify daemon exists (copy is handled by csproj).
    let exe = root
        .join("target")
        .join(profile)
        .join("chaos-daemon.exe");
    if !exe.exists() {
        return Err(format!("expected daemon at {}", exe.display()));
    }

    let ffi = root.join("target").join(profile).join("chaos_ffi.dll");
    if !ffi.exists() {
        return Err(format!("expected ffi dll at {}", ffi.display()));
    }

    Ok(())
}

fn ensure_winui3_ffmpeg(root: &Path) -> Result<(), String> {
    let proj = root.join("chaos-winui3").join("ChaosSeed.WinUI3");
    let ffmpeg_dir = proj.join("FFmpeg");

    if has_ffmpeg_dlls(&ffmpeg_dir) {
        return Ok(());
    }

    let script = root.join("scripts").join("fetch_ffmpeg_win.ps1");
    if !script.exists() {
        return Err(format!(
            "missing ffmpeg fetch script: {}",
            script.display()
        ));
    }

    eprintln!(
        "> FFmpeg DLLs not found under {}; fetching via {}",
        ffmpeg_dir.display(),
        script.display()
    );

    // Prefer Windows PowerShell; fall back to pwsh.
    let mut ps = Command::new("powershell");
    ps.arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .arg("-ProjectDir")
        .arg(&proj)
        .arg("-Track")
        .arg("n8.0");

    match run_cmd_allow_not_found(ps) {
        Ok(()) => return Ok(()),
        Err(RunErr::NotFound) => {}
        Err(RunErr::Failed(e)) => return Err(e),
    }

    let mut pwsh = Command::new("pwsh");
    pwsh.arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .arg("-ProjectDir")
        .arg(&proj)
        .arg("-Track")
        .arg("n8.0");

    run_cmd(pwsh)?;

    if !has_ffmpeg_dlls(&ffmpeg_dir) {
        return Err(format!(
            "ffmpeg fetch succeeded but DLLs still missing under {}",
            ffmpeg_dir.display()
        ));
    }

    Ok(())
}

fn has_ffmpeg_dlls(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if !p.is_file() {
            continue;
        }
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let n = name.to_ascii_lowercase();
        if n.starts_with("avcodec") && n.ends_with(".dll") {
            return true;
        }
    }
    false
}

fn repo_root() -> Result<PathBuf, String> {
    let here = env::current_dir().map_err(|e| e.to_string())?;
    // Heuristic: look upwards for Cargo.toml workspace file.
    for anc in here.ancestors() {
        let p = anc.join("Cargo.toml");
        if p.exists() {
            return Ok(anc.to_path_buf());
        }
    }
    Err("failed to locate repo root".to_string())
}

fn run_cmd(mut cmd: Command) -> Result<(), String> {
    eprintln!("> {:?}", cmd);
    let status = cmd.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("command failed with status: {status}"));
    }
    Ok(())
}

enum RunErr {
    NotFound,
    Failed(String),
}

fn run_cmd_allow_not_found(mut cmd: Command) -> Result<(), RunErr> {
    eprintln!("> {:?}", cmd);
    let status = cmd.status().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RunErr::NotFound
        } else {
            RunErr::Failed(e.to_string())
        }
    })?;
    if !status.success() {
        return Err(RunErr::Failed(format!(
            "command failed with status: {status}"
        )));
    }
    Ok(())
}
