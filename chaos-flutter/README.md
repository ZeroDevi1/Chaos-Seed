# chaos-flutter

`Chaos-Seed` 的 Flutter UI（重写 `chaos-winui3`），目标平台：
- Windows（Fluent 风格）：支持 **FFI**（`chaos_ffi.dll`）与 **daemon**（`chaos-daemon.exe --stdio`）两种方式调用 Rust Core
- Android（Material 3）：通过 **FFI** 调用 Rust Core（不走 daemon，不在 Dart 侧复刻核心逻辑）

构建指南（仓库根目录文档）：`../docs/BUILD_CHAOS_FLUTTER.md`

> 备注：Android 第一阶段暂时隐藏“字幕下载”入口；QQ 音乐支持扫码登录并缓存 Cookie，用于下载。

## 一次性初始化（生成平台脚手架）

本仓库不提交 `flutter create` 生成的完整平台工程（例如 `windows/runner`、完整 `android/` Gradle 工程）。

你可以在 `chaos-flutter/` 下执行：

```bash
flutter create --platforms=windows,android --org com.zerodevi1 --project-name chaos_flutter .
```

注意：该命令可能会重写 `lib/main.dart` / `pubspec.yaml` 等文件；更稳妥的做法请参考构建指南里的“一次性初始化”章节（在临时目录生成后再拷回）。

## Windows 依赖（FFI + daemon + Updater）

运行/打包时需要把以下文件与应用可执行文件放在同一目录（或用脚本拷贝到 `windows/deps/` 再由打包流程带上）：
- `chaos_ffi.dll`
- `chaos-daemon.exe`
- `ChaosSeed.Updater.exe`

构建/拷贝脚本：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/build_windows_deps.ps1
```

## Android FFI（JNI libs）

构建 `libchaos_ffi.so` 并复制到 `android/app/src/main/jniLibs/**/`：

```bash
bash tools/build_android_ffi.sh
```
