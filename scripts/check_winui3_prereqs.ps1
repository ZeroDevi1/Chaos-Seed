$ErrorActionPreference = "Stop"

param(
  [string]$RuntimeMajorMinor = "1.8"
)

function Write-Section([string]$title) {
  Write-Host ""
  Write-Host "== $title ==" -ForegroundColor Cyan
}

Write-Section "OS / Process"
Write-Host "ComputerName: $env:COMPUTERNAME"
Write-Host "OS: $([System.Environment]::OSVersion.VersionString)"
Write-Host "Arch: $env:PROCESSOR_ARCHITECTURE"

Write-Section "Windows App Runtime (AppX)"
try {
  $pkgs = Get-AppxPackage | Where-Object {
    $_.Name -like "*WindowsAppRuntime*" -or $_.Name -like "*WinAppRuntime*"
  }

  if (-not $pkgs) {
    Write-Host "No Windows App Runtime packages found via Get-AppxPackage." -ForegroundColor Yellow
  } else {
    $pkgs |
      Sort-Object Name, Version |
      Select-Object Name, Version, Architecture, PackageFullName, InstallLocation |
      Format-Table -AutoSize
  }

  $expected = @(
    "Microsoft.WindowsAppRuntime.$RuntimeMajorMinor",
    "MicrosoftCorporationII.WinAppRuntime.Main.$RuntimeMajorMinor",
    "MicrosoftCorporationII.WinAppRuntime.Singleton.$RuntimeMajorMinor"
  )

  foreach ($name in $expected) {
    $hit = $pkgs | Where-Object { $_.Name -like "$name*" }
    if ($hit) {
      Write-Host "[OK] $name" -ForegroundColor Green
    } else {
      Write-Host "[MISSING?] $name" -ForegroundColor Yellow
    }
  }
} catch {
  Write-Host "Get-AppxPackage failed: $($_.Exception.Message)" -ForegroundColor Yellow
}

Write-Section "VC++ 2015-2022 Redistributable (x64)"
function Get-VcRuntime([string]$arch) {
  $paths = @(
    "HKLM:\\SOFTWARE\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\$arch",
    "HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\$arch"
  )
  foreach ($p in $paths) {
    if (Test-Path $p) {
      try {
        $v = Get-ItemProperty -Path $p
        return @{
          Path = $p
          Installed = $v.Installed
          Version = $v.Version
          Major = $v.Major
          Minor = $v.Minor
          Bld = $v.Bld
          Rbld = $v.Rbld
        }
      } catch {
        return @{ Path = $p; Error = $_.Exception.Message }
      }
    }
  }
  return $null
}

$vc = Get-VcRuntime "x64"
if (-not $vc) {
  Write-Host "VC++ runtime registry key not found for x64." -ForegroundColor Yellow
} elseif ($vc.Error) {
  Write-Host "VC++ runtime read error ($($vc.Path)): $($vc.Error)" -ForegroundColor Yellow
} else {
  Write-Host "Key: $($vc.Path)"
  Write-Host "Installed: $($vc.Installed)  Version: $($vc.Version)  Build: $($vc.Major).$($vc.Minor).$($vc.Bld).$($vc.Rbld)"
}

$sys32 = Join-Path $env:WINDIR "System32"
$crtFiles = @("vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll")
foreach ($f in $crtFiles) {
  $p = Join-Path $sys32 $f
  if (Test-Path $p) {
    Write-Host "[OK] $p" -ForegroundColor Green
  } else {
    Write-Host "[MISSING?] $p" -ForegroundColor Yellow
  }
}

Write-Section ".NET (optional)"
try {
  $dotnet = Get-Command dotnet -ErrorAction Stop
  Write-Host "dotnet: $($dotnet.Source)"
  dotnet --list-runtimes
} catch {
  Write-Host "dotnet not found in PATH (this is OK if you run a self-contained build)." -ForegroundColor Yellow
}

Write-Section "Next steps"
Write-Host "- If ChaosSeed.WinUI3 crashes at startup with 0xc000027b and the log stops at 'Calling XamlCheckProcessRequirements...', fix Windows App Runtime / VC++ prerequisites."
Write-Host "- If you run a zip/unpackaged Release build, consider building with the default self-contained Windows App SDK runtime (or disable with /p:ChaosWinUI3SelfContained=false)."

