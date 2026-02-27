#Requires -Version 7.0

<#
.SYNOPSIS
  将本地导出的 CosyVoice ONNX pack（dream_sft_pack_v1）复制到本仓库约定位置（不进 git）。

.DESCRIPTION
  - 源目录默认指向 VoiceLab 工作流导出的 pack：
      C:\Projects\AntiGravityProjects\VoiceLab\workflows\cosyvoice\export_packs\dream_sft_pack_v1
  - 目标目录为本仓库根目录：
      models\cosyvoice\pack\dream_sft_pack_v1
  - 目标目录已被 .gitignore 排除，不会参与提交。

.PARAMETER SourceDir
  pack 源目录（包含 pack.json / *.onnx / tokenizer.json / spk2info.json 等）。

.PARAMETER DestDir
  复制到的目标目录（默认写入 repo 根目录下的 models/cosyvoice/pack/dream_sft_pack_v1）。
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [string]$SourceDir = "C:\\Projects\\AntiGravityProjects\\VoiceLab\\workflows\\cosyvoice\\export_packs\\dream_sft_pack_v1",

    [Parameter(Mandatory = $false)]
    [string]$DestDir
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot
{
    # 脚本位于 repo 的 scripts/ 目录，向上一级就是仓库根目录。
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($DestDir))
{
    $DestDir = Join-Path $repoRoot "models\\cosyvoice\\pack\\dream_sft_pack_v1"
}

$src = (Resolve-Path -LiteralPath $SourceDir -ErrorAction Stop).Path
$dst = $DestDir

if (-not (Test-Path -LiteralPath $src))
{
    throw "SourceDir not found: $src"
}

New-Item -ItemType Directory -Force -Path $dst | Out-Null

Write-Host "Copy CosyVoice pack:"
Write-Host "  Source: $src"
Write-Host "  Dest  : $dst"

# robocopy 对大文件夹更稳、速度更好；/E 包含子目录（含空目录）。
$robocopy = Get-Command robocopy.exe -ErrorAction SilentlyContinue
if ($null -ne $robocopy)
{
    # /R:2 /W:1 避免长时间卡住；/NP 不显示百分比，日志更清爽。
    $null = robocopy $src $dst /E /R:2 /W:1 /NP
    # robocopy 的 exit code: 0-7 都表示成功/有差异复制；>=8 才是错误。
    if ($LASTEXITCODE -ge 8)
    {
        throw "robocopy failed with exit code $LASTEXITCODE"
    }
} else
{
    Copy-Item -LiteralPath (Join-Path $src "*") -Destination $dst -Recurse -Force
}

Write-Host "Done."

# 简单校验（避免复制了不完整的 pack，运行时才踩坑）
$required = @(
    "pack.json",
    "tokenizer.json",
    "spk2info.json",
    "llm_prefill.onnx",
    "llm_decode.onnx",
    "flow_infer.onnx",
    "hift_infer.onnx"
)
$missing = @()
foreach ($f in $required)
{
    if (-not (Test-Path -LiteralPath (Join-Path $dst $f)))
    {
        $missing += $f
    }
}
if ($missing.Count -gt 0)
{
    Write-Warning ("pack 目录缺少文件：{0}`n当前目录可能无法生成音频（仅导出了部分 ONNX）。" -f ($missing -join ", "))
    Write-Warning "建议回到 VoiceLab 重新执行 tools/export_onnx_pack.py，确保导出 flow_infer.onnx 与 hift_infer.onnx。"
}
