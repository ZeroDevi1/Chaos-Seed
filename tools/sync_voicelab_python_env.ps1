param(
    # VoiceLab cosyvoice workflow 目录（包含 .venv）
    [string]$VoiceLabWorkdir = "C:\\Projects\\AntiGravityProjects\\VoiceLab\\workflows\\cosyvoice",
    # 输出到当前仓库 third_party/voicelab_py_env
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

function Info([string]$msg) { Write-Host "[sync_py_env] $msg" }
function Warn([string]$msg) { Write-Warning "[sync_py_env] $msg" }

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path $PSScriptRoot "..\\third_party\\voicelab_py_env"
}

$VoiceLabWorkdir = $VoiceLabWorkdir.Trim()
if (-not (Test-Path -LiteralPath $VoiceLabWorkdir)) {
    throw "VoiceLabWorkdir not found: $VoiceLabWorkdir"
}

$venvRoot = Join-Path $VoiceLabWorkdir ".venv"
if (-not (Test-Path -LiteralPath $venvRoot)) {
    throw "venv not found: $venvRoot (hint: run uv sync / create venv first)"
}

$venvPy = Join-Path $venvRoot "Scripts\\python.exe"
if (-not (Test-Path -LiteralPath $venvPy)) {
    throw "venv python.exe not found: $venvPy"
}

Info "VoiceLabWorkdir=$VoiceLabWorkdir"
Info "VenvRoot=$venvRoot"

# uv Python：base_prefix 通常位于 %APPDATA%\\uv\\python\\...，其中包含 python310.dll + 标准库 Lib/DLLs
$basePrefix = & $venvPy -c "import sys; print(sys.base_prefix)"
$basePrefix = ($basePrefix | Out-String).Trim()
if ([string]::IsNullOrWhiteSpace($basePrefix)) {
    throw "failed to resolve sys.base_prefix from venv python"
}
if (-not (Test-Path -LiteralPath $basePrefix)) {
    throw "sys.base_prefix not found: $basePrefix"
}

Info "PythonBasePrefix=$basePrefix"

$dstPython = Join-Path $OutDir "python"
$dstVenv = Join-Path $OutDir ".venv"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Info "Copy python runtime -> $dstPython"
if (Test-Path -LiteralPath $dstPython) { Remove-Item -LiteralPath $dstPython -Recurse -Force }
New-Item -ItemType Directory -Force -Path $dstPython | Out-Null

# 用 robocopy 更稳（大量小文件）
& robocopy $basePrefix $dstPython /MIR /NFL /NDL /NJH /NJS /NP | Out-Null

Info "Copy venv -> $dstVenv"
if (Test-Path -LiteralPath $dstVenv) { Remove-Item -LiteralPath $dstVenv -Recurse -Force }
New-Item -ItemType Directory -Force -Path $dstVenv | Out-Null
& robocopy $venvRoot $dstVenv /MIR /NFL /NDL /NJH /NJS /NP | Out-Null

Info "Done."

