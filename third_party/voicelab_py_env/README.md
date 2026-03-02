# voicelab_py_env

本目录用于 **WinUI3 zip 分发**时的“解压即用”Python 环境（默认不进 git）。

约定目录结构（由同步脚本生成）：

```
third_party/voicelab_py_env/
  python/                 # Python runtime（用于 PYTHONHOME）
    python310.dll
    Lib/
    DLLs/
    ...
  .venv/                  # VoiceLab cosyvoice 的 venv（用于 site-packages + Library/bin）
    Lib/site-packages/
    Library/bin/
    Scripts/
    ...
```

同步脚本：
- `tools/sync_voicelab_python_env.ps1`

WinUI3 构建后会（若存在该目录）自动把它们复制到输出目录：
- `$(OutDir)python/`
- `$(OutDir).venv/`

