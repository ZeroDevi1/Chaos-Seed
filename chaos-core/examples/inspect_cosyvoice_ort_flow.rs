//! 本地调试工具：检查 CosyVoice pack 的 flow_infer.onnx I/O，并用不同 token 长度做一次前向。

use std::path::{Path, PathBuf};

use chaos_core::tts::CosyVoicePack;

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
    let spk = pack
        .spk2info
        .get("dream")
        .or_else(|| pack.spk2info.values().next())
        .ok_or("spk2info empty")?;

    #[cfg(feature = "onnx-ort")]
    {
        use ort::session::Session;
        use ort::value::{Tensor, ValueType};

        let flow = Session::builder()?.commit_from_file(pack.path_flow_infer())?;

        eprintln!("[flow_infer] inputs={} outputs={}", flow.inputs.len(), flow.outputs.len());
        for i in &flow.inputs {
            eprintln!("  in  {} => {:?}", i.name, i.input_type);
        }
        for o in &flow.outputs {
            eprintln!("  out {} => {:?}", o.name, o.output_type);
        }

        let tok_name = pack
            .cfg
            .flow_io
            .as_ref()
            .and_then(|io| io.inputs.get(0))
            .map(|s| s.as_str())
            .unwrap_or("speech_tokens");
        let emb_name = pack
            .cfg
            .flow_io
            .as_ref()
            .and_then(|io| io.inputs.get(1))
            .map(|s| s.as_str())
            .unwrap_or("spk_embedding");
        let mel_name = pack
            .cfg
            .flow_io
            .as_ref()
            .and_then(|io| io.outputs.get(0))
            .map(|s| s.as_str())
            .unwrap_or("mel");

        for &len in &[4usize, 10, 16, 32, 135] {
            let speech_tokens: Vec<i64> = (0..len).map(|i| (i as i64) % 100).collect();
            let tok_t = Tensor::<i64>::from_array((
                vec![1usize, speech_tokens.len()],
                speech_tokens.into_boxed_slice(),
            ))?;
            let emb_t = Tensor::<f32>::from_array((
                vec![1usize, spk.embedding.len()],
                spk.embedding.clone().into_boxed_slice(),
            ))?;

            eprintln!("\n[run] len={len}");
            match flow.run(vec![(tok_name, tok_t.into_dyn()), (emb_name, emb_t.into_dyn())]) {
                Ok(mut out) => {
                    let mel_v = out.remove(mel_name).ok_or("missing mel output")?;
                    if let ValueType::Tensor { dimensions, ty, .. } = mel_v.dtype() {
                        eprintln!("  mel dtype={ty:?} shape={dimensions:?}");
                    } else {
                        eprintln!("  mel dtype={:?}", mel_v.dtype());
                    }
                }
                Err(e) => {
                    eprintln!("  error: {e}");
                }
            }
        }
    }

    #[cfg(not(feature = "onnx-ort"))]
    {
        eprintln!("This example requires feature `onnx-ort`.");
    }

    Ok(())
}

