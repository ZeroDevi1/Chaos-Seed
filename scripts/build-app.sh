#!/usr/bin/env bash
# 把 SPM 可执行产物打包成标准 macOS .app bundle。
#
# 用法：
#   scripts/build-app.sh [release|debug]
#
# 产出：
#   build/ChaosSeed.app   （可直接双击运行 / 拖入 /Applications）
#
# 说明：
# - 先用 `swift build` 编译可执行文件，再组装 .app bundle。
# - 不做签名/公证（如需分发，需自行配置 Apple Developer 证书 + notarytool）。
# - Xcode-beta 工具链可通过 DEVELOPER_DIR 指定。
set -euo pipefail

CONFIG="${1:-debug}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_DIR="$ROOT/build/ChaosSeed.app"

SWIFT_BIN="${SWIFT_BIN:-/Applications/Xcode-beta.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/swift}"

# 默认携带 Xcode-beta 开发目录（命令行 swift 缺 BuildServerProtocol.framework）。
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode-beta.app/Contents/Developer}"

echo "==> swift build ($CONFIG)"
if [ "$CONFIG" = "release" ]; then
  "$SWIFT_BIN" build --package-path "$ROOT" -c release
else
  "$SWIFT_BIN" build --package-path "$ROOT"
fi

# 定位产物目录（兼容 SwiftPM 新旧布局：.build/out/Products 或 .build）。
SHOW_BIN_ARGS=(build --show-bin-path)
if [ "$CONFIG" = "release" ]; then
  SHOW_BIN_ARGS+=(-c release)
fi
PRODUCTS_DIR="$(cd "$ROOT" && "$SWIFT_BIN" "${SHOW_BIN_ARGS[@]}" 2>/dev/null || true)"
if [ -z "$PRODUCTS_DIR" ] || [ ! -d "$PRODUCTS_DIR" ]; then
  # 兜底：手动查找。
  PRODUCTS_DIR="$ROOT/.build/out/Products/$([ "$CONFIG" = release ] && echo Release || echo Debug)"
fi
EXEC="$PRODUCTS_DIR/ChaosSeed"
if [ ! -f "$EXEC" ]; then
  echo "❌ 找不到可执行产物：$EXEC" >&2
  exit 1
fi

echo "==> 组装 .app bundle -> $APP_DIR"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

cp "$EXEC" "$APP_DIR/Contents/MacOS/ChaosSeed"
chmod +x "$APP_DIR/Contents/MacOS/ChaosSeed"

# SwiftPM 的 Bundle.module 会从应用 Resources 中查找资源 bundle。
RESOURCE_BUNDLE="$PRODUCTS_DIR/ChaosSeed_ChaosSeedApp.bundle"
if [ -d "$RESOURCE_BUNDLE" ]; then
  cp -R "$RESOURCE_BUNDLE" "$APP_DIR/Contents/Resources/"
  echo "    资源：ChaosSeed_ChaosSeedApp.bundle"
else
  echo "❌ 找不到 SwiftPM 资源 bundle：$RESOURCE_BUNDLE" >&2
  exit 1
fi

# 复制应用图标（.icns）到 Resources，供 Info.plist 的 CFBundleIconFile 引用。
ICON_SRC="$ROOT/Sources/ChaosSeedApp/Resources/AppIcon.icns"
if [ -f "$ICON_SRC" ]; then
  cp "$ICON_SRC" "$APP_DIR/Contents/Resources/AppIcon.icns"
  echo "    图标：AppIcon.icns"
else
  echo "    ⚠️ 未找到 AppIcon.icns（跳过图标）"
fi

# Info.plist：声明为 LSUIElement=NO 的普通窗口应用、最小系统版本和图标。
cat > "$APP_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Chaos Seed</string>
    <key>CFBundleDisplayName</key>
    <string>Chaos Seed</string>
    <key>CFBundleIdentifier</key>
    <string>com.zerodevi1.chaosseed</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleExecutable</key>
    <string>ChaosSeed</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>26.0</string>
    <key>LSUIElement</key>
    <false/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsArbitraryLoads</key>
        <true/>
    </dict>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
PLIST

# PkgInfo（经典 8 字节标记）。
printf "APPL????" > "$APP_DIR/Contents/PkgInfo"

# SwiftPM 产物自带临时签名，其标识与最终 Bundle ID 不同。重新对完整 App
# 做 ad-hoc 签名，确保系统媒体服务读取到一致的应用身份。
codesign \
  --force \
  --deep \
  --sign - \
  --identifier com.zerodevi1.chaosseed \
  "$APP_DIR"
echo "    签名：ad-hoc（com.zerodevi1.chaosseed）"

echo "==> 完成 ✅"
echo "    应用：$APP_DIR"
echo "    启动：open \"$APP_DIR\""
