#![cfg(feature = "live-tests")]

use std::path::{Path, PathBuf};

use chaos_core::tts::{PromptStrategy, SamplingConfig};

#[cfg(feature = "cosyvoice3-candle")]
use chaos_core::tts::{CosyVoice3CandleEngine, CosyVoice3CandleParams, CosyVoice3Mode};

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

fn default_candle_model_dir() -> PathBuf {
    // 与 tools/run_infer_dream_sft_pack_v1_candle_cuda.ps1 保持一致
    repo_root()
        .join("models")
        .join("cosyvoice3_candle")
        .join("dream_sft_epoch5")
}

fn env_flag(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        })
        .unwrap_or(false)
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_string(key).map(PathBuf::from)
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

/// 对齐 VoiceLab 的 Python 推理命令（`tools/infer_sft.py`）里最常用的参数集（SFT + guide_prefix）：
///
/// ```bash
/// uv run python tools/infer_sft.py \
///   --model_dir pretrained_models/Fun-CosyVoice3-0.5B-dream-sft \
///   --spk_id dream \
///   --text "..." \
///   --out_dir out_wav/dream \
///   --prompt_text "...<|endofprompt|>" \
///   --prompt_strategy guide_prefix \
///   --guide_sep "。 " \
///   --speed 1.1 \
///   --seed 1986 \
///   --temperature 1.0 --top_p 0.75 --top_k 20 --win_size 10 --tau_r 1.0
/// ```
///
/// 说明：
/// - 本测试 **不再使用 ONNX pack 推理**，只跑 cosyvoice3-candle（llm/flow/hift safetensors）。
/// - `infer_sft.py` 的 `--llm_ckpt/--flow_ckpt` 是 torch checkpoint；Rust 侧需要你先用转换脚本得到 safetensors 模型目录。
/// - SFT `spk_id` 需要 `spk2info.json`（可用 `tools/export_spk2info_json_from_pt.py` 从 VoiceLab 的 `spk2info.pt` 导出）。
/// - 本测试不使用 prompt_wav（也就不需要 ONNX frontend）；完全走 spk2info.json 的说话人 embedding 路线。
/// - `seed/win_size/tau_r` 属于 python 的 RAS 采样参数；当前 cosyvoice3-candle LLM 采样接口不支持这些参数（会读取但不生效）。
#[test]
fn infer_dream_sft_pack_v1_writes_wav_file() {
    #[cfg(not(feature = "cosyvoice3-candle"))]
    {
        eprintln!("skip: this test requires cargo feature `cosyvoice3-candle`");
        return;
    }

    #[cfg(feature = "cosyvoice3-candle")]
    {
        // 1) cosyvoice3-candle model dir（safetensors）
        let model_dir =
            env_path("CHAOS_COSYVOICE3_CANDLE_MODEL_DIR").unwrap_or_else(default_candle_model_dir);
        if !model_dir.exists() {
            eprintln!(
                "skip: CHAOS_COSYVOICE3_CANDLE_MODEL_DIR not found: {}",
                model_dir.display()
            );
            return;
        }

        // 2) 输出目录（对齐 python 的 --out_dir）
        let out_dir = env_path("CHAOS_TTS_OUT_DIR").unwrap_or_else(default_out_dir);
        std::fs::create_dir_all(&out_dir).expect("create out_dir");

        // 3) 说话人（SFT）
        let spk_id = env_string("CHAOS_COSYVOICE3_SPK_ID")
            .or_else(|| env_string("CHAOS_TTS_SPK_ID"))
            .unwrap_or_else(|| "dream".to_string());
        let spk2info_json = env_path("CHAOS_COSYVOICE3_SPK2INFO_JSON");
        let has_spk2info = if let Some(p) = spk2info_json.as_ref() {
            p.exists()
        } else {
            model_dir.join("spk2info.json").exists()
        };
        if !has_spk2info {
            eprintln!(
                "skip: spk2info.json not found (required for spk_id route). Hint: run tools/export_spk2info_json_from_pt.py"
            );
            return;
        }

        // 4) 文本/采样参数（尽量对齐 python 的命令行）
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
        let text_frontend = if std::env::var_os("CHAOS_TTS_TEXT_FRONTEND").is_some() {
            env_flag("CHAOS_TTS_TEXT_FRONTEND")
        } else {
            true
        };

        let greedy = env_flag("CHAOS_TTS_GREEDY");
        let mut sampling = SamplingConfig {
            temperature: env_parse::<f32>("CHAOS_TTS_TEMPERATURE").unwrap_or(1.0),
            top_p: env_parse::<f32>("CHAOS_TTS_TOP_P").unwrap_or(0.75),
            top_k: env_parse::<usize>("CHAOS_TTS_TOP_K").unwrap_or(20),
            win_size: env_parse::<usize>("CHAOS_TTS_WIN_SIZE").unwrap_or(10),
            tau_r: env_parse::<f32>("CHAOS_TTS_TAU_R").unwrap_or(1.0),
        };
        if greedy {
            sampling.top_k = 1;
            sampling.top_p = 1.0;
        }

        // cosyvoice3-candle 的 mode：默认 instruct（更贴近 SFT 的“prompt_text 引导风格”用法）
        let mode = env_string("CHAOS_COSYVOICE3_MODE")
            .unwrap_or_else(|| "instruct".to_string())
            .to_ascii_lowercase();
        let mode = match mode.as_str() {
            "zero_shot" | "zeroshot" => CosyVoice3Mode::ZeroShot,
            "cross_lingual" | "crosslingual" => CosyVoice3Mode::CrossLingual,
            _ => CosyVoice3Mode::Instruct,
        };

        eprintln!(
            "cosyvoice3-candle: spk_id={} mode={:?} prompt_strategy={} speed={} seed={} sampling={:?}",
            spk_id,
            mode,
            prompt_strategy.as_str(),
            speed,
            seed,
            sampling
        );
        if !env_flag("CHAOS_COSYVOICE3_DEBUG") {
            eprintln!("hint: set CHAOS_COSYVOICE3_DEBUG=1 for more logs");
        }
        eprintln!(
            "note: cosyvoice3-candle does not currently use seed/win_size/tau_r (python-only RAS params); shown here for alignment"
        );

        // 6) 推理
        let engine =
            CosyVoice3CandleEngine::load(&model_dir).expect("load cosyvoice3 candle engine");
        let params = CosyVoice3CandleParams {
            model_dir: model_dir.to_string_lossy().to_string(),
            mode,
            text,
            prompt_text,
            spk_id: Some(spk_id),
            spk2info_json,
            prompt_wav: None,
            prompt_strategy,
            guide_sep,
            text_frontend,
            sampling,
            n_timesteps: 10,
            speed,
        };
        let r = engine
            .synthesize_wav_bytes_debug(&params)
            .expect("synthesize candle");

        assert!(r.wav.wav_bytes.len() > 44, "wav bytes too small");
        assert_eq!(&r.wav.wav_bytes[0..4], b"RIFF");
        assert_eq!(&r.wav.wav_bytes[8..12], b"WAVE");

        let out_path = out_dir.join("infer_sft_candle.wav");
        std::fs::write(&out_path, &r.wav.wav_bytes).expect("write wav");
        eprintln!(
            "wrote wav: {} (duration_ms={} sample_rate={} speech_tokens_len={})",
            out_path.display(),
            r.wav.duration_ms,
            r.wav.sample_rate,
            r.speech_tokens.len()
        );
    }
}
