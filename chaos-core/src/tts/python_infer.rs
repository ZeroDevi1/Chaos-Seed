//! 使用嵌入式 Python 运行 VoiceLab 的 `tools/infer_sft.py`（直接加载 .pt checkpoint）。
//!
//! 说明：
//! - 本模块用于“完整复刻 Python 推理命令”的兜底/兼容路径（llm_ckpt/flow_ckpt 为 .pt）。
//! - 由于 infer_sft.py 是黑盒脚本，这里采用 `runpy.run_path()` + `sys.argv` 的方式执行。
//! - 取消：无法硬中断 Python 脚本执行；仅支持“开始前/结束后”检查取消标记，并丢弃结果。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;
use pyo3::types::IntoPyDict;
use pyo3::types::PyList;

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
    // 目前主要覆盖 Windows 发行环境（VoiceLab/uv venv 默认目录）。
    // - <workdir>/.venv/Lib/site-packages
    let p_win = workdir.join(".venv").join("Lib").join("site-packages");
    if p_win.exists() {
        return Some(p_win.to_string_lossy().to_string());
    }
    None
}

fn detect_torch_python_abi(site_pkgs: &Path) -> Option<(u8, u8, String)> {
    // 通过 torch 的二进制扩展名推断 wheel 的 Python ABI，例如：
    // - torch/_C.cp310-win_amd64.pyd  => Python 3.10
    // - torch/_C.cp39-win_amd64.pyd   => Python 3.9
    //
    // 这样可以在“嵌入式 Python 版本不匹配”时给出更明确的报错（否则常见是 WinError 126）。
    let torch_dir = site_pkgs.join("torch");
    if !torch_dir.exists() {
        return None;
    }
    let rd = std::fs::read_dir(&torch_dir).ok()?;
    for it in rd.flatten() {
        let p = it.path();
        if p.extension()
            .and_then(OsStr::to_str)
            .unwrap_or("")
            .eq_ignore_ascii_case("pyd")
        {
            let name = p.file_name()?.to_string_lossy();
            // 找到 "cp" + 数字（2 或 3 位），例如 cp39 / cp310
            let s = name.as_ref();
            let bytes = s.as_bytes();
            for i in 0..bytes.len().saturating_sub(3) {
                if bytes.get(i) == Some(&b'c') && bytes.get(i + 1) == Some(&b'p') {
                    let mut j = i + 2;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    let digits = &s[i + 2..j];
                    if digits.len() == 2 || digits.len() == 3 {
                        if let Ok(v) = digits.parse::<u16>() {
                            let (major, minor) = if digits.len() == 2 {
                                // cp39 => 3.9
                                (3u8, (v % 10) as u8)
                            } else {
                                // cp310 => 3.10
                                ((v / 100) as u8, (v % 100) as u8)
                            };
                            return Some((major, minor, format!("cp{digits}")));
                        }
                    }
                }
            }
        }
    }
    None
}

fn derive_venv_root_from_site_packages(site_pkgs: &Path) -> Option<PathBuf> {
    // Windows venv 典型结构：<venv>/Lib/site-packages
    // site_pkgs.parent() = Lib, parent().parent() = <venv>
    let lib_dir = site_pkgs.parent()?;
    if !lib_dir
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("Lib"))
    {
        return None;
    }
    lib_dir.parent().map(|p| p.to_path_buf())
}

fn pick_latest_wav(out_dir: &Path) -> Result<PathBuf, TtsError> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let rd = std::fs::read_dir(out_dir).map_err(|e| {
        TtsError::InvalidArg(format!(
            "python out_dir not readable: {}: {e}",
            out_dir.display()
        ))
    })?;
    for it in rd {
        let it = it.map_err(|e| TtsError::Io(e))?;
        let p = it.path();
        if p.extension()
            .and_then(OsStr::to_str)
            .unwrap_or("")
            .eq_ignore_ascii_case("wav")
        {
            let meta = it.metadata().map_err(|e| TtsError::Io(e))?;
            let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &best {
                None => best = Some((mtime, p)),
                Some((t0, _)) if mtime > *t0 => best = Some((mtime, p)),
                _ => {}
            }
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| {
        TtsError::Candle(format!(
            "python infer finished but produced no .wav in out_dir={}",
            out_dir.display()
        ))
    })
}

fn prompt_strategy_as_py(s: PromptStrategy) -> &'static str {
    match s {
        PromptStrategy::Inject => "inject",
        PromptStrategy::GuidePrefix => "guide_prefix",
    }
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

    // out_dir：
    // - 默认用临时目录承接 python 的输出（避免污染仓库 out_wav/）。
    // - 若设置 `CHAOS_TTS_PY_OUT_DIR`，则严格对齐 python 命令行的 --out_dir（适合 A/B 对齐测试）。
    let out_dir_raw = env_string("CHAOS_TTS_PY_OUT_DIR");
    let (out_dir, out_dir_arg, should_cleanup_out_dir) = if let Some(raw) = out_dir_raw.as_ref() {
        let raw = raw.trim().to_string();
        let p = PathBuf::from(&raw);
        let abs = if p.is_absolute() {
            p
        } else {
            // python 侧会在 chdir(workdir) 后解析相对路径；这里读文件时也按 workdir 解析。
            workdir_path.join(&raw)
        };
        (abs, raw, false)
    } else {
        let p = std::env::temp_dir().join(format!("chaos_tts_py_{}", fastrand::u64(..)));
        (p.clone(), p.to_string_lossy().to_string(), true)
    };
    std::fs::create_dir_all(&out_dir).map_err(|e| TtsError::Io(e))?;

    // 构造 argv：尽量与 VoiceLab 的 infer_sft.py 参数保持一致。
    // 注意：infer_sft.py 的参数名为 --spk_id/--guide_sep 等（下划线风格）。
    let argv: Vec<String> = vec![
        script_path.to_string_lossy().to_string(),
        "--model_dir".into(),
        p.model_dir.clone(),
        "--spk_id".into(),
        p.spk_id.clone(),
        "--text".into(),
        p.text.clone(),
        "--out_dir".into(),
        out_dir_arg,
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
    let mut argv = argv;
    if !p.text_frontend {
        // infer_sft.py: argparse.BooleanOptionalAction
        argv.push("--no-text_frontend".into());
    }

    let site_pkgs = env_string("CHAOS_TTS_PY_VENV_SITE_PACKAGES")
        .or_else(|| pick_default_site_packages(&workdir_path));
    let torch_abi = site_pkgs
        .as_ref()
        .and_then(|p| detect_torch_python_abi(Path::new(p)));
    let script_dir = script_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| workdir_path.clone());

    // 运行 Python。
    let run_res: Result<(), TtsError> = Python::with_gil(|py| -> Result<(), TtsError> {
        // sys.path：
        // - 先插入脚本目录（等价于 `python tools/infer_sft.py` 时的 sys.path[0]），确保能 import `voicelab_bootstrap`。
        // - 再注入 venv/site-packages（便于在嵌入式 python 下 import 成功；支持 env 显式指定，也支持自动探测 workdir/.venv）。
        let sys = py
            .import("sys")
            .map_err(|e| TtsError::Candle(e.to_string()))?;
        if env_bool("CHAOS_TTS_PY_DEBUG") {
            // 仅在 debug 时打印，避免污染正常输出。
            // 这些信息对排查 “torch DLL load failed / WinError 126” 很关键（通常是 Python 版本/ABI 不匹配）。 
            let platform = py
                .import("platform")
                .map_err(|e| TtsError::Candle(e.to_string()))?;
            let version: String = sys
                .getattr("version")
                .map_err(|e| TtsError::Candle(e.to_string()))?
                .extract()
                .unwrap_or_else(|_| "<unknown>".to_string());
            let executable: String = sys
                .getattr("executable")
                .map_err(|e| TtsError::Candle(e.to_string()))?
                .extract()
                .unwrap_or_else(|_| "<unknown>".to_string());
            let prefix: String = sys
                .getattr("prefix")
                .map_err(|e| TtsError::Candle(e.to_string()))?
                .extract()
                .unwrap_or_else(|_| "<unknown>".to_string());
            let base_prefix: String = sys
                .getattr("base_prefix")
                .map_err(|e| TtsError::Candle(e.to_string()))?
                .extract()
                .unwrap_or_else(|_| "<unknown>".to_string());
            let arch: String = platform
                .call_method0("architecture")
                .map_err(|e| TtsError::Candle(e.to_string()))?
                .extract()
                .unwrap_or_else(|_| "<unknown>".to_string());
            eprintln!(
                "[pyo3(pt)] python: version={} executable={} prefix={} base_prefix={} arch={}",
                version, executable, prefix, base_prefix, arch
            );
        }

        if let Some((need_major, need_minor, tag)) = torch_abi.as_ref() {
            // 如果 site-packages 的 torch wheel 与当前嵌入式 Python 版本不匹配，
            // 继续执行通常会变成 WinError 126（无法加载 .pyd/.dll），这里提前给出明确提示。
            let vi = sys
                .getattr("version_info")
                .map_err(|e| TtsError::Candle(e.to_string()))?;
            let cur_major: u8 = vi
                .getattr("major")
                .map_err(|e| TtsError::Candle(e.to_string()))?
                .extract()
                .unwrap_or(0);
            let cur_minor: u8 = vi
                .getattr("minor")
                .map_err(|e| TtsError::Candle(e.to_string()))?
                .extract()
                .unwrap_or(0);
            if cur_major != *need_major || cur_minor != *need_minor {
                return Err(TtsError::InvalidArg(format!(
                    "python ABI mismatch: embedded python={cur_major}.{cur_minor}, but torch wheel in site-packages requires {need_major}.{need_minor} ({tag}). \
请在编译时设置环境变量 `PYO3_PYTHON` 指向对应版本的 python.exe（建议指向 VoiceLab 的 .venv\\Scripts\\python.exe），然后重新编译/运行。"
                )));
            }
        }

        let path = sys
            .getattr("path")
            .map_err(|e| TtsError::Candle(e.to_string()))?;
        let _ = path
            .call_method1("insert", (0usize, script_dir.to_string_lossy().to_string()))
            .map_err(|e| TtsError::Candle(e.to_string()))?;

        if let Some(site) = site_pkgs.as_ref() {
            // 1) 用 site.addsitedir 解析 .pth（更贴近 venv python 行为）
            let site_mod = py
                .import("site")
                .map_err(|e| TtsError::Candle(e.to_string()))?;
            site_mod
                .call_method1("addsitedir", (site.as_str(),))
                .map_err(|e| TtsError::Candle(e.to_string()))?;

            // 2) 防御：确保路径靠前（避免被同名包覆盖）
            let _ = path
                .call_method1("insert", (0usize, site.as_str()))
                .map_err(|e| TtsError::Candle(e.to_string()))?;

            // 3) Windows：torch/torchaudio 通常需要 DLL 搜索路径；best-effort 增加 torch/lib
            // 注意：这不是严格必须（取决于具体 wheel），但能显著减少 “DLL load failed” 之类问题。
            let os = py
                .import("os")
                .map_err(|e| TtsError::Candle(e.to_string()))?;
            if let Ok(add_dll) = os.getattr("add_dll_directory") {
                // 重要：add_dll_directory 返回的 handle 必须保持引用，否则可能被 GC 回收导致目录失效。
                // 这里把 handle 挂到 sys 模块上，确保本次脚本执行期间有效。
                let mut handles: Vec<PyObject> = Vec::new();

                // 额外增加 venv 的目录：很多 wheel 依赖会在 venv/Scripts 或 venv 根目录下解析 DLL。
                let site_path = PathBuf::from(site);
                if let Some(venv_root) = derive_venv_root_from_site_packages(&site_path) {
                    let venv_scripts = venv_root.join("Scripts");
                    let venv_library_bin = venv_root.join("Library").join("bin");
                    if venv_scripts.exists() {
                        if let Ok(h) =
                            add_dll.call1((venv_scripts.to_string_lossy().to_string(),))
                        {
                            handles.push(h.into_py(py));
                        }
                    }
                    // uv/conda 风格的 venv 里经常会把 MKL 等依赖放在 <venv>/Library/bin，
                    // 而 torch_cuda.dll 等会直接依赖它们（缺失时常见 WinError 126）。
                    if venv_library_bin.exists() {
                        if let Ok(h) =
                            add_dll.call1((venv_library_bin.to_string_lossy().to_string(),))
                        {
                            handles.push(h.into_py(py));
                        }
                    }
                    if venv_root.exists() {
                        if let Ok(h) = add_dll.call1((venv_root.to_string_lossy().to_string(),)) {
                            handles.push(h.into_py(py));
                        }
                    }
                    // PATH 也做一次前置，提升兼容性（某些依赖走 PATH 而不是 AddDllDirectory）。
                    if let Ok(env) = os.getattr("environ") {
                        let prev: String = match env.get_item("PATH") {
                            Ok(v) => v.extract::<String>().unwrap_or_default(),
                            Err(_) => String::new(),
                        };
                        let new_path = format!(
                            "{};{};{};{}",
                            venv_scripts.to_string_lossy(),
                            venv_library_bin.to_string_lossy(),
                            venv_root.to_string_lossy(),
                            prev
                        );
                        let _ = env.set_item("PATH", new_path);
                    }
                }

                let torch_lib = PathBuf::from(site).join("torch").join("lib");
                if torch_lib.exists() {
                    if let Ok(h) = add_dll.call1((torch_lib.to_string_lossy().to_string(),)) {
                        handles.push(h.into_py(py));
                    }
                    // 某些依赖可能通过 PATH 搜索 DLL（而不是 AddDllDirectory），这里也做一次 PATH 前置以增强兼容性。
                    if let Ok(env) = os.getattr("environ") {
                        let prev: String = match env.get_item("PATH") {
                            Ok(v) => v.extract::<String>().unwrap_or_default(),
                            Err(_) => String::new(),
                        };
                        let new_path = format!("{};{}", torch_lib.to_string_lossy(), prev);
                        let _ = env.set_item("PATH", new_path);
                    }
                }
                let torchaudio_lib = PathBuf::from(site).join("torchaudio").join("lib");
                if torchaudio_lib.exists() {
                    if let Ok(h) = add_dll.call1((torchaudio_lib.to_string_lossy().to_string(),)) {
                        handles.push(h.into_py(py));
                    }
                }

                if !handles.is_empty() {
                    let keep =
                        PyList::new(py, &handles).map_err(|e| TtsError::Candle(e.to_string()))?;
                    // 不覆盖用户可能已有的同名字段；若存在则 append。
                    match sys.getattr("_chaos_added_dll_dirs") {
                        Ok(prev) => {
                            let _ = prev.call_method1("extend", (keep,));
                        }
                        Err(_) => {
                            sys.setattr("_chaos_added_dll_dirs", keep)
                                .map_err(|e| TtsError::Candle(e.to_string()))?;
                        }
                    }
                }
            }
        }

        let os = py
            .import("os")
            .map_err(|e| TtsError::Candle(e.to_string()))?;
        os.call_method1("chdir", (workdir.as_str(),))
            .map_err(|e| TtsError::Candle(e.to_string()))?;

        let py_argv = PyList::new(py, &argv).map_err(|e| TtsError::Candle(e.to_string()))?;
        sys.setattr("argv", py_argv)
            .map_err(|e| TtsError::Candle(e.to_string()))?;

        let runpy = py
            .import("runpy")
            .map_err(|e| TtsError::Candle(e.to_string()))?;
        // run_name="__main__"：让脚本按命令行方式执行。
        let kwargs = [("run_name", "__main__")]
            .into_py_dict(py)
            .map_err(|e| TtsError::Candle(e.to_string()))?;
        match runpy.call_method(
            "run_path",
            (script_path.to_string_lossy().to_string(),),
            Some(&kwargs),
        ) {
            Ok(_) => {}
            Err(e) => {
                // VoiceLab 的脚本末尾可能调用 sys.exit()；这种情况下 runpy 会抛 SystemExit。
                // code=0 视为成功（继续去 out_dir 里取 wav）；非 0 才当错误返回。
                if e.is_instance_of::<pyo3::exceptions::PySystemExit>(py) {
                    let code = e
                        .value(py)
                        .getattr("code")
                        .ok()
                        .and_then(|v| v.extract::<i64>().ok())
                        .unwrap_or(0);
                    if code != 0 {
                        return Err(TtsError::Candle(format!("python SystemExit: code={code}")));
                    }
                } else {
                    return Err(TtsError::Candle(e.to_string()));
                }
            }
        }

        Ok(())
    });

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

    // 读取输出 wav。
    let wav_path = pick_latest_wav(&out_dir)?;
    let wav_bytes = std::fs::read(&wav_path).map_err(|e| TtsError::Io(e))?;
    let meta = read_wav_meta_from_bytes(&wav_bytes)?;

    let r = TtsWavResult {
        sample_rate: meta.sample_rate,
        channels: meta.channels,
        duration_ms: duration_ms(meta.sample_rate, meta.samples as usize),
        wav_bytes,
    };

    // best-effort cleanup
    if should_cleanup_out_dir {
        let _ = std::fs::remove_dir_all(&out_dir);
    }
    Ok(r)
}
