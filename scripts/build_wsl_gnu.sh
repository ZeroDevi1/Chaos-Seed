#!/usr/bin/env bash
set -euo pipefail

# WSL -> Windows GNU cross build.
#
# Prereqs (Ubuntu/Debian):
#   sudo apt-get update
#   sudo apt-get install -y mingw-w64 pkg-config libfontconfig1-dev
#   rustup target add x86_64-pc-windows-gnu
#
command -v pkg-config >/dev/null 2>&1 || { echo "missing: pkg-config (install: sudo apt-get install -y pkg-config)"; exit 1; }
command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 || { echo "missing: mingw-w64 (install: sudo apt-get install -y mingw-w64)"; exit 1; }

# Software renderer is much easier to cross-compile than Skia.
cargo build --release --target x86_64-pc-windows-gnu --no-default-features --features renderer-software
echo "Built: target/x86_64-pc-windows-gnu/release/chaos-seed.exe"
