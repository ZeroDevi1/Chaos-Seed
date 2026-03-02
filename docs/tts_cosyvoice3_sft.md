# CosyVoice3 SFT TTS（PyO3 / VoiceLab infer_sft.py）

本仓库已移除 Candle 推理路径；当前 **仅支持** 通过 **PyO3 嵌入式 Python** 执行 VoiceLab 的 `tools/infer_sft.py`，直接使用 `.pt` checkpoint（`llm_ckpt/flow_ckpt`）完成推理。

## 1) 对外接口（daemon / FFI）

方法（见 `chaos-proto`）：
- `tts.sft.start`
- `tts.sft.status`
- `tts.sft.cancel`

返回结果包含：
- `result.wavBase64`（WAV/PCM16 单声道）
- `result.sampleRate`, `result.durationMs`, ...

## 2) 运行时必须准备什么

至少需要：
1) VoiceLab cosyvoice workflow（包含 `tools/infer_sft.py` 及 vendor python 源码）
2) Python runtime + venv（能 import `torch/torchaudio/cosyvoice`）
3) 训练产物权重：`model_dir` + `llm_ckpt` + `flow_ckpt`

对应环境变量（推荐由启动进程设置，便于“解压即用”）：
- `CHAOS_TTS_PY_WORKDIR`：workdir（包含 `tools/infer_sft.py`）
- `CHAOS_TTS_PY_INFER_SFT`：脚本路径（默认 `tools/infer_sft.py`）
- `CHAOS_TTS_PY_VENV_SITE_PACKAGES`：venv 的 `site-packages`（必须包含 torch）
- `CHAOS_TTS_PY_MODEL_DIR`：默认 `--model_dir`（相对 workdir 或绝对路径）
- `CHAOS_TTS_PY_LLM_CKPT`：默认 `--llm_ckpt`
- `CHAOS_TTS_PY_FLOW_CKPT`：默认 `--flow_ckpt`

可选：
- `CHAOS_TTS_PY_OUT_DIR`：对齐 python 的 `--out_dir`（不设置则使用临时目录）
- `CHAOS_TTS_PY_DEBUG=1`：打印 python 版本/路径等调试信息

## 3) 为分发准备：把 VoiceLab + 权重 + Python 环境同步进仓库（gitignored）

说明：以下 `third_party/*` 目录默认 `gitignore`，只用于本地分发/打包，不会提交到 git。

同步最小必需脚本/源码：

```powershell
pwsh -NoLogo -NoProfile -File tools/sync_voicelab_cosyvoice_min.ps1
```

同步 dream_sft 推理权重（大文件）：

```powershell
pwsh -NoLogo -NoProfile -File tools/sync_voicelab_cosyvoice_dream_sft_weights.ps1
```

同步 python runtime + venv（大文件）：

```powershell
pwsh -NoLogo -NoProfile -File tools/sync_voicelab_python_env.ps1
```

## 4) WinUI3：构建后自动拷贝到输出目录（解压即用）

`ChaosSeed.WinUI3.csproj` 在 `AfterTargets=Build` 中做了 best-effort 复制：
- `third_party/voicelab_embed/workflows/cosyvoice/**` → `$(OutDir)/voicelab/workflows/cosyvoice/**`
- `third_party/voicelab_py_env/python/**` → `$(OutDir)/python/**`
- `third_party/voicelab_py_env/.venv/**` → `$(OutDir)/.venv/**`

WinUI3 进程启动时（`Program.cs`）也会 best-effort 注入：
- `PYTHONHOME/PYTHONPATH/PATH`（确保 python 标准库与 `python310.dll` 可加载）
- `CHAOS_TTS_PY_*` 的默认值（指向打包内置的 dream_sft 权重）

因此：只要同步目录存在，Release 构建后的 zip 解压即可用（无需外部 VoiceLab checkout）。

## 5) CUDA 自动切换（Python 侧）

是否走 GPU 推理由 python 环境决定：
- 安装/同步的是 **CUDA 版 torch** 且机器有 NVIDIA GPU + 对应 CUDA runtime → 通常会自动走 CUDA
- 否则回落 CPU

本仓库不会在 Rust 侧强行指定 `--device`（以保持对 VoiceLab `infer_sft.py` 行为的黑盒复刻）。

## 6) 本地测试（复刻 infer_sft.py 命令行）

该测试尽量复刻如下命令的参数行为：

```bash
uv run python tools/infer_sft.py \
  --model_dir pretrained_models/Fun-CosyVoice3-0.5B-dream-sft \
  --spk_id dream \
  --text \"...\" \
  --out_dir out_wav/dream \
  --llm_ckpt  exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt \
  --flow_ckpt exp/dream_sft/flow/torch_ddp/flow_avg.pt \
  --prompt_text \"...<|endofprompt|>\" \
  --prompt_strategy guide_prefix \
  --guide_sep \"。 \" \
  --speed 1.1 \
  --seed 1986 \
  --temperature 1.0 \
  --top_p 0.75 \
  --top_k 20 \
  --win_size 10 \
  --tau_r 1.0
```

运行（Windows PowerShell）：

```powershell
$env:CHAOS_TTS_PY_WORKDIR = \"$pwd\\third_party\\voicelab_embed\\workflows\\cosyvoice\"
$env:CHAOS_TTS_PY_VENV_SITE_PACKAGES = \"$pwd\\third_party\\voicelab_py_env\\.venv\\Lib\\site-packages\"
$env:CHAOS_TTS_PY_LLM_CKPT = \"exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt\"
$env:CHAOS_TTS_PY_FLOW_CKPT = \"exp/dream_sft/flow/torch_ddp/flow_avg.pt\"

cargo test -p chaos-core --release --no-default-features --features \"live-tests tts-python\" --test infer_dream_sft_pack_v1 -- --nocapture
```

