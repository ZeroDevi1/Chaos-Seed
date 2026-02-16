#!/usr/bin/env bash
set -euo pipefail

# 从仓库根目录的 assets/icon.png 生成 Android launcher icon（mipmap-*）。
# 说明：
# - Android launcher icon 需要 PNG，不支持直接使用 .ico。
# - 我们用 ImageMagick 的 `convert` 做缩放；如果你没有安装 ImageMagick，请先安装它。

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SRC_PNG="${ROOT_DIR}/assets/icon.png"
RES_DIR="${ROOT_DIR}/chaos-flutter/android/app/src/main/res"

if [[ ! -f "${SRC_PNG}" ]]; then
  echo "找不到 ${SRC_PNG}"
  exit 1
fi

if ! command -v convert >/dev/null 2>&1; then
  echo "未找到 ImageMagick 的 convert，请先安装 ImageMagick。"
  exit 1
fi

declare -A SIZES=(
  ["mipmap-mdpi"]=48
  ["mipmap-hdpi"]=72
  ["mipmap-xhdpi"]=96
  ["mipmap-xxhdpi"]=144
  ["mipmap-xxxhdpi"]=192
)

for d in "${!SIZES[@]}"; do
  size="${SIZES[$d]}"
  out="${RES_DIR}/${d}/ic_launcher.png"
  mkdir -p "$(dirname "$out")"
  convert "${SRC_PNG}" -resize "${size}x${size}" "${out}"
  echo "OK: ${out} (${size}x${size})"
done

echo "Done."

