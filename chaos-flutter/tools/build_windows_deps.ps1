param(
  [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"

function Info([string]$msg) { Write-Host "==> $msg" }

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\\..")).Path
$flutter = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$deps = Join-Path $flutter "windows\\deps"

Info "Repo root: $root"
Info "Flutter dir: $flutter"
Info "Deps out: $deps"

New-Item -ItemType Directory -Force -Path $deps | Out-Null

Push-Location $root
try {
  Info "Building chaos-ffi ($Configuration)"
  cargo build -p chaos-ffi --release | Out-Host

  Info "Building chaos-daemon ($Configuration)"
  cargo build -p chaos-daemon --release | Out-Host

  Info "Building Updater ($Configuration)"
  dotnet build (Join-Path $root "chaos-winui3\\ChaosSeed.Updater\\ChaosSeed.Updater.csproj") -c $Configuration | Out-Host
}
finally {
  Pop-Location
}

$ffi = Join-Path $root "target\\release\\chaos_ffi.dll"
$daemon = Join-Path $root "target\\release\\chaos-daemon.exe"

if (!(Test-Path $ffi)) { throw "Missing $ffi" }
if (!(Test-Path $daemon)) { throw "Missing $daemon" }

Copy-Item -Force $ffi $deps
Copy-Item -Force $daemon $deps

# Locate ChaosSeed.Updater.exe
$updaterCandidates = @(
  (Join-Path $root "chaos-winui3\\ChaosSeed.Updater\\bin\\$Configuration\\net8.0-windows\\ChaosSeed.Updater.exe"),
  (Join-Path $root "chaos-winui3\\ChaosSeed.Updater\\bin\\x64\\$Configuration\\net8.0-windows\\ChaosSeed.Updater.exe")
)
$updater = $updaterCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (!$updater) {
  $probe = Get-ChildItem -Recurse -ErrorAction SilentlyContinue -Path (Join-Path $root "chaos-winui3\\ChaosSeed.Updater\\bin") -Filter "ChaosSeed.Updater.exe" | Select-Object -First 1
  if ($probe) { $updater = $probe.FullName }
}
if (!$updater) { throw "Failed to locate ChaosSeed.Updater.exe under chaos-winui3/ChaosSeed.Updater/bin" }

Copy-Item -Force $updater $deps

Info "Done. Copied:"
Info " - $ffi"
Info " - $daemon"
Info " - $updater"

