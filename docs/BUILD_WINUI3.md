# BUILD_WINUI3（Windows / WinUI 3 + chaos-daemon）

> 本文档用于在 Windows 上构建与运行 `chaos-winui3`（WinUI 3 UI）与 `chaos-daemon`（Rust NamedPipe JSON-RPC 后端）。
> 本仓库在 WSL 内开发时，推荐先同步到 Windows 文件系统后再在 Windows 侧执行构建。

## 依赖（Windows）

- Rust toolchain：仓库根 `rust-toolchain.toml` 固定（当前为 `1.93.0`）
- .NET SDK：`.NET 8`
- Visual Studio Build Tools（包含 MSBuild）
- Windows SDK（WinUI 3 需要）
- Windows App SDK Runtime（WinUI 3 需要）
- VC++ 运行库（Flyleaf/FFmpeg 相关 DLL 可能依赖；通常随 VS/常用运行库安装包已具备）
- FFmpeg Shared Libraries（x64）：WinUI3 播放内核 Flyleaf 依赖；本仓库提供脚本自动拉取（见下）

## 一键构建（推荐：xtask）

在仓库根目录执行：

```bash
# 1) cargo build -p chaos-daemon --release
# 2) msbuild/dotnet build chaos-winui3/ChaosSeed.WinUI3.sln
cargo xtask build-winui3 --release
```

说明：
- `chaos-winui3` 的 `csproj` 会在构建完成后，把 `target/release/chaos-daemon.exe` 复制到 WinUI 3 输出目录（与 WinUI exe 同目录）。

## 运行（开发/调试）

1. 运行 `chaos-winui3` 输出目录下的 WinUI 3 可执行文件（会自动启动 `chaos-daemon.exe`）
2. 在主页输入直播间地址（URL 或带平台前缀的输入）
3. 点击“解析并进入直播”进入直播页：
   - 先解析并展示清晰度/线路卡片（含缩略图/标题/主播/状态等）
   - 点击卡片进入播放（Flyleaf）并连接右侧弹幕滚动（`用户名: 内容`；表情图片通过 `danmaku.fetchImage` 拉取）

## 运行时依赖（FFmpeg / Flyleaf）

WinUI3 播放内核切换为 Flyleaf（基于 FFmpeg/DirectX），需要在可执行文件同目录下提供 `FFmpeg/` 目录（包含 `avcodec-*.dll` 等共享库）。本仓库在 Windows 侧执行 `cargo xtask build-winui3` 时会自动运行脚本拉取并放置到 `chaos-winui3/ChaosSeed.WinUI3/FFmpeg/`，并由 `csproj` 复制到输出目录（当前默认拉取 `BtbN/FFmpeg-Builds` 的 `n8.0` win64 lgpl shared）。

> 注意：仓库默认不提交 FFmpeg DLL（`chaos-winui3/ChaosSeed.WinUI3/FFmpeg/` 初始可能只有 `.gitkeep`）。
> 如果你是直接用 Visual Studio / `dotnet build` 在 Debug 运行 WinUI3，而没跑过 `cargo xtask build-winui3`，就很容易因为缺少 FFmpeg DLL 导致进入“直播”页后初始化 Flyleaf 失败。

检查方法：输出目录里应存在 `FFmpeg\\avcodec-*.dll`（例如 `bin\\x64\\Debug\\net8.0-windows10.0.19041.0\\FFmpeg\\avcodec-*.dll`）。

如果你手动下载/替换了 FFmpeg：
- **FFmpeg DLL 的主版本号必须与 Flyleaf 绑定版本匹配**（例如 `avcodec-61.dll` vs `avcodec-62.dll`）。版本不匹配会导致 `Flyleaf 初始化失败：Loading FFmpeg libraries ... failed`。

## 常见问题

### 启动即崩溃（`Microsoft.UI.Xaml.dll` / `0xc000027b`）

这类问题通常与 WinUI 3 / Windows App Runtime 的启动自检或依赖缺失相关，建议按顺序排查：

1. 确认已安装 **Windows App Runtime 1.8 (x64)**（本仓库当前 `ChaosSeed.WinUI3.csproj` 使用 `Microsoft.WindowsAppSDK 1.8.x`）。
   - PowerShell 快速检查：`(get-appxpackage micro*win*appruntime*).PackageFullName`（应能看到 `Microsoft.WindowsAppRuntime.1.8` 的 x64 包，通常包含 Framework/Main/Singleton 等）。
2. 安装/修复 **Visual C++ Redistributable 2015-2022 x64**（VLC 与部分 WinUI 运行时依赖）。
3. 重新 `msbuild ... /restore`，确保 NuGet 依赖完整还原后再运行。
4. 若依旧崩溃：删除 `bin/`、`obj/` 后重建，避免旧的 XAML 生成物残留；并确保工程启用了自定义入口（本仓库通过 `DISABLE_XAML_GENERATED_MAIN` + `Program.cs` 在启动前调用 `XamlCheckProcessRequirements()`）。

### “解析中…” 一直不结束

如果进入直播页后一直卡在“解析中…”，通常是 `chaos-daemon.exe` 启动/连接成功但 RPC 没有返回（例如 daemon 启动失败、网络请求卡住、或依赖缺失导致 daemon 异常）。

排查顺序：
1. 确认输出目录存在 `chaos-daemon.exe`（应与 `ChaosSeed.WinUI3.exe` 同目录）。
2. 直接在该目录用命令行运行 `chaos-daemon.exe --help`，确认能正常启动（如果弹窗/退出码异常，先修复 daemon 运行环境）。
3. 确认网络/代理环境能访问对应直播站点（bilibili/douyu/huya 等）。
4. 删除 `bin/`、`obj/` 后重建，避免旧文件导致行为异常。
5. 查看 daemon 日志：`%LOCALAPPDATA%\\ChaosSeed.WinUI3\\logs\\chaos-daemon-*.log`（WinUI 会在启动 daemon 时自动写入 stdout/stderr）。

## WSL 协作建议（你当前的开发方式）

在 WSL 根目录执行同步脚本，把仓库同步到 Windows 目录（示例）：

```bash
./sync_to_win.sh
```

然后在 Windows 侧打开同步后的目录并执行：

```powershell
cargo xtask build-winui3 --release
```
