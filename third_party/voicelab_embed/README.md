# VoiceLab (CosyVoice workflow) - Local Embed (gitignored)

此目录用于**本地**嵌入 VoiceLab 的 CosyVoice workflow 代码，方便 Chaos-Seed 的 PyO3(Python) 推理后端
在没有外部 VoiceLab checkout 的情况下也能运行 `tools/infer_sft.py`。

约定：
- 此目录下的实际内容默认被 `.gitignore` 排除，不会被提交到 git。
- 需要时通过仓库脚本把外部 VoiceLab 的最小必需内容同步到这里。

同步脚本：
- `tools/sync_voicelab_cosyvoice_min.ps1`
- （可选）`tools/sync_voicelab_cosyvoice_dream_sft_weights.ps1`（同步 dream_sft 推理权重：`pretrained_models/` + `exp/`，大文件）

同步后典型结构：
```
third_party/voicelab_embed/
  workflows/cosyvoice/
    tools/infer_sft.py
    tools/voicelab_bootstrap.py
    voicelab_cosyvoice/
  vendor/CosyVoice/
    cosyvoice/
    third_party/Matcha-TTS/
```

运行 PyO3(Python) 后端时建议设置：
- `CHAOS_TTS_PY_WORKDIR=<repo>/third_party/voicelab_embed/workflows/cosyvoice`
