#!/usr/bin/env bash
set -euo pipefail

# Build libchaos_ffi.so for Android and copy into Flutter jniLibs.
#
# Requirements:
# - Android NDK installed (via Android Studio)
# - `cargo ndk` installed: `cargo install cargo-ndk`
#
# Usage:
#   bash tools/build_android_ffi.sh

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FLUTTER_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR"

# 需要构建的 Android ABI 列表。
# 默认只构建真机最常用的 arm64-v8a。
# 如需支持模拟器（通常是 x86_64），可这样调用：
#   CHAOS_ANDROID_ABIS="arm64-v8a x86_64" bash tools/build_android_ffi.sh
if [ -n "${CHAOS_ANDROID_ABIS:-}" ]; then
  # shellcheck disable=SC2206
  TARGETS=(${CHAOS_ANDROID_ABIS})
else
  TARGETS=("arm64-v8a")
fi

OUT_DIR="$FLUTTER_DIR/android/app/src/main/jniLibs"
mkdir -p "$OUT_DIR"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "ERROR: missing command: $cmd" >&2
    exit 1
  fi
}

abi_to_rust_target() {
  local abi="$1"
  case "$abi" in
    arm64-v8a) echo "aarch64-linux-android" ;;
    armeabi-v7a) echo "armv7-linux-androideabi" ;;
    x86_64) echo "x86_64-linux-android" ;;
    x86) echo "i686-linux-android" ;;
    *)
      echo "ERROR: unknown ABI: $abi" >&2
      exit 1
      ;;
  esac
}

require_cmd rustup
require_cmd cargo

if ! cargo ndk --version >/dev/null 2>&1; then
  echo "ERROR: missing cargo-ndk. Install with: cargo install cargo-ndk" >&2
  exit 1
fi

# Ensure the Rust std for Android target is installed for the active toolchain
# (this repo pins toolchain via rust-toolchain.toml).
TOOLCHAIN="$(rustup show active-toolchain 2>/dev/null | awk '{print $1}')"
if [ -z "${TOOLCHAIN:-}" ]; then
  echo "ERROR: unable to detect active Rust toolchain (rustup show active-toolchain)" >&2
  exit 1
fi

for ABI in "${TARGETS[@]}"; do
  RUST_TARGET="$(abi_to_rust_target "$ABI")"
  if ! rustup target list --installed --toolchain "$TOOLCHAIN" | grep -qx "$RUST_TARGET"; then
    echo "==> Installing Rust target $RUST_TARGET for toolchain $TOOLCHAIN"
    rustup target add --toolchain "$TOOLCHAIN" "$RUST_TARGET"
  fi

  echo "==> Building chaos-ffi for $ABI"
  cargo ndk -t "$ABI" -o "$OUT_DIR" build -p chaos-ffi --release
done

echo "==> Done. Output under: $OUT_DIR"
