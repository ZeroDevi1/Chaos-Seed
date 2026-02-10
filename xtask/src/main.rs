use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

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

    if !cfg!(windows) {
        return Ok(());
    }

    let root = repo_root()?;
    let sln = root.join("chaos-winui3").join("ChaosSeed.WinUI3.sln");
    if !sln.exists() {
        return Err(format!("missing solution: {}", sln.display()));
    }

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

    Ok(())
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
