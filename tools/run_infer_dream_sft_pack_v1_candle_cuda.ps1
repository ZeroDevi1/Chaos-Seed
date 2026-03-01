#!/usr/bin/env pwsh
#requires -Version 7.0

$ErrorActionPreference = "Stop"

function Add-ToPath([string]$p) {
    if ([string]::IsNullOrWhiteSpace($p)) { return }
    if (-not (Test-Path $p)) { return }
    $env:Path = "$p;$env:Path"
}

function Find-OrtDllDir() {
    $root = Join-Path $env:LOCALAPPDATA "ort.pyke.io\dfbin"
    if (-not (Test-Path $root)) { return $null }
    $dll = Get-ChildItem -Path $root -Recurse -Filter "onnxruntime.dll" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $dll) { return $null }
    return (Split-Path $dll.FullName -Parent)
}

# 1) VS Dev Env（用于 nvcc -> cl.exe / link.exe）
if (Get-Command vsenv -ErrorAction SilentlyContinue) {
    vsenv | Out-Host
} else {
    throw "vsenv 未找到：请确保 PowerShell Profile 里已配置 vsenv（或手动先运行 vsenv 再执行本脚本）。"
}

# 2) CUDA 环境（运行期 DLL 在 bin\x64）
if (-not $env:CUDA_PATH) { throw "CUDA_PATH 未设置：请先安装 CUDA Toolkit 并确保系统环境变量 CUDA_PATH 存在。" }
$env:CUDA_HOME = $env:CUDA_PATH
Add-ToPath (Join-Path $env:CUDA_PATH "bin")
Add-ToPath (Join-Path $env:CUDA_PATH "bin\x64")

# 3) ORT DLL
# 说明：本脚本默认用 `--no-default-features` 运行测试，从而避免引入 chaos-core 的默认 onnx-ort 依赖；
# 因此通常不需要 onnxruntime.dll。
# 如果你手动改成启用 onnx-ort（或默认 features），再考虑把 ORT DLL 目录加到 PATH。

# 4) nvcc host compiler（让 bindgen_cuda/nvcc 使用当前 VS 环境的 cl.exe）
$cl = (Get-Command cl.exe).Source
$env:NVCC_CCBIN = Split-Path $cl -Parent
Write-Host "[nvcc] NVCC_CCBIN=$env:NVCC_CCBIN"

# 5) 生成 protobuf（如未设置会导致 prost-build 找不到 protoc）
if (-not $env:PROTOC) {
    $defaultProtoc = "C:\Projects\RustRoverProjects\Chaos-Seed\third_party\_tools\protoc-28.3-win64\bin\protoc.exe"
    if (Test-Path $defaultProtoc) {
        $env:PROTOC = $defaultProtoc
        Write-Host "[protoc] PROTOC=$env:PROTOC"
    } else {
        Write-Warning "[protoc] PROTOC 未设置且默认路径不存在：$defaultProtoc"
    }
}

# 6) 推理相关 env（允许用户在外部覆盖）
if (-not $env:CHAOS_TTS_BACKEND) { $env:CHAOS_TTS_BACKEND = "candle" }
if (-not $env:CHAOS_COSYVOICE3_DEVICE) { $env:CHAOS_COSYVOICE3_DEVICE = "auto" }
if (-not $env:CHAOS_COSYVOICE3_SPK_ID) { $env:CHAOS_COSYVOICE3_SPK_ID = "dream" }

if (-not $env:CHAOS_COSYVOICE3_CANDLE_MODEL_DIR) {
    $defaultModel = Join-Path $PSScriptRoot "..\models\cosyvoice3_candle\dream_sft_epoch5"
    if (Test-Path $defaultModel) {
        $env:CHAOS_COSYVOICE3_CANDLE_MODEL_DIR = $defaultModel
        Write-Host "[model] CHAOS_COSYVOICE3_CANDLE_MODEL_DIR=$env:CHAOS_COSYVOICE3_CANDLE_MODEL_DIR"
    } else {
        Write-Warning "[model] CHAOS_COSYVOICE3_CANDLE_MODEL_DIR 未设置且默认路径不存在：$defaultModel"
    }
}

# 注：prompt_wav 必须是“参考声音音频”，不是生成出来的 TTS 音频；否则提取到的 prompt features 往往不稳定，容易导致杂音/长度异常。
if (-not $env:CHAOS_TTS_PROMPT_WAV) {
    $spk2info = Join-Path $env:CHAOS_COSYVOICE3_CANDLE_MODEL_DIR "spk2info.json"
    if (Test-Path $spk2info) {
        Write-Host "[spk] prompt_wav 未设置：将使用 spk_id=$env:CHAOS_COSYVOICE3_SPK_ID (spk2info.json) 做说话人条件。"
    } else {
        Write-Warning "CHAOS_TTS_PROMPT_WAV 未设置，且模型目录下也没有 spk2info.json：将退化到 text-only fallback（音色/质量通常不可控）。"
        Write-Warning "提示：把 VoiceLab 的 spk2info.pt 转成 spk2info.json（embedding Vec<f32>）放到模型目录，或设置 CHAOS_COSYVOICE3_SPK2INFO_JSON。"
    }
}

Write-Host "[run] cargo test (release + cuda) ..."
$features = @("live-tests", "cosyvoice3-candle", "cosyvoice3-candle-cuda")
if ($env:CHAOS_TTS_PROMPT_WAV) {
    # 仅当你真的需要从 prompt_wav 提取 prompt features 时，才启用 ONNX frontend，避免额外编译/依赖。
    $features += "cosyvoice3-candle-onnx"
}
$featureStr = ($features -join " ")
& cargo test -p chaos-core --no-default-features --release --features $featureStr --test infer_dream_sft_pack_v1 -- --nocapture
exit $LASTEXITCODE
