//! 使用外部 Python 子进程运行 VoiceLab 的 `tools/infer_sft.py`。
//!
//! 设计目标：
//! - 不把 `python3xx.dll` 变成主程序/主 DLL 的加载前置条件；
//! - 仅在真正调用 TTS/voice chat 时才探测 Python 运行环境；
//! - 尽量保持与现有 `infer_sft.py` 命令行参数一致。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::tts::wav::{duration_ms, read_wav_meta_from_bytes};
use crate::tts::{PromptStrategy, TtsError, TtsWavResult};

use super::TtsSftParams;

fn env_bool(key: &str) -> bool {
    matches!(
        env_string(key).as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn pick_default_site_packages(workdir: &Path) -> Option<String> {
    let p_win = workdir.join(".venv").join("Lib").join("site-packages");
    if p_win.exists() {
        return Some(p_win.to_string_lossy().to_string());
    }
    None
}

fn derive_venv_root_from_site_packages(site_packages: &Path) -> Option<PathBuf> {
    let lib = site_packages.parent()?;
    if !lib
        .file_name()
        .map(|s| s.to_string_lossy().eq_ignore_ascii_case("Lib"))
        .unwrap_or(false)
    {
        return None;
    }
    lib.parent().map(|p| p.to_path_buf())
}

fn prompt_strategy_as_py(v: PromptStrategy) -> &'static str {
    match v {
        PromptStrategy::Inject => "inject",
        PromptStrategy::GuidePrefix => "guide_prefix",
    }
}

fn resolve_python_exe(workdir: &Path, site_pkgs: Option<&str>) -> PathBuf {
    if let Some(v) = env_string("CHAOS_TTS_PYTHON_EXE") {
        return PathBuf::from(v);
    }

    if let Some(site) = site_pkgs {
        let site_path = PathBuf::from(site);
        if let Some(venv_root) = derive_venv_root_from_site_packages(&site_path) {
            let candidate = venv_root.join("Scripts").join("python.exe");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    if let Some(home) = env_string("PYTHONHOME") {
        let candidate = PathBuf::from(home).join("python.exe");
        if candidate.exists() {
            return candidate;
        }
    }

    let bundled = workdir.join("python").join("python.exe");
    if bundled.exists() {
        return bundled;
    }

    if cfg!(windows) {
        PathBuf::from("python.exe")
    } else {
        PathBuf::from("python3")
    }
}

fn prepend_env_path(mut current: String, parts: &[PathBuf]) -> String {
    for p in parts.iter().rev() {
        if !p.exists() {
            continue;
        }
        let s = p.to_string_lossy().to_string();
        if current
            .split(';')
            .any(|item| item.eq_ignore_ascii_case(s.as_str()))
        {
            continue;
        }
        if current.is_empty() {
            current = s;
        } else {
            current = format!("{s};{current}");
        }
    }
    current
}

fn build_python_command(
    p: &TtsSftParams,
    llm_ckpt: &str,
    flow_ckpt: &str,
    python_workdir: Option<&str>,
    python_infer_script: Option<&str>,
    out_dir: &Path,
    out_dir_arg: &str,
    stream: bool,
) -> Result<Command, TtsError> {
    let workdir = python_workdir
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_string("CHAOS_TTS_PY_WORKDIR"))
        .ok_or_else(|| {
            TtsError::InvalidArg(
                "missing python workdir: set `pythonWorkdir` or env CHAOS_TTS_PY_WORKDIR".into(),
            )
        })?;

    let infer_script = python_infer_script
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_string("CHAOS_TTS_PY_INFER_SFT"))
        .unwrap_or_else(|| "tools/infer_sft.py".to_string());

    let workdir_path = PathBuf::from(&workdir);
    let script_path = {
        let p = PathBuf::from(&infer_script);
        if p.is_absolute() {
            p
        } else {
            workdir_path.join(p)
        }
    };
    if !script_path.exists() {
        return Err(TtsError::InvalidArg(format!(
            "python infer script not found: {} (workdir={})",
            script_path.display(),
            workdir
        )));
    }

    std::fs::create_dir_all(out_dir)?;

    let mut argv: Vec<String> = vec![
        script_path.to_string_lossy().to_string(),
        "--model_dir".into(),
        p.model_dir.clone(),
        "--spk_id".into(),
        p.spk_id.clone(),
        "--text".into(),
        p.text.clone(),
        "--out_dir".into(),
        out_dir_arg.to_string(),
        "--llm_ckpt".into(),
        llm_ckpt.to_string(),
        "--flow_ckpt".into(),
        flow_ckpt.to_string(),
        "--prompt_text".into(),
        p.prompt_text.clone(),
        "--prompt_strategy".into(),
        prompt_strategy_as_py(p.prompt_strategy).into(),
        "--guide_sep".into(),
        p.guide_sep.clone(),
        "--speed".into(),
        format!("{}", p.speed),
        "--seed".into(),
        format!("{}", p.seed),
        "--temperature".into(),
        format!("{}", p.sampling.temperature),
        "--top_p".into(),
        format!("{}", p.sampling.top_p),
        "--top_k".into(),
        format!("{}", p.sampling.top_k),
        "--win_size".into(),
        format!("{}", p.sampling.win_size),
        "--tau_r".into(),
        format!("{}", p.sampling.tau_r),
    ];
    if !p.text_frontend {
        argv.push("--no-text_frontend".into());
    }
    if stream {
        argv.push("--stream".into());
    }

    let site_pkgs = env_string("CHAOS_TTS_PY_VENV_SITE_PACKAGES")
        .or_else(|| pick_default_site_packages(&workdir_path));
    let python_exe = resolve_python_exe(&workdir_path, site_pkgs.as_deref());
    let script_dir = script_path
        .parent()
        .map(|v| v.to_path_buf())
        .unwrap_or_else(|| workdir_path.clone());

    let mut path_parts: Vec<PathBuf> = Vec::new();
    if let Some(home) = env_string("PYTHONHOME") {
        path_parts.push(PathBuf::from(home));
    }
    if let Some(site) = site_pkgs.as_ref() {
        let site_path = PathBuf::from(site);
        if let Some(venv_root) = derive_venv_root_from_site_packages(&site_path) {
            path_parts.push(venv_root.join("Scripts"));
            path_parts.push(venv_root.join("Library").join("bin"));
            path_parts.push(venv_root);
        }
        path_parts.push(site_path.join("torch").join("lib"));
        path_parts.push(site_path.join("torchaudio").join("lib"));
    }

    let existing_path = std::env::var("PATH").unwrap_or_default();
    let merged_path = prepend_env_path(existing_path, &path_parts);

    let mut py_path_items = vec![script_dir.to_string_lossy().to_string()];
    if let Some(site) = site_pkgs.as_ref() {
        py_path_items.push(site.clone());
    }
    if let Some(existing) = env_string("PYTHONPATH") {
        py_path_items.push(existing);
    }
    let merged_pythonpath = py_path_items.join(";");

    if env_bool("CHAOS_TTS_PY_DEBUG") {
        eprintln!(
            "[pyproc] exe={} workdir={} script={} out_dir={}",
            python_exe.display(),
            workdir_path.display(),
            script_path.display(),
            out_dir.display()
        );
        if let Some(site) = site_pkgs.as_ref() {
            eprintln!("[pyproc] venv_site_packages={site}");
        }
    }

    let mut cmd = Command::new(&python_exe);
    cmd.current_dir(&workdir_path)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PATH", merged_path)
        .env("PYTHONPATH", merged_pythonpath);

    Ok(cmd)
}

fn run_python_and_wait(mut cmd: Command) -> Result<(), TtsError> {
    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TtsError::InvalidArg(
                "python executable not found: set `CHAOS_TTS_PYTHON_EXE`, or ensure `python.exe` is available in bundled runtime / PATH".into(),
            )
        } else {
            TtsError::Io(e)
        }
    })?;

    if output.status.success() {
        return Ok(());
    }

    let code = output
        .status
        .code()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "terminated".to_string());
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "python subprocess exited without error output".to_string()
    };

    Err(TtsError::Candle(format!(
        "python subprocess failed (exit={code}): {detail}"
    )))
}

fn pick_output_wav(out_dir: &Path) -> Result<PathBuf, TtsError> {
    let mut files = std::fs::read_dir(out_dir)?
        .filter_map(|entry| entry.ok().map(|v| v.path()))
        .filter(|path| {
            path.extension()
                .map(|ext| ext.to_string_lossy().eq_ignore_ascii_case("wav"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    files.sort_by(|a, b| {
        let ma = a
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mb = b
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        ma.cmp(&mb).then_with(|| a.cmp(b))
    });

    files.pop().ok_or_else(|| {
        TtsError::InvalidArg(format!("no wav output found under {}", out_dir.display()))
    })
}

/// 运行 Python 的 infer_sft.py，将结果写入指定 out_dir（用于流式：piece_*.wav）。
pub fn run_infer_sft_pt_to_out_dir_with_cancel(
    p: &TtsSftParams,
    llm_ckpt: &str,
    flow_ckpt: &str,
    python_workdir: Option<&str>,
    python_infer_script: Option<&str>,
    out_dir: &Path,
    stream: bool,
    cancel: Option<&AtomicBool>,
) -> Result<(), TtsError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(TtsError::Canceled);
        }
    }

    let out_dir_arg = out_dir.to_string_lossy().to_string();
    let cmd = build_python_command(
        p,
        llm_ckpt,
        flow_ckpt,
        python_workdir,
        python_infer_script,
        out_dir,
        &out_dir_arg,
        stream,
    )?;

    run_python_and_wait(cmd)?;

    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(TtsError::Canceled);
        }
    }

    Ok(())
}

/// 运行 Python 的 infer_sft.py，返回 WAV bytes（PCM16）。
pub fn infer_sft_pt_wav_bytes_with_cancel(
    p: &TtsSftParams,
    llm_ckpt: &str,
    flow_ckpt: &str,
    python_workdir: Option<&str>,
    python_infer_script: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> Result<TtsWavResult, TtsError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(TtsError::Canceled);
        }
    }

    let workdir = python_workdir
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_string("CHAOS_TTS_PY_WORKDIR"))
        .ok_or_else(|| {
            TtsError::InvalidArg(
                "missing python workdir: set `pythonWorkdir` or env CHAOS_TTS_PY_WORKDIR".into(),
            )
        })?;
    let workdir_path = PathBuf::from(&workdir);

    let out_dir_raw = env_string("CHAOS_TTS_PY_OUT_DIR");
    let (out_dir, out_dir_arg, should_cleanup_out_dir) = if let Some(raw) = out_dir_raw.as_ref() {
        let raw = raw.trim().to_string();
        let p = PathBuf::from(&raw);
        let abs = if p.is_absolute() {
            p
        } else {
            workdir_path.join(&raw)
        };
        (abs, raw, false)
    } else {
        let p = std::env::temp_dir().join(format!("chaos_tts_py_{}", fastrand::u64(..)));
        (p.clone(), p.to_string_lossy().to_string(), true)
    };

    let cmd = build_python_command(
        p,
        llm_ckpt,
        flow_ckpt,
        Some(&workdir),
        python_infer_script,
        &out_dir,
        &out_dir_arg,
        false,
    )?;

    let run_res = run_python_and_wait(cmd);
    if let Err(e) = run_res {
        if should_cleanup_out_dir {
            let _ = std::fs::remove_dir_all(&out_dir);
        }
        return Err(e);
    }

    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            if should_cleanup_out_dir {
                let _ = std::fs::remove_dir_all(&out_dir);
            }
            return Err(TtsError::Canceled);
        }
    }

    let wav_path = match pick_output_wav(&out_dir) {
        Ok(p) => p,
        Err(e) => {
            if should_cleanup_out_dir {
                let _ = std::fs::remove_dir_all(&out_dir);
            }
            return Err(e);
        }
    };
    let wav_bytes = std::fs::read(&wav_path)?;
    let meta = read_wav_meta_from_bytes(&wav_bytes)
        .map_err(|e| TtsError::Candle(format!("decode wav meta failed: {e}")))?;
    let duration_ms = duration_ms(meta.sample_rate, meta.samples as usize);

    if should_cleanup_out_dir {
        let _ = std::fs::remove_dir_all(&out_dir);
    }

    Ok(TtsWavResult {
        wav_bytes,
        sample_rate: meta.sample_rate,
        channels: meta.channels,
        duration_ms,
    })
}
