# BUILD_WINUI3（Windows / WinUI 3 + chaos-daemon）

> 本文档用于在 Windows 上构建与运行 `chaos-winui3`（WinUI 3 UI）与 `chaos-daemon`（Rust NamedPipe JSON-RPC 后端）。
> 本仓库在 WSL 内开发时，推荐先同步到 Windows 文件系统后再在 Windows 侧执行构建。

## 依赖（Windows）

- Rust toolchain：仓库根 `rust-toolchain.toml` 固定（当前为 `1.93.0`）
- .NET SDK：`.NET 8`
- Visual Studio Build Tools（包含 MSBuild）
- Windows SDK（WinUI 3 需要）
- Windows App SDK Runtime（WinUI 3 需要）
- VC++ 运行库（VLC 播放器依赖；通常随 VS/常用运行库安装包已具备）

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
   - 左侧播放：默认使用 **VLC（LibVLCSharp）**，以覆盖多数 FLV 直播源（可在设置里切换“系统播放器（为 FFmpegInteropX 预留）”）
   - 右侧弹幕：实时滚动（`用户名: 内容`；表情图片通过 `danmaku.fetchImage` 拉取）

## 许可与分发提示（VLC）

WinUI3 默认使用 `LibVLCSharp` + `VideoLAN.LibVLC.Windows` 分发 libVLC，通常体积会明显增大；同时需要注意 VLC / libVLC 的 **LGPL** 许可要求（例如动态链接与替换权利等）。PoC 阶段建议先验证体验与稳定性，后续再做发布形态与合规梳理。

## 常见问题

### 启动即崩溃（`Microsoft.UI.Xaml.dll` / `0xc000027b`）

这类问题通常与 WinUI 3 / Windows App Runtime 的启动自检或依赖缺失相关，建议按顺序排查：

1. 确认已安装 **Windows App Runtime 1.6 (x64)**（与你的 `Microsoft.WindowsAppSDK 1.6.x` 对应）。
2. 安装/修复 **Visual C++ Redistributable 2015-2022 x64**（VLC 与部分 WinUI 运行时依赖）。
3. 重新 `msbuild ... /restore`，确保 NuGet 依赖完整还原后再运行。
4. 若依旧崩溃：删除 `bin/`、`obj/` 后重建，避免旧的 XAML 生成物残留；并确保工程启用了自定义入口（本仓库通过 `DISABLE_XAML_GENERATED_MAIN` + `Program.cs` 在启动前调用 `XamlCheckProcessRequirements()`）。

## WSL 协作建议（你当前的开发方式）

在 WSL 根目录执行同步脚本，把仓库同步到 Windows 目录（示例）：

```bash
./sync_to_win.sh
```

然后在 Windows 侧打开同步后的目录并执行：

```powershell
cargo xtask build-winui3 --release
```
