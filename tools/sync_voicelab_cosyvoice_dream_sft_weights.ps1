#!/usr/bin/env pwsh
# 将 VoiceLab/workflows/cosyvoice 的“dream_sft 推理必需权重”同步到 Chaos-Seed/third_party/voicelab_embed（用于本地分发/离线跑 PyO3 后端）。
#
# 说明：
# - 该目录默认 gitignored，不会被提交到 git。
# - 该同步会复制大文件（pretrained_models + pt ckpt），请确保磁盘空间充足。

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

  & robocopy $Src $Dst /MIR /R:2 /W:1 /XD "__pycache__" ".git" /XF "*.pyc" /NFL /NDL /NJH /NJS | Out-Null
  $code = $LASTEXITCODE
  if ($code -ge 8) {
    throw "robocopy failed (exit=$code): $Src -> $Dst"
  }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$destRoot = Join-Path $repoRoot "third_party\voicelab_embed\workflows\cosyvoice"

$VoiceLabRoot = (Resolve-Path $VoiceLabRoot).Path
$srcRoot = Join-Path $VoiceLabRoot "workflows\cosyvoice"

$srcModelDir = Join-Path $srcRoot "pretrained_models\Fun-CosyVoice3-0.5B-dream-sft"
$srcLlm = Join-Path $srcRoot "exp\dream_sft\llm\torch_ddp\epoch_5_whole.pt"
$srcFlow = Join-Path $srcRoot "exp\dream_sft\flow\torch_ddp\flow_avg.pt"

if (!(Test-Path -LiteralPath $srcModelDir)) {
  throw "model_dir not found: $srcModelDir"
}
if (!(Test-Path -LiteralPath $srcLlm)) {
  throw "llm_ckpt not found: $srcLlm"
}
if (!(Test-Path -LiteralPath $srcFlow)) {
  throw "flow_ckpt not found: $srcFlow"
}

$dstModelDir = Join-Path $destRoot "pretrained_models\Fun-CosyVoice3-0.5B-dream-sft"
$dstLlmDir = Join-Path $destRoot "exp\dream_sft\llm\torch_ddp"
$dstFlowDir = Join-Path $destRoot "exp\dream_sft\flow\torch_ddp"

Invoke-RoboCopy -Src $srcModelDir -Dst $dstModelDir

New-Item -ItemType Directory -Force -Path $dstLlmDir | Out-Null
Copy-Item -Force -LiteralPath $srcLlm -Destination (Join-Path $dstLlmDir "epoch_5_whole.pt")

New-Item -ItemType Directory -Force -Path $dstFlowDir | Out-Null
Copy-Item -Force -LiteralPath $srcFlow -Destination (Join-Path $dstFlowDir "flow_avg.pt")

Write-Host "OK: synced weights into $destRoot"
Write-Host "Hint:"
Write-Host "  set CHAOS_TTS_PY_WORKDIR=$destRoot"
Write-Host "  then use relative paths:"
Write-Host "    model_dir=pretrained_models/Fun-CosyVoice3-0.5B-dream-sft"
Write-Host "    llm_ckpt=exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt"
Write-Host "    flow_ckpt=exp/dream_sft/flow/torch_ddp/flow_avg.pt"

