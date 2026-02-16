#!/usr/bin/env bash
set -euo pipefail

# Build libchaos_ffi.so for Android and copy into chaos-android jniLibs.
#
# Requirements:
# - Android NDK installed (via Android Studio)
# - cargo-ndk installed: `cargo install cargo-ndk`
#
# Usage:
#   bash tools/build_android_ffi.sh
#
# Optional env vars:
# - CHAOS_ANDROID_ABIS="arm64-v8a x86_64"   (default: "arm64-v8a x86_64")
# - CHAOS_ANDROID_PROFILE=release|debug    (default: release)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANDROID_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="$(cd "$ANDROID_DIR/.." && pwd)"

cd "$ROOT_DIR"

if [ -n "${CHAOS_ANDROID_ABIS:-}" ]; then
  # shellcheck disable=SC2206
  TARGETS=(${CHAOS_ANDROID_ABIS})
else
  TARGETS=("arm64-v8a" "x86_64")
fi

PROFILE="${CHAOS_ANDROID_PROFILE:-release}"
if [ "$PROFILE" != "release" ] && [ "$PROFILE" != "debug" ]; then
  echo "ERROR: invalid CHAOS_ANDROID_PROFILE=$PROFILE (expected release|debug)" >&2
  exit 1
fi

OUT_DIR="$ANDROID_DIR/app/src/main/jniLibs"
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
require_cmd awk

if ! cargo ndk --version >/dev/null 2>&1; then
  echo "ERROR: missing cargo-ndk. Install with: cargo install cargo-ndk" >&2
  exit 1
fi

# Keep original flags so we don't append duplicates per-ABI.
ORIG_RUSTFLAGS="${RUSTFLAGS:-}"

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

  echo "==> Building chaos-ffi for ABI=$ABI profile=$PROFILE"
  
  # ==============================
  # [新增] 强制设置链接器参数以支持 16KB Page Size
  # ==============================
  # -C link-arg=-Wl,-z,max-page-size=16384 : 告诉链接器将最大页大小设为 16KB
  export RUSTFLAGS="${ORIG_RUSTFLAGS} -C link-arg=-Wl,-z,max-page-size=16384"

  if [ "$PROFILE" = "release" ]; then
    cargo ndk -t "$ABI" -o "$OUT_DIR" build -p chaos-ffi --release
  else
    cargo ndk -t "$ABI" -o "$OUT_DIR" build -p chaos-ffi
  fi
done

echo "==> Done. Output under: $OUT_DIR"
echo "    e.g. $OUT_DIR/arm64-v8a/libchaos_ffi.so"
