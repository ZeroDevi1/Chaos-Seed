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

### ORT + CUDA（可选）

如需在 Windows 上使用 ONNX Runtime 的 CUDA Execution Provider：

- 编译开启：`-p chaos-core --features onnx-ort-cuda`
- 运行时可选环境变量：
  - `CHAOS_ORT_EP=cpu|cuda|auto`（默认 `auto`；若开启 `onnx-ort-cuda` 则会优先尝试 CUDA）
  - `CHAOS_ORT_CUDA_DEVICE_ID=0`（可选）

备注：即使编译启用 CUDA，若机器缺少对应的 CUDA runtime / cuDNN DLL（或未加入 PATH），运行时也会自动回落到 CPU。

If you want to experiment with `candle-onnx`, note that some versions of `candle-onnx` require
`protoc` to be installed at build time.

