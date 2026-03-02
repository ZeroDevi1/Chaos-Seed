# chaos-core TTS（CosyVoice3 SFT via PyO3 / VoiceLab）

本仓库已移除 Candle 推理路径；当前 **仅支持** 通过 **PyO3 嵌入式 Python** 运行 VoiceLab 的 `tools/infer_sft.py`，直接使用训练产物 `.pt`（`llm_ckpt/flow_ckpt`）进行推理。

## 核心接口

- `chaos_core::tts::python_infer::infer_sft_pt_wav_bytes_with_cancel(...) -> TtsWavResult`
  - 通过 `runpy.run_path()` 执行 `tools/infer_sft.py`，并以 `sys.argv` 方式传参
  - 返回 WAV bytes（PCM16，单声道；采样率从输出 wav 元信息读取）
  - 取消：无法硬中断 python 脚本，仅支持开始前/结束后检查取消标记并丢弃结果

## 运行所需环境变量（推荐）

最少需要：
- `CHAOS_TTS_PY_WORKDIR`：cosyvoice workflow 工作目录（包含 `tools/infer_sft.py`）
- `CHAOS_TTS_PY_VENV_SITE_PACKAGES`：指向包含 `torch/torchaudio/cosyvoice` 的 `site-packages`
- `CHAOS_TTS_PY_LLM_CKPT`：默认 LLM checkpoint（.pt）
- `CHAOS_TTS_PY_FLOW_CKPT`：默认 Flow checkpoint（.pt）

可选：
- `CHAOS_TTS_PY_INFER_SFT`：脚本路径（相对 workdir 或绝对路径），默认 `tools/infer_sft.py`
- `CHAOS_TTS_PY_OUT_DIR`：对齐 python 的 `--out_dir`；不设置则使用临时目录（自动清理）
- `CHAOS_TTS_PY_DEBUG=1`：打印 python 版本/路径等调试信息

## 分发（解压即用）建议目录约定

WinUI3 启动时会做 best-effort 环境注入（见 `chaos-winui3/ChaosSeed.WinUI3/Program.cs`），约定输出目录包含：
- `python/`：Python runtime（用于 `PYTHONHOME`）
- `.venv/`：venv（用于 `CHAOS_TTS_PY_VENV_SITE_PACKAGES` 和 torch DLL）
- `voicelab/workflows/cosyvoice/`：infer 脚本与（可选）权重

本仓库提供同步脚本（目录默认 gitignored，不会提交）：
- `tools/sync_voicelab_cosyvoice_min.ps1`：同步最小必需脚本/源码到 `third_party/voicelab_embed/`
- `tools/sync_voicelab_cosyvoice_dream_sft_weights.ps1`：同步 dream_sft 推理权重到 `third_party/voicelab_embed/workflows/cosyvoice/`
- `tools/sync_voicelab_python_env.ps1`：同步 python runtime + venv 到 `third_party/voicelab_py_env/`

## 测试命令（对齐 infer_sft.py）

该测试尽量复刻如下命令的参数行为：

```bash
uv run python tools/infer_sft.py \
  --model_dir pretrained_models/Fun-CosyVoice3-0.5B-dream-sft \
  --spk_id dream \
  --text "..." \
  --out_dir out_wav/dream \
  --llm_ckpt  exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt \
  --flow_ckpt exp/dream_sft/flow/torch_ddp/flow_avg.pt \
  --prompt_text "...<|endofprompt|>" \
  --prompt_strategy guide_prefix \
  --guide_sep "。 " \
  --speed 1.1 \
  --seed 1986 \
  --temperature 1.0 \
  --top_p 0.75 \
  --top_k 20 \
  --win_size 10 \
  --tau_r 1.0
```

Windows PowerShell（建议先运行同步脚本，把 voicelab + 权重 + python env 放到 third_party）：

```powershell
# （可选但强烈建议）让 PyO3 编译时绑定到 VoiceLab venv 的 python.exe（避免 ABI 不匹配）
# $env:PYO3_PYTHON = "C:\Projects\AntiGravityProjects\VoiceLab\workflows\cosyvoice\.venv\Scripts\python.exe"

$env:CHAOS_TTS_PY_WORKDIR = "$pwd\\third_party\\voicelab_embed\\workflows\\cosyvoice"
$env:CHAOS_TTS_PY_VENV_SITE_PACKAGES = "$pwd\\third_party\\voicelab_py_env\\.venv\\Lib\\site-packages"
$env:CHAOS_TTS_PY_LLM_CKPT = "exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt"
$env:CHAOS_TTS_PY_FLOW_CKPT = "exp/dream_sft/flow/torch_ddp/flow_avg.pt"

cargo test -p chaos-core --release --no-default-features --features "live-tests tts-python" --test infer_dream_sft_pack_v1 -- --nocapture
```

