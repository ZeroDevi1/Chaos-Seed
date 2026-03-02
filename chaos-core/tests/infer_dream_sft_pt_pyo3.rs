#![cfg(feature = "live-tests")]

use std::path::{Path, PathBuf};

use chaos_core::tts::{PromptStrategy, SamplingConfig};

#[cfg(feature = "tts-python")]
use chaos_core::tts::TtsSftParams;

fn repo_root() -> PathBuf {
    // tests 位于 chaos-core crate 中：repo_root = chaos-core/..（workspace 根）
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn default_out_dir() -> PathBuf {
    repo_root().join("out_wav").join("dream")
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_parse<T: std::str::FromStr>(key: &str) -> Option<T> {
    env_string(key).and_then(|v| v.parse::<T>().ok())
}

fn env_prompt_strategy() -> PromptStrategy {
    match env_string("CHAOS_TTS_PROMPT_STRATEGY")
        .unwrap_or_else(|| "guide_prefix".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "inject" => PromptStrategy::Inject,
        _ => PromptStrategy::GuidePrefix,
    }
}

fn resolve_workdir() -> Option<PathBuf> {
    env_string("CHAOS_TTS_PY_WORKDIR")
        .map(PathBuf::from)
        .or_else(|| {
            // 默认指向仓库内嵌目录（需要先运行 tools/sync_voicelab_cosyvoice_min.ps1）
            Some(
                repo_root()
                    .join("third_party")
                    .join("voicelab_embed")
                    .join("workflows")
                    .join("cosyvoice"),
            )
        })
}

fn resolve_maybe_relative(base: &Path, p: &str) -> PathBuf {
    let p = PathBuf::from(p);
    if p.is_absolute() { p } else { base.join(p) }
}

fn try_set_default_venv_site_packages(workdir: &Path) {
    if env_string("CHAOS_TTS_PY_VENV_SITE_PACKAGES").is_some() {
        return;
    }

    let candidates = [
        workdir.join(".venv").join("Lib").join("site-packages"),
        repo_root()
            .join("third_party")
            .join("voicelab_py_env")
            .join(".venv")
            .join("Lib")
            .join("site-packages"),
    ];

    for c in candidates {
        if c.exists() {
            unsafe {
                std::env::set_var(
                    "CHAOS_TTS_PY_VENV_SITE_PACKAGES",
                    c.to_string_lossy().to_string(),
                )
            };
            eprintln!(
                "hint: auto set CHAOS_TTS_PY_VENV_SITE_PACKAGES={}",
                c.display()
            );
            return;
        }
    }
}

/// PyO3(Python/.pt) 兼容后端：复刻 VoiceLab 的 `tools/infer_sft.py`（直接使用 llm_ckpt/flow_ckpt）。
///
/// 运行前需要：
/// - `cargo test ... --features "live-tests tts-python"`
/// - `CHAOS_TTS_PY_WORKDIR`（指向 VoiceLab 的 workflows/cosyvoice 或本仓库 third_party/voicelab_embed/workflows/cosyvoice）
/// - `CHAOS_TTS_PY_MODEL_DIR` / `CHAOS_TTS_PY_LLM_CKPT` / `CHAOS_TTS_PY_FLOW_CKPT`
#[test]
fn infer_dream_sft_pt_pyo3_writes_wav_file() {
    #[cfg(not(feature = "tts-python"))]
    {
        eprintln!("skip: this test requires cargo feature `tts-python`");
        return;
    }

    #[cfg(feature = "tts-python")]
    {
        let workdir = resolve_workdir().expect("workdir");
        if !workdir.exists() {
            eprintln!(
                "skip: python workdir not found: {} (hint: set CHAOS_TTS_PY_WORKDIR or run tools/sync_voicelab_cosyvoice_min.ps1)",
                workdir.display()
            );
            return;
        }
        try_set_default_venv_site_packages(&workdir);

        let model_dir = env_string("CHAOS_TTS_PY_MODEL_DIR")
            .unwrap_or_else(|| "pretrained_models/Fun-CosyVoice3-0.5B-dream-sft".to_string());
        let llm_ckpt = env_string("CHAOS_TTS_PY_LLM_CKPT")
            .unwrap_or_else(|| "exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt".to_string());
        let flow_ckpt = env_string("CHAOS_TTS_PY_FLOW_CKPT")
            .unwrap_or_else(|| "exp/dream_sft/flow/torch_ddp/flow_avg.pt".to_string());

        // 目标设备上这些文件通常是大文件；若不存在则直接 skip，避免把 CI/新机器卡死在 python import/torch load 上。
        let model_dir_path = resolve_maybe_relative(&workdir, &model_dir);
        if !model_dir_path.exists() {
            eprintln!(
                "skip: python model_dir not found: {} (workdir={})",
                model_dir_path.display(),
                workdir.display()
            );
            return;
        }
        let llm_ckpt_path = resolve_maybe_relative(&workdir, &llm_ckpt);
        if !llm_ckpt_path.exists() {
            eprintln!(
                "skip: python llm_ckpt not found: {} (workdir={})",
                llm_ckpt_path.display(),
                workdir.display()
            );
            return;
        }
        let flow_ckpt_path = resolve_maybe_relative(&workdir, &flow_ckpt);
        if !flow_ckpt_path.exists() {
            eprintln!(
                "skip: python flow_ckpt not found: {} (workdir={})",
                flow_ckpt_path.display(),
                workdir.display()
            );
            return;
        }

        let out_dir = env_string("CHAOS_TTS_OUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_out_dir);
        std::fs::create_dir_all(&out_dir).expect("create out_dir");

        let text = env_string("CHAOS_TTS_TEXT").unwrap_or_else(|| {
            "看到码头就发马头，看到鸡就发欸由机，看到一男一女就发凿，看到一点那啥的就发爆了"
                .to_string()
        });
        let prompt_text = env_string("CHAOS_TTS_PROMPT_TEXT").unwrap_or_else(|| {
            "我在抖音上老刷那种，就是讲一个明星他的成长史...<|endofprompt|>".to_string()
        });
        let prompt_strategy = env_prompt_strategy();
        let guide_sep = env_string("CHAOS_TTS_GUIDE_SEP").unwrap_or_else(|| "。 ".to_string());
        let speed = env_parse::<f32>("CHAOS_TTS_SPEED").unwrap_or(1.1);
        let seed = env_parse::<u64>("CHAOS_TTS_SEED").unwrap_or(1986);
        let text_frontend = env_parse::<u8>("CHAOS_TTS_TEXT_FRONTEND")
            .map(|v| v != 0)
            .unwrap_or(true);

        let sampling = SamplingConfig {
            temperature: env_parse::<f32>("CHAOS_TTS_TEMPERATURE").unwrap_or(1.0),
            top_p: env_parse::<f32>("CHAOS_TTS_TOP_P").unwrap_or(0.75),
            top_k: env_parse::<usize>("CHAOS_TTS_TOP_K").unwrap_or(20),
            win_size: env_parse::<usize>("CHAOS_TTS_WIN_SIZE").unwrap_or(10),
            tau_r: env_parse::<f32>("CHAOS_TTS_TAU_R").unwrap_or(1.0),
        };

        let p = TtsSftParams {
            model_dir,
            spk_id: "dream".to_string(),
            text,
            prompt_text,
            prompt_strategy,
            guide_sep,
            speed,
            seed,
            sampling,
            text_frontend,
        };

        eprintln!(
            "pyo3(pt) infer: workdir={} model_dir={} llm_ckpt={} flow_ckpt={} seed={}",
            workdir.display(),
            p.model_dir,
            llm_ckpt,
            flow_ckpt,
            p.seed
        );
        eprintln!(
            "hint: 如果 python import fails，请设置 `CHAOS_TTS_PY_VENV_SITE_PACKAGES` 指向 VoiceLab 的 `.venv\\\\Lib\\\\site-packages`；若仍报 WinError 126，通常是 python 版本不匹配（例如 torch 为 cp310），需要在编译时设置 `PYO3_PYTHON`=对应 python.exe 后重新编译。"
        );

        let wav = match chaos_core::tts::python_infer::infer_sft_pt_wav_bytes_with_cancel(
            &p,
            &llm_ckpt,
            &flow_ckpt,
            Some(&workdir.to_string_lossy()),
            None,
            None,
        ) {
            Ok(w) => w,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("No module named 'torch'") {
                    eprintln!(
                        "skip: python env cannot import torch (hint: run tools/sync_voicelab_python_env.ps1, or set CHAOS_TTS_PY_VENV_SITE_PACKAGES to a venv that has torch)"
                    );
                    return;
                }
                panic!("infer_sft_pt: {e:?}");
            }
        };

        assert!(wav.wav_bytes.len() > 44, "wav bytes too small");
        assert_eq!(&wav.wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav.wav_bytes[8..12], b"WAVE");

        let out_path = out_dir.join("infer_sft_pyo3_pt.wav");
        std::fs::write(&out_path, &wav.wav_bytes).expect("write wav");
        eprintln!(
            "wrote wav: {} (duration_ms={} sample_rate={} channels={})",
            out_path.display(),
            wav.duration_ms,
            wav.sample_rate,
            wav.channels
        );
    }
}
