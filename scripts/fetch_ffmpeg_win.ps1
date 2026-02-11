param(
  [Parameter(Mandatory = $true)]
  [string]$ProjectDir,
  [ValidateSet('n7.1','n8.0','master')]
  [string]$Track = 'n8.0',
  [switch]$Force
)

$ErrorActionPreference = 'Stop'

function Ensure-Dir([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path)) {
    New-Item -ItemType Directory -Path $Path | Out-Null
  }
}

function Has-FFmpegDlls([string]$Dir) {
  if (-not (Test-Path -LiteralPath $Dir)) { return $false }
  $dll = Get-ChildItem -LiteralPath $Dir -Filter 'avcodec*.dll' -File -ErrorAction SilentlyContinue | Select-Object -First 1
  return $null -ne $dll
}

$proj = Resolve-Path -LiteralPath $ProjectDir
$outDir = Join-Path $proj 'FFmpeg'

if (-not $Force) {
  if (Has-FFmpegDlls $outDir) {
    Write-Host "[ffmpeg] OK: already present at $outDir"
    exit 0
  }
}

Ensure-Dir $outDir

$headers = @{
  'User-Agent' = 'chaos-seed-ffmpeg-fetch'
  'Accept'     = 'application/vnd.github+json'
}

Write-Host "[ffmpeg] Fetching latest win64 LGPL shared build info (track=$Track)..."
$rel = Invoke-RestMethod -Uri 'https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest' -Headers $headers

$asset = $rel.assets | Where-Object {
  $_.name -match '^ffmpeg-' -and
  $_.name -match $Track -and
  $_.name -match 'win64' -and
  $_.name -match 'lgpl' -and
  $_.name -match 'shared' -and
  $_.name -match 'latest'
} | Select-Object -First 1

if (-not $asset) {
  throw "No matching asset found in latest release (track=$Track, win64 + lgpl + shared + latest)."
}

$zipUrl = $asset.browser_download_url
Write-Host "[ffmpeg] Downloading: $zipUrl"

$tmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("chaos-seed-ffmpeg-" + [System.Guid]::NewGuid().ToString('N'))
Ensure-Dir $tmpRoot
$zipPath = Join-Path $tmpRoot 'ffmpeg.zip'

Invoke-WebRequest -Uri $zipUrl -OutFile $zipPath -Headers $headers

$unzipDir = Join-Path $tmpRoot 'unzipped'
Ensure-Dir $unzipDir
Expand-Archive -LiteralPath $zipPath -DestinationPath $unzipDir -Force

$binDir = Get-ChildItem -LiteralPath $unzipDir -Directory -Recurse |
  Where-Object { $_.Name -ieq 'bin' } |
  Select-Object -First 1

if (-not $binDir) {
  throw 'Failed to locate bin/ directory in FFmpeg zip.'
}

Write-Host "[ffmpeg] Copying DLLs from: $($binDir.FullName)"
Get-ChildItem -LiteralPath $binDir.FullName -Filter '*.dll' -File | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force
}

Write-Host "[ffmpeg] Done: $(Get-ChildItem -LiteralPath $outDir -Filter '*.dll' -File | Measure-Object | Select-Object -ExpandProperty Count) DLL(s) copied to $outDir"

try {
  Remove-Item -LiteralPath $tmpRoot -Recurse -Force -ErrorAction SilentlyContinue
} catch {
  # ignore
}
