$ErrorActionPreference = "Stop"

# Windows native build (MSVC) with Skia renderer.
# If this fails with unresolved C++ STL symbols (e.g. __std_find_first_of_trivial_pos_1),
# use scripts/build_win.ps1 (software renderer) instead.
cargo build --release --no-default-features --features renderer-skia
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Built: target/release/chaos-seed.exe"
