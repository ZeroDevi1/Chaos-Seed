#![cfg(feature = "live-tests")]

use std::path::{Path, PathBuf};

use chaos_core::tts::{
    CosyVoiceEngine, CosyVoicePack, PromptStrategy, SamplingConfig, TtsSftParams,
};

fn default_pack_dir() -> Option<PathBuf> {
    // 默认约定：模型 pack 放在 repo 根目录 models/cosyvoice/pack/dream_sft_pack_v1（不进 git）
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;
    let p = repo_root
        .join("models")
        .join("cosyvoice")
        .join("pack")
        .join("dream_sft_pack_v1");
    if p.exists() { Some(p) } else { None }
}

fn default_out_dir() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;
    Some(repo_root.join("out_wav").join("dream"))
}

/// 对齐 VoiceLab 的 Python 推理命令（tools/infer_sft.py）：
/// - 使用导出的 ONNX pack（dream_sft_pack_v1）
/// - 生成 WAV，并落盘到 out_wav/dream
///
/// 说明：
/// - 该测试用于本地手工验证，默认不在 CI 跑（需要模型文件且推理耗时）
/// - 如需自定义 pack 目录，设置环境变量 `CHAOS_COSYVOICE_PACK_DIR`
/// - 如需自定义输出目录，设置环境变量 `CHAOS_TTS_OUT_DIR`
/// - 如需输出更详尽的张量/生成日志到文件，设置环境变量：
///   - `CHAOS_COSYVOICE_DEBUG_LOG=out_wav/dream/cosyvoice_debug.log`
///   - `CHAOS_COSYVOICE_DEBUG_LOG_TRUNCATE=1`（可选，覆盖写）
///   - `CHAOS_COSYVOICE_DEBUG_LOG_EVERY=20`（可选，控制解码过程日志频率）
/// - 如推理极慢，建议先确认 ORT 是否真的启用 CUDA：`CHAOS_ORT_EP_DEBUG=1`
#[test]
fn infer_dream_sft_pack_v1_writes_wav_file() {
    let dir = match std::env::var("CHAOS_COSYVOICE_PACK_DIR") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v.trim()),
        _ => match default_pack_dir() {
            Some(p) => p,
            None => {
                eprintln!(
                    "skip: CHAOS_COSYVOICE_PACK_DIR is not set, and default pack dir does not exist"
                );
                return;
            }
        },
    };

    let out_dir = match std::env::var("CHAOS_TTS_OUT_DIR") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v.trim()),
        _ => default_out_dir().expect("repo root"),
    };

    // 该 pack 必须包含 flow/vocoder ONNX；否则只能跑到 LLM，无法生成音频。
    let required = [
        "pack.json",
        "tokenizer.json",
        "spk2info.json",
        "llm_prefill.onnx",
        "llm_decode.onnx",
        "flow_infer.onnx",
        "hift_infer.onnx",
    ];
    let mut missing: Vec<String> = Vec::new();
    for f in required {
        if !dir.join(f).exists() {
            missing.push(f.to_string());
        }
    }
    if !missing.is_empty() {
        eprintln!(
            "skip: pack is incomplete (missing: {}). Please re-export the ONNX pack with flow+hift models.",
            missing.join(", ")
        );
        return;
    }

    let pack = CosyVoicePack::load(&dir).expect("load pack");
    let engine = CosyVoiceEngine::load(pack).expect("load engine");

    // Python 示例使用 dream；如果 pack 里没有，就退化到第一个 spkId，方便复用其他 pack 做 sanity check。
    let spk_id = if engine.pack().spk2info.contains_key("dream") {
        "dream".to_string()
    } else {
        engine
            .pack()
            .spk2info
            .keys()
            .next()
            .expect("spk2info not empty")
            .to_string()
    };

    let mut params = TtsSftParams {
        model_dir: dir.to_string_lossy().to_string(),
        spk_id,
        text: "看到码头就发马头，看到鸡就发欸由机，看到一男一女就发凿，看到一点那啥的就发爆了"
            .to_string(),
        prompt_text: "我在抖音上老刷那种，就是讲一个明星他的成长史...<|endofprompt|>".to_string(),
        prompt_strategy: PromptStrategy::GuidePrefix,
        guide_sep: "。 ".to_string(),
        speed: 1.1,
        seed: 1986,
        sampling: SamplingConfig {
            temperature: 1.0,
            top_p: 0.75,
            top_k: 20,
            win_size: 10,
            tau_r: 1.0,
        },
        // WinUI 默认开启（与页面 toggle 对齐）；如需更贴近上游 python，可自行对比开关差异。
        text_frontend: true,
    };

    // Greedy 对齐：用于和 Python 侧做 token 对比（规避 RNG 差异）。
    // 约定：设置 `CHAOS_TTS_GREEDY=1` 时强制 `top_k=1, top_p=1.0`。
    let greedy = std::env::var("CHAOS_TTS_GREEDY")
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        })
        .unwrap_or(false);
    if greedy {
        params.sampling.top_k = 1;
        params.sampling.top_p = 1.0;
        eprintln!("CHAOS_TTS_GREEDY=1 => force sampling: {:?}", params.sampling);
    } else {
        eprintln!("sampling: {:?}", params.sampling);
    }

    // 调试版：一次性拿到 wav + speech_tokens + logits vocab（避免重复跑 LLM）。
    let r = engine
        .synthesize_wav_bytes_debug(&params)
        .expect("synthesize");

    eprintln!(
        "Rust speech_tokens[0..20] = {:?}",
        &r.speech_tokens[0..20.min(r.speech_tokens.len())]
    );
    eprintln!("Rust logits shape = {:?}", r.llm_logits_vocab_size);

    assert!(r.wav.wav_bytes.len() > 44, "wav bytes too small");
    assert_eq!(&r.wav.wav_bytes[0..4], b"RIFF");
    assert_eq!(&r.wav.wav_bytes[8..12], b"WAVE");

    std::fs::create_dir_all(&out_dir).expect("create out_dir");
    let out_path = out_dir.join("dream_sft_pack_v1_rust.wav");
    std::fs::write(&out_path, &r.wav.wav_bytes).expect("write wav");
    eprintln!("wrote wav: {}", out_path.display());
}
