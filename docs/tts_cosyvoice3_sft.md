# CosyVoice3 SFT TTS（纯 Rust 运行时）

ChaosSeed 运行时使用 `chaos-core::tts`（tract-onnx + tokenizers + hound）从 **CosyVoice ONNX Pack（V1）** 合成 **WAV（base64）**。

## 1）离线导出（Python，VoiceLab 工作流）

在你的 CosyVoice 工作流环境中导出 ONNX pack（这是**离线工具链**，不属于运行时）：

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

导出后的目录结构应为：

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

## 2）Daemon JSON-RPC

方法（见 `chaos-proto`）：
- `tts.sft.start`
- `tts.sft.status`
- `tts.sft.cancel`

返回结果包含：
- `result.wavBase64`（WAV/PCM16 单声道）
- `result.sampleRate`, `result.durationMs`, ...

备注：为支持更大的 base64 WAV 负载，已将 daemon 的 LSP 帧大小上限提升到 **64 MiB**。

## 3）FFI（C ABI）

导出函数（JSON 入/出，UTF-8 编码）：
- `chaos_tts_sft_start_json(const char* params_json_utf8) -> char*`
- `chaos_tts_sft_status_json(const char* session_id_utf8) -> char*`
- `chaos_tts_sft_cancel_json(const char* session_id_utf8) -> char*`

参见 `chaos-ffi/docs/API.md`（api=9）。

## 4）本地测试（可选）

```bash
cd /home/nul1fi3nd/AntiGravityProjects/Chaos-Seed

CHAOS_COSYVOICE_PACK_DIR=/abs/path/to/export_packs/dream_sft_pack_v1 \
  cargo test -p chaos-core --features live-tests
```
