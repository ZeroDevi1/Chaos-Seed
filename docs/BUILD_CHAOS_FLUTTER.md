# Chaos-Flutter 构建指南（Windows + Android，第一阶段）

本指南面向仓库根目录 `Chaos-Seed/`，Flutter 项目位于 `chaos-flutter/`。

> 重要：当前仓库内 `chaos-flutter/` 只包含 Dart 代码与脚本，**未包含** Flutter 自动生成的 `windows/runner`、完整 `android/` Gradle 工程等平台脚手架。你需要先用 `flutter create` 生成它们（见下文“一次性初始化”）。

---

## 0. 依赖与环境

### 通用（Windows/Android 都需要）
- Flutter SDK（含 Dart，Dart >= 3.6）：`flutter doctor` 可通过
- Rust（按仓库 `rust-toolchain.toml` 固定）：当前 `1.93.0`

验证：

```bash
flutter --version
flutter doctor -v
rustc -V
cargo -V
```

### Windows 额外需要
- Visual Studio 2022（Desktop development with C++）
- .NET 8 SDK（用于构建 `ChaosSeed.Updater`）

验证：

```powershell
dotnet --version
```

### Android 额外需要
- Android Studio（安装 SDK / NDK）
- `cargo-ndk`（用于构建 `libchaos_ffi.so`）

安装：

```bash
cargo install cargo-ndk
```

建议设置环境变量（示例）：
- `ANDROID_SDK_ROOT`
- `ANDROID_NDK_HOME`

另外需要为 Rust 安装 Android target（否则会报 `can't find crate for core/std`）：

```bash
# 本仓库 rust-toolchain.toml 固定 toolchain 为 1.93.0，建议显式指定 --toolchain
rustup target add --toolchain 1.93.0 aarch64-linux-android

# 可选：如果你要构建更多 ABI
# rustup target add --toolchain 1.93.0 armv7-linux-androideabi
# rustup target add --toolchain 1.93.0 x86_64-linux-android
# rustup target add --toolchain 1.93.0 i686-linux-android
```

> 说明：如果你在 WSL/Linux 环境构建 Android so，请确保你配置的是 **Linux 版** Android NDK（而不是 Windows 版路径）。最简单做法是直接在 Linux 下安装 Android Studio/Commandline tools，并设置 `ANDROID_SDK_ROOT`/`ANDROID_NDK_HOME`。

---

## 1. 一次性初始化（生成 Flutter 平台脚手架）

由于本仓库不提交 `flutter create` 生成的完整平台工程，你需要在 `chaos-flutter/` 下生成 `windows/` 与 `android/`。

推荐做法（最安全，不覆盖现有 `lib/`）：

1) 在临时目录创建一个新 Flutter 工程：

```bash
mkdir -p /tmp/chaos_flutter_bootstrap
cd /tmp/chaos_flutter_bootstrap
flutter create --platforms=windows,android --org com.zerodevi1 --project-name chaos_flutter chaos_flutter_bootstrap
```

2) 把生成的 `windows/`、`android/`、`pubspec.yaml` 中的必要段落合并/拷贝到仓库 `chaos-flutter/`：
- 复制目录：
  - `/tmp/chaos_flutter_bootstrap/chaos_flutter_bootstrap/windows` -> `Chaos-Seed/chaos-flutter/windows`
  - `/tmp/chaos_flutter_bootstrap/chaos_flutter_bootstrap/android` -> `Chaos-Seed/chaos-flutter/android`
- **不要覆盖**仓库里的 `chaos-flutter/lib/`（里面已经是项目代码）

> 如果你更习惯在现有目录直接运行 `flutter create .`：请先确保 `git status` 干净并做好备份，因为它可能重写 `lib/main.dart` / `pubspec.yaml` 等文件。

---

## 2. Windows：本地开发运行（FFI + daemon）

### 2.1 构建并拷贝 Windows 依赖（推荐脚本）

在仓库根目录执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File chaos-flutter/tools/build_windows_deps.ps1
```

该脚本会：
- `cargo build -p chaos-ffi --release` 生成 `target/release/chaos_ffi.dll`
- `cargo build -p chaos-daemon --release` 生成 `target/release/chaos-daemon.exe`
- `dotnet build chaos-winui3/ChaosSeed.Updater/...` 生成 `ChaosSeed.Updater.exe`
- 把它们拷贝到：`chaos-flutter/windows/deps/`

### 2.2 运行 Flutter（Windows）

```powershell
cd chaos-flutter
flutter pub get
flutter run -d windows
```

说明：
- Windows 端支持两种调用 Rust Core 的方式：
  - **FFI**：直接调用 `chaos_ffi.dll`
  - **daemon**：Flutter 启动 `chaos-daemon.exe --stdio --auth-token <token>`，通过 JSON-RPC(LSP framing) 通讯
- 设置页可切换 Live/Music/Lyrics/Danmaku 的 backend 模式（Auto/FFI/daemon）。当前实现使用 `HybridChaosBackend` 进行按模块路由。

### 2.3 Windows 打包（zip 便携版）

```powershell
cd chaos-flutter
flutter build windows --release
```

然后把依赖文件拷贝到可执行文件同目录（通常是 `build/windows/x64/runner/Release/`）：
- `chaos_ffi.dll`
- `chaos-daemon.exe`
- `ChaosSeed.Updater.exe`

最后将整个目录打包为 zip（供自更新使用）。

> 自更新：Flutter 侧未来会用 `ChaosSeed.Updater.exe --app-name ChaosSeed.Flutter ...`。Updater 已支持 `--app-name`，会把日志与 pending updates 放到 `%LocalAppData%/<app-name>/...`，避免与 WinUI3 冲突。

---

## 3. Android：本地开发运行（只走 FFI 调用 core）

### 3.0 准备 Android 调试环境（设备 / 模拟器）

你不需要“额外开启 Flutter 配置”，但你需要让 Flutter 能识别到 **Android 设备或模拟器**。

在终端检查：

```bash
flutter doctor -v
flutter devices
flutter emulators
```

常见情况与处理：
- `flutter devices` 里没有 Android：说明 Android SDK/adb 没装好或 Android Studio 没配置好 SDK。
  - 在 Android Studio：`Tools -> SDK Manager` 安装 Android SDK（Platform + Build-Tools + Command-line Tools）
  - `Tools -> Device Manager` 创建并启动一个模拟器
  - 真机：打开“开发者选项/USB 调试”，并在电脑上允许调试授权
- IDE 里设备下拉列表没刷新：点一下 “Restart Flutter Daemon” 或重启 IDE

> 提醒：Android 模拟器通常是 **x86_64**，真机通常是 **arm64-v8a**。你的 `libchaos_ffi.so` 必须包含对应 ABI，否则运行时会报找不到/无法加载动态库。

### 3.1 构建 Android 的 `libchaos_ffi.so` 并放到 jniLibs

在仓库根目录执行：

```bash
bash chaos-flutter/tools/build_android_ffi.sh
```

默认仅构建 `arm64-v8a`，输出到：
- `chaos-flutter/android/app/src/main/jniLibs/arm64-v8a/`

如需同时支持更多 ABI（例如模拟器 x86_64），可以通过环境变量指定：

```bash
CHAOS_ANDROID_ABIS="arm64-v8a x86_64" bash chaos-flutter/tools/build_android_ffi.sh
```

如果你改为构建其它 ABI，但 Rust target 未安装，脚本会自动执行 `rustup target add`。

### 3.2 运行 Flutter（Android）

```bash
cd chaos-flutter
flutter pub get
flutter run -d android
```

说明（与你的要求对齐）：
- Android 端 **固定使用 FFI** 调用 Rust Core（不走 daemon，不在 Dart 侧复刻业务逻辑）。
- 当前 backend 选择逻辑见：`chaos-flutter/lib/core/backend/backend_factory.dart`

### 3.3 应用名称与图标（Android）

当前 Android 应用名称为：`Chaos Seed`（见 `chaos-flutter/android/app/src/main/AndroidManifest.xml`）。

图标源文件在仓库根目录：`assets/icon.png` 与 `assets/icon.ico`：
- Android launcher icon 必须是 PNG（`.ico` 不能直接作为 launcher icon）
- Windows runner icon 使用 `.ico`

如需重新生成 Android launcher icon（覆盖 `android/app/src/main/res/mipmap-*/ic_launcher.png`）：

```bash
bash chaos-flutter/tools/gen_android_launcher_icons.sh
```

### 3.4 画中画（PiP）说明

播放器页提供“画中画/小窗播放”按钮（Android 8.0 / API 26+ 支持）。

如果你的设备系统版本低于 Android 8.0，该按钮会自动隐藏。

---

## 4. 常见问题（排错）

### 4.1 找不到 `chaos_ffi.dll` / `libchaos_ffi.so`
- Windows：确认 `chaos-flutter/windows/deps/` 下存在 `chaos_ffi.dll`，并且运行目录能找到它（通常需要拷到 exe 同目录）
- Android：确认 `chaos-flutter/android/app/src/main/jniLibs/<abi>/libchaos_ffi.so` 存在

### 4.2 daemon `--stdio` 无法工作
- 确认 `chaos-daemon.exe` 是最新构建，启动参数包含 `--stdio --auth-token <token>`
- 确认 stdout 没被其它日志污染（stdio 模式下 stdout 只应输出协议帧）

### 4.3 Flutter 平台工程缺失
- 参考“一次性初始化”，先生成 `windows/`、`android/` 目录

### 4.4 Windows 运行时报错：No Windows desktop project configured

现象（示例）：
> Error: No Windows desktop project configured.

原因：
- `chaos-flutter/` 下缺少 Flutter 自动生成的 Windows 工程（通常是 `windows/runner` 等）。

解决：
1) 确保 Flutter 开启 Windows 桌面支持（Windows 机器上执行）：

```powershell
flutter config --enable-windows-desktop
```

2) 在 `chaos-flutter/` 目录生成 Windows 平台工程（会补齐 `windows/`）：

```powershell
cd chaos-flutter
flutter create --platforms=windows --org com.zerodevi1 --project-name chaos_flutter .
```

3) 如 `flutter create` 覆盖了本仓库已有的 `lib/` 代码，请用 git 恢复：
- 优先做法：先 `git status` 确认变更，再用 `git restore`/`git checkout` 恢复 `chaos-flutter/lib/` 与 `chaos-flutter/pubspec.yaml`。

### 4.5 Windows 构建时报错：mpv-dev-*.7z Integrity check failed

现象（示例）：
> `mpv-dev-x86_64-....7z Integrity check failed, please try to re-build project again.`

原因（常见）：
- 构建时 `media_kit_libs_windows_video` 会自动下载 `mpv` 依赖压缩包（`.7z`）。
- 网络抖动/代理/杀软拦截可能导致下载文件不完整或被篡改，从而校验失败。

解决（推荐按顺序尝试）：

1) 删除损坏的 `.7z` 并重新构建（最常见有效）：
```powershell
cd chaos-flutter
Remove-Item -Force -ErrorAction SilentlyContinue build\\windows\\x64\\mpv-dev-*.7z
flutter clean
flutter pub get
flutter run -d windows
```

2) 如果仍失败，删掉 Windows ephemeral 目录让插件重新生成并重新下载：
```powershell
cd chaos-flutter
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue windows\\flutter\\ephemeral
flutter clean
flutter pub get
flutter run -d windows
```

3) 仍失败时：
- 暂时关闭代理/抓包软件/杀毒实时防护（或把项目目录加入信任）
- 换一个网络环境（例如手机热点）
- 重试构建（该问题通常与网络/缓存有关）

### 4.6 Android 构建时报错：flutter-plugin-loader included build 不存在

现象（示例）：
> Error resolving plugin [id: 'dev.flutter.flutter-plugin-loader', version: '1.0.0']  
> Included build '.../packages/flutter_tools/gradle' does not exist.

常见原因：
- 你把仓库从 WSL/Linux 同步到了 Windows（例如 `sync_to_win.sh`），但 `chaos-flutter/android/local.properties` 仍是 Linux 路径：
  - `flutter.sdk=/home/.../flutter`
  - `sdk.dir=/home/.../android-sdk`
  Gradle 在 Windows 上会把以 `/` 开头的路径当成“相对路径”，最后拼出类似 `android\\home\\...`，从而找不到 `flutter_tools/gradle`。

解决：
1) 在 **Windows** 上编辑 `chaos-flutter/android/local.properties`，把路径改成本机真实路径，例如：

```properties
sdk.dir=C:\\Users\\<你>\\AppData\\Local\\Android\\Sdk
flutter.sdk=C:\\src\\flutter
```

2) 或者删除 `local.properties` 后，让 Flutter/Gradle 在本机重新生成：
   - 删除：`chaos-flutter\\android\\local.properties`
   - 然后在 `chaos-flutter/` 目录重新运行：`flutter clean`、`flutter pub get`、`flutter run -d <device>`

3) 如果你不想依赖 `local.properties`，也可以设置环境变量：
   - `FLUTTER_ROOT=<Flutter SDK 路径>`

> 额外建议：`local.properties` 是**机器本地文件**，不要跨机器同步。仓库里的 `sync_to_win.sh` 已排除该文件（避免把 WSL 路径同步到 Windows）。

另外，`sync_to_win.sh` 也会排除 `chaos-flutter/windows/deps/`（Windows 本地构建出来的 dll/exe 依赖不应该被 Linux 侧同步时的 `--delete` 删除）。
