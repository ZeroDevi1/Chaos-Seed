$ErrorActionPreference = "Stop"

# Windows native build (MSVC)
# Default to software renderer for reliability. (Skia may fail to link on some toolsets.)
cargo build --release --no-default-features --features renderer-software
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Built: target/release/chaos-seed.exe"

