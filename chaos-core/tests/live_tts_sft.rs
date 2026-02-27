#![cfg(feature = "live-tests")]

use chaos_core::tts::{CosyVoiceEngine, CosyVoicePack, PromptStrategy, SamplingConfig, TtsSftParams};

#[test]
fn live_sft_produces_valid_wav_and_is_deterministic() {
    let dir = match std::env::var("CHAOS_COSYVOICE_PACK_DIR") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("[live-tests] CHAOS_COSYVOICE_PACK_DIR is not set; skipping.");
            return;
        }
    };

    let pack = CosyVoicePack::load(&dir).expect("load pack");
    let engine = CosyVoiceEngine::load(pack).expect("load engine");

    let spk_id = if engine.pack().spk2info.contains_key("dream") {
        "dream".to_string()
    } else {
        engine
            .pack()
            .spk2info
            .keys()
            .next()
            .expect("spk2info is empty")
            .to_string()
    };

    let params = TtsSftParams {
        model_dir: dir.clone(),
        spk_id,
        text: "Hello from ChaosSeed.".to_string(),
        prompt_text: "<|endofprompt|>".to_string(),
        prompt_strategy: PromptStrategy::Inject,
        guide_sep: " ".to_string(),
        speed: 1.0,
        seed: 1986,
        sampling: SamplingConfig {
            temperature: 1.0,
            top_p: 0.75,
            top_k: 20,
            win_size: 10,
            tau_r: 1.0,
        },
        text_frontend: false,
    };

    let a = engine.synthesize_wav_bytes(&params).expect("synthesize");
    let b = engine.synthesize_wav_bytes(&params).expect("synthesize again");

    assert_eq!(a.sample_rate, b.sample_rate);
    assert_eq!(a.wav_bytes, b.wav_bytes, "expected byte-identical output for same seed");
    assert!(a.duration_ms > 0);
    assert!(a.wav_bytes.len() > 44, "wav too small");
    assert_eq!(&a.wav_bytes[0..4], b"RIFF");
    assert_eq!(&a.wav_bytes[8..12], b"WAVE");
}

