//! 本地调试工具：检查 CosyVoice pack 的 LLM（llm_prefill/llm_decode）ONNX I/O 以及一次 prefill+decode 的张量形状。
//!
//! 用法（在仓库根目录）：
//! - `cargo run -p chaos-core --example inspect_cosyvoice_ort_llm`
//! - 可选：设置 `CHAOS_COSYVOICE_PACK_DIR` 指向 pack 目录；否则使用默认约定 `models/cosyvoice/pack/dream_sft_pack_v1`

use std::path::{Path, PathBuf};

use chaos_core::tts::{
    CosyVoicePack, PromptStrategy, SamplingConfig, TtsSftParams, resolve_tts_text_basic,
};

fn default_pack_dir() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;
    let p = repo_root
        .join("models")
        .join("cosyvoice")
        .join("pack")
        .join("dream_sft_pack_v1");
    if p.exists() { Some(p) } else { None }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = match std::env::var("CHAOS_COSYVOICE_PACK_DIR") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v.trim()),
        _ => default_pack_dir().ok_or("default pack dir not found; set CHAOS_COSYVOICE_PACK_DIR")?,
    };

    let pack = CosyVoicePack::load(&dir)?;

    // 生成一段与测试相同的输入，确保 token id 在词表范围内。
    let params = TtsSftParams {
        model_dir: dir.to_string_lossy().to_string(),
        spk_id: "dream".to_string(),
        text: "看到码头就发马头，看到鸡就发欸由机，看到一男一女就发凿，看到一点那啥的就发爆了".to_string(),
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
        text_frontend: true,
    };

    let mut input_ids: Vec<i64> = Vec::new();
    if std::env::var("CHAOS_ORT_TINY_INPUT").ok().as_deref() == Some("1") {
        // 仅用于定位 decode 是否“与 past_len 无关地必崩”。
        input_ids.push(pack.cfg.end_of_prompt_token_id as i64);
    } else {
        let resolved = resolve_tts_text_basic(
            &params.text,
            &params.prompt_text,
            params.prompt_strategy,
            &params.guide_sep,
            params.text_frontend,
        )?;

        let add_special = pack.cfg.tokenizer_add_special_tokens;
        let enc_prompt = pack.tokenizer.encode(resolved.prompt_inject_text, add_special)?;
        let enc_text = pack.tokenizer.encode(resolved.spoken_text, add_special)?;
        input_ids.extend(enc_prompt.get_ids().iter().map(|&x| x as i64));
        input_ids.extend(enc_text.get_ids().iter().map(|&x| x as i64));
    }

    // 可选：为了定位“长度阈值”问题，允许在示例里截断输入长度（保留末尾 N 个 token）。
    if let Ok(v) = std::env::var("CHAOS_ORT_MAX_INPUT_LEN") {
        let max_len: usize = v.trim().parse().unwrap_or(0);
        if max_len > 0 && input_ids.len() > max_len {
            let drop = input_ids.len() - max_len;
            input_ids.drain(0..drop);
            eprintln!("[debug] truncated input_ids to len={max_len}");
        }
    }

    // ---- ORT: load sessions ----
    #[cfg(feature = "onnx-ort")]
    {
        use ort::session::Session;
        use ort::value::{Tensor, ValueType};

        let prefill = Session::builder()?.commit_from_file(pack.path_llm_prefill())?;
        let decode = Session::builder()?.commit_from_file(pack.path_llm_decode())?;

        eprintln!("[llm_prefill] inputs={} outputs={}", prefill.inputs.len(), prefill.outputs.len());
        for i in &prefill.inputs {
            eprintln!("  in  {} => {:?}", i.name, i.input_type);
        }
        for o in prefill.outputs.iter().take(6) {
            eprintln!("  out {} => {:?}", o.name, o.output_type);
        }
        if prefill.outputs.len() > 6 {
            eprintln!("  ... ({} more outputs)", prefill.outputs.len() - 6);
        }

        eprintln!(
            "[llm_decode] inputs={} outputs={}",
            decode.inputs.len(),
            decode.outputs.len()
        );
        let inits = decode.overridable_initializers();
        if !inits.is_empty() {
            eprintln!("[llm_decode] overridable_initializers={}", inits.len());
            for x in inits.iter().take(8) {
                eprintln!("  init {} => {:?}", x.name(), x.dtype());
            }
            if inits.len() > 8 {
                eprintln!("  ... ({} more initializers)", inits.len() - 8);
            }
        }
        for i in decode.inputs.iter().take(12) {
            eprintln!("  in  {} => {:?}", i.name, i.input_type);
        }
        if decode.inputs.len() > 12 {
            eprintln!("  ... ({} more inputs)", decode.inputs.len() - 12);
        }
        for o in decode.outputs.iter().take(6) {
            eprintln!("  out {} => {:?}", o.name, o.output_type);
        }
        if decode.outputs.len() > 6 {
            eprintln!("  ... ({} more outputs)", decode.outputs.len() - 6);
        }

        // ---- prefill ----
        let input_name = pack
            .cfg
            .llm
            .prefill_io
            .as_ref()
            .and_then(|io| io.inputs.get(0))
            .map(|s| s.as_str())
            .unwrap_or("input_ids");

        let logits_name = pack
            .cfg
            .llm
            .prefill_io
            .as_ref()
            .and_then(|io| io.outputs.get(0))
            .map(|s| s.as_str())
            .unwrap_or("logits");

        let input = Tensor::<i64>::from_array((
            vec![1usize, input_ids.len()],
            input_ids.clone().into_boxed_slice(),
        ))?;

        let mut prefill_out = prefill.run(vec![(input_name, input.into_dyn())])?;

        let logits_v = prefill_out
            .remove(logits_name)
            .ok_or("prefill missing logits")?;
        match logits_v.dtype() {
            ValueType::Tensor { dimensions, ty, .. } => {
                eprintln!("[prefill] logits dtype={ty:?} shape={dimensions:?}");
            }
            t => eprintln!("[prefill] logits dtype={t:?}"),
        }

        // 打印一层 KV cache 的形状，便于判断轴顺序（heads/seq）。
        let past0k = prefill_out
            .get("past_0_key")
            .ok_or("prefill missing past_0_key")?;
        let past0v = prefill_out
            .get("past_0_value")
            .ok_or("prefill missing past_0_value")?;
        if let ValueType::Tensor { dimensions, ty, .. } = past0k.dtype() {
            eprintln!("[prefill] past_0_key dtype={ty:?} shape={dimensions:?}");
        }
        if let ValueType::Tensor { dimensions, ty, .. } = past0v.dtype() {
            eprintln!("[prefill] past_0_value dtype={ty:?} shape={dimensions:?}");
        }

        // ---- decode (single step) ----
        let token_name = pack
            .cfg
            .llm
            .decode_io
            .as_ref()
            .and_then(|io| io.inputs.get(0))
            .map(|s| s.as_str())
            .unwrap_or("token_id");
        let decode_logits_name = pack
            .cfg
            .llm
            .decode_io
            .as_ref()
            .and_then(|io| io.outputs.get(0))
            .map(|s| s.as_str())
            .unwrap_or("logits");

        let token_id = 0i64;
        let token_t = Tensor::<i64>::from_array((vec![1usize, 1usize], vec![token_id].into_boxed_slice()))?;

        let mut inputs: Vec<(&str, ort::value::DynValue)> = Vec::new();
        inputs.push((token_name, token_t.into_dyn()));

        // 注意：这里按 pack.json 的 decodeIo.inputs 顺序塞入 past_*，方便复现实测逻辑。
        let copy_past = std::env::var("CHAOS_ORT_COPY_PAST").ok().as_deref() == Some("1");
        let clamp_past: Option<usize> = std::env::var("CHAOS_ORT_CLAMP_PAST")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .filter(|&v| v > 0);
        if let Some(io) = pack.cfg.llm.decode_io.as_ref() {
            for past_in in io.inputs.iter().skip(1) {
                let v = prefill_out
                    .remove(past_in.as_str())
                    .ok_or("prefill missing past for decode")?;
                if copy_past || clamp_past.is_some() {
                    let (shape, data) = v.try_extract_raw_tensor::<f32>()?;
                    let b = shape.get(0).copied().unwrap_or(0).max(0) as usize;
                    let h = shape.get(1).copied().unwrap_or(0).max(0) as usize;
                    let s = shape.get(2).copied().unwrap_or(0).max(0) as usize;
                    let d = shape.get(3).copied().unwrap_or(0).max(0) as usize;
                    let keep = clamp_past.unwrap_or(s).min(s);
                    let start = s.saturating_sub(keep);

                    // 仅支持 [B,H,S,D] 的 KV cache。
                    let mut out = vec![0.0f32; b * h * keep * d];
                    for bb in 0..b {
                        for hh in 0..h {
                            for ss in 0..keep {
                                let src_s = start + ss;
                                let src_base = (((bb * h + hh) * s + src_s) * d) as usize;
                                let dst_base = (((bb * h + hh) * keep + ss) * d) as usize;
                                out[dst_base..dst_base + d]
                                    .copy_from_slice(&data[src_base..src_base + d]);
                            }
                        }
                    }

                    let t = Tensor::<f32>::from_array((
                        vec![b, h, keep, d],
                        out.into_boxed_slice(),
                    ))?;
                    inputs.push((past_in.as_str(), t.into_dyn()));
                } else {
                    inputs.push((past_in.as_str(), v));
                }
            }
        }

        let mut decode_out = decode.run(inputs)?;
        let v = decode_out.remove(decode_logits_name).ok_or("decode missing logits")?;
        if let ValueType::Tensor { dimensions, ty, .. } = v.dtype() {
            eprintln!("[decode] logits dtype={ty:?} shape={dimensions:?}");
        }

        let present0k = decode_out.get("present_0_key").ok_or("decode missing present_0_key")?;
        if let ValueType::Tensor { dimensions, ty, .. } = present0k.dtype() {
            eprintln!("[decode] present_0_key dtype={ty:?} shape={dimensions:?}");
        }

        eprintln!("decode ok");
    }

    #[cfg(not(feature = "onnx-ort"))]
    {
        eprintln!("This example requires feature `onnx-ort`.");
    }

    Ok(())
}
