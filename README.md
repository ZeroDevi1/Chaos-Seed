# chaos-seed

一个 Windows GUI（Rust + Slint）应用 + 纯 Rust 核心（`chaos-core`）：提供字幕下载、弹幕接入与直播源解析等能力；并通过 `chaos-ffi` 以 C ABI + JSON 形式对外导出，便于 WinUI3/Qt 等调用。

## 功能

- 字幕（已完成）：Thunder 搜索 / 列表展示 / 单条下载（每次下载选择目录，支持超时与重试）
- 弹幕（已完成）：BiliLive / Douyu / Huya 连接与解析；UI 已接入（Chat / Overlay）
- 直播源解析（已完成 core/ffi）：BiliLive / Douyu / Huya 的 `manifest/variants` 解析 + `resolve_variant` 二段补全
- UI（已完成初版）：直播源解析 UI（manifest/variants）+ 新窗口播放器（Master 风格；Hls.js + Libmedia AvPlayer），支持清晰度/线路切换、直连 URL 调试显示、关闭窗口自动停止播放
- UI（后续增强）：反盗链/本地代理（Referer/UA/Cookie 注入）、播放诊断与更完善的自动重试策略、播放器观感与快捷键

## 构建前提（重要）

本仓库通过 `rust-toolchain.toml` 固定 Rust 工具链版本（当前为 `1.93.0`）。

如果你看到类似错误：

> rustc 1.85.0 is not supported by slint / i-slint-*

请在仓库根目录执行：

```bash
rustup toolchain install 1.93.0
rustup override set 1.93.0
rustc -V
```

## Workspace（双 UI）

本仓库已拆成 Cargo workspace：

- `chaos-core`：纯 Rust 核心（字幕 + 弹幕 + 直播源解析）
- `chaos-slint`：Slint UI（产物仍为 `chaos-seed` 可执行文件）
- `chaos-tauri`：Tauri v2 + Vite(TS) UI（当前主 UI 方案）
- `chaos-ffi`：C ABI 适配层（导出 `chaos-core` 为 dll/so，供 WinUI3/Qt 等调用）

## 架构（当前 / 未来）

```
Currently Strategy                         Future Strategy
------------------                         ---------------

[ chaos-core ]  <-- 纯 Rust 业务逻辑，无 UI 依赖 (通用)
      ^
      |__________________________________________
      |                    |                    |
[ chaos-slint ]      [ chaos-tauri ]      [ chaos-ffi ]  <-- 新增的适配层
   (Rust UI)           (Web UI)          (DLL 导出层)
                                                |
                                          (编译为 .dll/.so)
                                                |
                                          [ WinUI 3 App ]
                                          (C# / XAML)
```

## 构建

### Windows 原生（MSVC）

默认使用 **software renderer**（最稳，跨环境兼容最好）：

```powershell
.\scripts\build_win.ps1
```

如果你想尝试 Skia renderer（性能更好，但对 C++ 工具链更敏感）：

```powershell
.\scripts\build_win_skia.ps1
```

产物：`target\release\chaos-seed.exe`

#### Windows 上 Skia 链接失败（LNK2019 / LNK1120）

如果你遇到类似：

> skia.lib(...): unresolved external symbol __std_find_first_of_trivial_pos_1  
> fatal error LNK1120: 1 unresolved externals

这是典型的 Skia/C++ 标准库符号不匹配问题（不同 MSVC toolset / STL 版本组合可能触发）。

解决方式（推荐从上到下尝试）：

1) 直接使用 software renderer：运行 `.\scripts\build_win.ps1`
2) 更新/切换到稳定的 VS 2022 toolset（不要混用 Preview 工具链），并确保 C++ build tools 完整安装

### WSL -> Windows（GNU, mingw-w64）

（以下以 Debian/Ubuntu 为例）

```bash
sudo apt-get update
sudo apt-get install -y mingw-w64 pkg-config libfontconfig1-dev
rustup target add x86_64-pc-windows-gnu
./scripts/build_wsl_gnu.sh
```

产物：`target/x86_64-pc-windows-gnu/release/chaos-seed.exe`

说明：WSL 的交叉编译脚本默认使用 **software renderer**，更容易成功。

### WSL -> Windows（MSVC, cargo-xwin）

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libfontconfig1-dev clang lld
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin
./scripts/build_wsl_msvc_xwin.sh
```

产物路径以 cargo-xwin 输出为准（通常在 `target/x86_64-pc-windows-msvc/release/chaos-seed.exe`）。

## 渲染器切换（手动）

- Skia renderer：

```bash
cargo build -p chaos-slint --release --no-default-features --features renderer-skia
```

- Software renderer：

```bash
cargo build -p chaos-slint --release --no-default-features --features renderer-software
```

## 参考项目

参考项目在 `refs/` 下，仅作学习与对照用，已剥离其 `.git`，并在本仓库中 gitignore，不进入提交历史。

## 弹幕（调试 / CLI 验证）

本仓库已实现弹幕“功能层”（连接/解析/统一事件）并已接入 UI；同时保留 example 方便快速验证。

你可以用 example 快速验证：

```bash
cargo run -p chaos-core --example danmaku_dump -- 'https://live.bilibili.com/<RID>'
cargo run -p chaos-core --example danmaku_dump -- 'https://www.douyu.com/<RID>'
cargo run -p chaos-core --example danmaku_dump -- 'https://www.huya.com/<RID>'
```

输入也支持平台前缀：

```bash
cargo run -p chaos-core --example danmaku_dump -- 'bilibili:<RID>'
cargo run -p chaos-core --example danmaku_dump -- 'douyu:https://www.douyu.com/<RID>'
cargo run -p chaos-core --example danmaku_dump -- 'huya:<RID>'
```

事件语义（对齐 IINA+）：
- `LiveDMServer`：`text == ""` 表示连接 OK；`text == "error"` 表示失败/断线
- `SendDM`：`dms` 中包含弹幕内容；表情弹幕会带 `image_url` 与（可选）`image_width`

## Tauri（当前 UI）

仅 Rust 侧编译检查（不跑前端构建）：

```bash
cargo build -p chaos-tauri --release
```

Linux 上若缺少系统依赖（GTK/WebKit 等）会编译失败；请按 Tauri 官方文档安装依赖后再构建/运行。

前端开发运行（在 `chaos-tauri/` 下）：

```bash
pnpm install
pnpm tauri:dev
```

Windows 开发时如果遇到依赖预构建（`node_modules/.vite/deps/*`）相关报错或播放器黑屏，请先删除 `chaos-tauri/node_modules/.vite` 后重启 `tauri:dev`。

构建二进制（不打包安装器，适合 CI/快速验证）：

```bash
pnpm tauri:build:nobundle
```

## chaos-ffi（dll/so 导出）

构建：

```bash
cargo build -p chaos-ffi --release
```

文档：
- `chaos-ffi/docs/API.md`
- `chaos-ffi/docs/CSharp.md`
- `chaos-ffi/docs/BUILD.md`

Header 生成（cbindgen，Rust 内置生成器）：

```bash
cargo run -p chaos-ffi --features gen-header --bin gen_header
```

直播源解析（真实 URL 校验，运行时传参；不在仓库中写死 URL）：

```bash
cargo test -p chaos-ffi --features live-tests --test livestream_live -- \
  --bili-url <URL> --huya-url <URL> --dump-json
```
