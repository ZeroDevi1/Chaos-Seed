# chaos-core TTS (CosyVoice3 pack)

This module provides a pure-Rust runtime for synthesizing speech from a **CosyVoice3 ONNX pack**.

## Model directory layout (pack)

The engine expects a directory containing:

- `pack.json`
- `tokenizer.json`
- `spk2info.json`
- `llm_prefill.onnx`
- `llm_decode.onnx`
- `flow_infer.onnx`
- `hift_infer.onnx`

The exact filenames can be overridden via `pack.json`'s `files` section.

## Core API

- `chaos_core::tts::CosyVoicePack::load(model_dir)`
- `chaos_core::tts::CosyVoiceEngine::load(pack)`
- `engine.synthesize_pcm16(&TtsSftParams)` → `TtsPcm16Result`
- `engine.synthesize_wav_bytes(&TtsSftParams)` → `TtsWavResult`
- `chaos_core::tts::trim_output_pcm16(pcm16, sample_rate, &TrimConfig)` → trimmed PCM

## Notes on ONNX backend

V1 uses the `tract-onnx` backend by default for higher ONNX operator coverage and a smoother build
experience (no external build tools).

If you want to experiment with `candle-onnx`, note that some versions of `candle-onnx` require
`protoc` to be installed at build time.

