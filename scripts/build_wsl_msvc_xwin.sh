#!/usr/bin/env bash
set -euo pipefail

# WSL -> Windows MSVC cross build (cargo-xwin).
#
# Prereqs:
#   rustup target add x86_64-pc-windows-msvc
#   cargo install cargo-xwin
#
# First build will download Windows SDK artifacts managed by xwin.
#
command -v pkg-config >/dev/null 2>&1 || { echo "missing: pkg-config (install: sudo apt-get install -y pkg-config)"; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "missing: cargo"; exit 1; }
cargo xwin --version >/dev/null 2>&1 || { echo "missing: cargo-xwin (install: cargo install cargo-xwin)"; exit 1; }
command -v clang-cl >/dev/null 2>&1 || { echo "missing: clang-cl (install: sudo apt-get install -y clang lld)"; exit 1; }

# Software renderer is much easier to cross-compile than Skia.
cargo xwin build --release --target x86_64-pc-windows-msvc --no-default-features --features renderer-software
echo "Built (path may vary): target/x86_64-pc-windows-msvc/release/chaos-seed.exe"
