# chaos-seed

一个 Windows GUI（Rust + Slint）小工具：在迅雷字幕接口中搜索字幕，并支持逐条下载到你选择的目录（每次下载都会弹出目录选择）。

## 功能

- 左侧侧边栏：Home / 字幕下载 / 直播源 / 弹幕 / Settings / About
- 字幕下载流程：搜索 -> 列表展示 -> 点击单条“下载” -> 选择目录 -> 下载
- 弹幕：BiliLive / Douyu / Huya 连接与解析，UI 已接入（Chat / Overlay）
- 业务逻辑纯 Rust（不调用 Python）

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
cargo build --release --no-default-features --features renderer-skia
```

- Software renderer：

```bash
cargo build --release --no-default-features --features renderer-software
```

## 参考项目

参考项目在 `refs/` 下，仅作学习与对照用，已剥离其 `.git`，并在本仓库中 gitignore，不进入提交历史。

## 弹幕（调试 / CLI 验证）

本仓库已实现弹幕“功能层”（连接/解析/统一事件）并已接入 UI；同时保留 example 方便快速验证。

你可以用 example 快速验证：

```bash
cargo run --example danmaku_dump -- 'https://live.bilibili.com/47867'
cargo run --example danmaku_dump -- 'https://www.douyu.com/9999'
cargo run --example danmaku_dump -- 'https://www.huya.com/660000'
```

输入也支持平台前缀：

```bash
cargo run --example danmaku_dump -- 'bilibili:47867'
cargo run --example danmaku_dump -- 'douyu:https://www.douyu.com/9999'
cargo run --example danmaku_dump -- 'huya:660000'
```

事件语义（对齐 IINA+）：
- `LiveDMServer`：`text == ""` 表示连接 OK；`text == "error"` 表示失败/断线
- `SendDM`：`dms` 中包含弹幕内容；表情弹幕会带 `image_url` 与（可选）`image_width`
