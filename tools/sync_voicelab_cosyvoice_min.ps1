#!/usr/bin/env pwsh
# 同步 VoiceLab/workflows/cosyvoice 的最小必需内容到 Chaos-Seed/third_party/voicelab_embed（用于 PyO3 推理后端）。
# 说明：
# - 该目录默认 gitignored，只用于本地分发/打包。
# - 仅复制脚本 + vendor CosyVoice 源码（不复制 .venv / exp / pretrained_models 等大文件）。

param(
  [Parameter(Mandatory = $false)]
  [string]$VoiceLabRoot = "C:\Projects\AntiGravityProjects\VoiceLab"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Invoke-RoboCopy {
  param(
    [Parameter(Mandatory = $true)][string]$Src,
    [Parameter(Mandatory = $true)][string]$Dst
  )

  New-Item -ItemType Directory -Force -Path $Dst | Out-Null

  # /MIR: mirror directory tree
  # /R:2 /W:1: fail fast on locked files
  # /NFL /NDL /NJH /NJS: reduce noise
  & robocopy $Src $Dst /MIR /R:2 /W:1 /XD "__pycache__" ".git" /XF "*.pyc" /NFL /NDL /NJH /NJS | Out-Null
  $code = $LASTEXITCODE
  # robocopy: 0..7 are success (incl. "copied some files"); >=8 are failures.
  if ($code -ge 8) {
    throw "robocopy failed (exit=$code): $Src -> $Dst"
  }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$destRoot = Join-Path $repoRoot "third_party\voicelab_embed"

$VoiceLabRoot = (Resolve-Path $VoiceLabRoot).Path
$srcWorkflow = Join-Path $VoiceLabRoot "workflows\cosyvoice"
$srcVendorCosy = Join-Path $VoiceLabRoot "vendor\CosyVoice"

if (!(Test-Path -LiteralPath $srcWorkflow)) {
  throw "VoiceLab workflow not found: $srcWorkflow"
}
if (!(Test-Path -LiteralPath $srcVendorCosy)) {
  throw "VoiceLab vendor/CosyVoice not found: $srcVendorCosy"
}

$dstWorkflow = Join-Path $destRoot "workflows\cosyvoice"
$dstTools = Join-Path $dstWorkflow "tools"
$dstVendorCosy = Join-Path $destRoot "vendor\CosyVoice"

New-Item -ItemType Directory -Force -Path $dstTools | Out-Null

# tools
Copy-Item -Force -LiteralPath (Join-Path $srcWorkflow "tools\infer_sft.py") -Destination (Join-Path $dstTools "infer_sft.py")
Copy-Item -Force -LiteralPath (Join-Path $srcWorkflow "tools\voicelab_bootstrap.py") -Destination (Join-Path $dstTools "voicelab_bootstrap.py")

# workflow patches
Invoke-RoboCopy -Src (Join-Path $srcWorkflow "voicelab_cosyvoice") -Dst (Join-Path $dstWorkflow "voicelab_cosyvoice")

# vendor python sources (minimal)
Invoke-RoboCopy -Src (Join-Path $srcVendorCosy "cosyvoice") -Dst (Join-Path $dstVendorCosy "cosyvoice")
Invoke-RoboCopy -Src (Join-Path $srcVendorCosy "third_party\Matcha-TTS") -Dst (Join-Path $dstVendorCosy "third_party\Matcha-TTS")

# optional: keep requirements.txt for reference
$req = Join-Path $srcVendorCosy "requirements.txt"
if (Test-Path -LiteralPath $req) {
  New-Item -ItemType Directory -Force -Path $dstVendorCosy | Out-Null
  Copy-Item -Force -LiteralPath $req -Destination (Join-Path $dstVendorCosy "requirements.txt")
}

Write-Host "OK: synced into $destRoot"
Write-Host "Hint: set CHAOS_TTS_PY_WORKDIR to: $destRoot\\workflows\\cosyvoice"

