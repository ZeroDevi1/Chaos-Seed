# CosyVoice3 SFT TTS (Pure Rust Runtime)

ChaosSeed runtime uses the `chaos-tts` crate (tract-onnx + tokenizers + hound) to synthesize **WAV(base64)** from a **CosyVoice ONNX Pack (V1)**.

## 1) Offline Export (Python, VoiceLab workflow)

Export the ONNX pack in your CosyVoice workflow environment (this is **offline tooling**, not part of runtime):

```bash
cd /home/nul1fi3nd/AntiGravityProjects/VoiceLab/workflows/cosyvoice

uv run python tools/export_onnx_pack.py \
  --model_dir pretrained_models/Fun-CosyVoice3-0.5B-dream-sft \
  --spk_id dream \
  --llm_ckpt  exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt \
  --flow_ckpt exp/dream_sft/flow/torch_ddp/flow_avg.pt \
  --out_dir   export_packs/dream_sft_pack_v1 \
  --device cpu
```

Expected output directory structure:

```
<pack_dir>/
  pack.json
  tokenizer.json
  spk2info.json
  llm_prefill.onnx
  llm_decode.onnx
  flow_infer.onnx
  hift_infer.onnx
  sha256.json
```

## 2) Daemon JSON-RPC

Methods (see `chaos-proto`):
- `tts.sft.start`
- `tts.sft.status`
- `tts.sft.cancel`

Result contains:
- `result.wavBase64` (WAV/PCM16 mono)
- `result.sampleRate`, `result.durationMs`, ...

Note: daemon LSP frame limit was bumped to **64 MiB** to allow large base64 WAV payloads.

## 3) FFI (C ABI)

Exports (JSON in/out, UTF-8):
- `chaos_tts_sft_start_json(const char* params_json_utf8) -> char*`
- `chaos_tts_sft_status_json(const char* session_id_utf8) -> char*`
- `chaos_tts_sft_cancel_json(const char* session_id_utf8) -> char*`

See `chaos-ffi/docs/API.md` (api=9).

## 4) Live Test (optional)

```bash
cd /home/nul1fi3nd/AntiGravityProjects/Chaos-Seed

CHAOS_COSYVOICE_PACK_DIR=/abs/path/to/export_packs/dream_sft_pack_v1 \
  cargo test -p chaos-tts --features live-tests
```

