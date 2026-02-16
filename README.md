# chaos-seed

一个“多 UI 壳 + 纯 Rust 核心”的项目：核心能力集中在 `chaos-core`（字幕 / 弹幕 / 直播源 / 歌词等），Windows 可选 **WinUI 3 原生 UI（`chaos-winui3`）** 通过 **NamedPipe + JSON-RPC（LSP Content-Length framing）** 与 `chaos-daemon` 通讯；同时保留 **Tauri（`chaos-tauri`）** 作为跨平台备选 UI。`chaos-ffi` 作为独立导出路线保留，但不参与本次 IPC 主链路。

## 功能

- 字幕（已完成）：Thunder 搜索 / 列表展示 / 单条下载（每次下载选择目录，支持超时与重试）
- 弹幕（已完成）：BiliLive / Douyu / Huya 连接与解析；UI 已接入（Chat / Overlay）
- 直播源解析（已完成 core/ffi）：BiliLive / Douyu / Huya 的 `manifest/variants` 解析 + `resolve_variant` 二段补全
- 直播目录（已完成 WinUI3/FFI/Daemon）：首页/分类（平台 Tab + 站内搜索 + 卡片列表 + 分页）；点卡片跳转“直播”页，仅解析清晰度列表（不自动播放）
- 歌词（已完成增强）：对齐 BetterLyrics 三源（QQ 音乐 / 网易云 / LRCLIB），按“顺序 + 匹配阈值”自动搜索；读取系统 Now Playing（Windows SMTC snapshot 自适应轮询）并推送时间轴事件；支持主界面 + 停靠（Dock）+ 桌面悬浮（Float），暂停自动隐藏；支持轻量特效背景（fluid / fan3d / snow）；提供 Tauri 托盘开关“歌词检测”（旧 Chat/Overlay 窗口保留作调试/兼容）
- 歌曲下载（已接入）：QQ 音乐（QQ/微信扫码登录、刷新 Cookie、音质选择）；酷狗/网易云（可配置 baseUrl）；酷我；支持按单曲/专辑/歌手搜索与批量下载；下载任务由 daemon 执行并可轮询进度/取消
- UI（已完成初版）：直播源解析 UI（manifest/variants）+ 新窗口播放器（Master 风格；Hls.js + Libmedia AvPlayer），支持清晰度/线路切换、直连 URL 调试显示、关闭窗口自动停止播放
- UI（后续增强）：反盗链/本地代理（Referer/UA/Cookie 注入）、播放诊断与更完善的自动重试策略、播放器观感与快捷键
- WinUI 3（PoC 已实现）：新增“首页/分类/直播/弹幕/歌词/歌曲”导航；歌曲页支持搜索、扫码登录、选择音质下载、下载队列与进度轮询/取消；其余页面见现有说明

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

## Workspace（多 UI）

本仓库已拆成 Cargo workspace：

- `chaos-core`：纯 Rust 核心（字幕 + 弹幕 + 直播源解析）
- `chaos-proto`：IPC 协议与 DTO（JSON-RPC 方法/参数/返回/事件）
- `chaos-app`：应用编排层（会话/任务/缓存/事件，**无 UI 依赖**；当前主要供 daemon 使用）
- `chaos-daemon`：Windows 后端进程（NamedPipe + JSON-RPC），供 WinUI3 连接
- `chaos-slint`：Slint UI（产物仍为 `chaos-seed` 可执行文件）
- `chaos-tauri`：Tauri v2 + Vite(TS) UI（跨平台备选 UI；Rust 后端目前仍直接调用 `chaos-core`）
- `chaos-ffi`：C ABI 适配层（导出 `chaos-core` 为 dll/so，供 Qt/其他语言调用；与 IPC 主链路解耦）
- `xtask`：一键构建编排（Windows 上可一条命令构建 daemon + WinUI3）

另外：
- `chaos-winui3`：WinUI 3（C# / XAML）工程目录（不由 cargo 编译，由 `xtask` 调用 MSBuild/dotnet 构建）

## 架构（当前）

关键点：
- `chaos-core` 作为纯 Rust 核心能力库（持续扩展）；对外 JSON 形状以 `chaos-proto` 为准
- WinUI3 与 Rust 默认走 **IPC**（NamedPipe + JSON-RPC 2.0 + LSP framing）；同时保留 **可选 FFI 后端**（`chaos_ffi.dll`，便于调试/兜底）

```
                    (独立导出路线，不参与 IPC 主链路)
        ┌───────────────────────────────────────────────┐
        │                   chaos-ffi                    │
        │           C ABI + JSON 导出（dll/so）            │
        └────────────────────────▲──────────────────────┘
                                 │

┌────────────────────────────────┴────────────────────────────────┐
│                           chaos-core                              │
│        纯 Rust 业务核心（字幕/弹幕/直播源/歌词/NowPlaying…）         │
└────────────────────────────────▲────────────────────────────────┘
                                 │
┌────────────────────────────────┴────────────────────────────────┐
│                           chaos-app                               │
│      应用编排层（会话/订阅/后台任务/缓存/事件建议；无 UI 依赖）       │
└────────────────────────────────▲────────────────────────────────┘
                                 │
                    out-of-process（Windows IPC）
                                 │
┌────────────────────────────────┴────────────────────────────────┐
│                          chaos-daemon                             │
│     Windows 后端进程：NamedPipe + JSON-RPC（LSP Content-Length）    │
└────────────────────────────────▲────────────────────────────────┘
                                 │
┌────────────────────────────────┴────────────────────────────────┐
│                         chaos-winui3                              │
│                        WinUI 3（C# / XAML）                        │
└─────────────────────────────────────────────────────────────────┘

in-process（现状不改，跨平台备选 UI）
┌─────────────────────────────────────────────────────────────────┐
│                         chaos-tauri                               │
│         Tauri v2 + Web UI（目前 Rust 侧仍直接调用 chaos-core）       │
└─────────────────────────────────────────────────────────────────┘
```

## 构建

### Tauri

```bash
cd chaos-tauri
pnpm run tauri:build:nobundle
```

### WinUI 3（新增 PoC）

文档：`docs/BUILD_WINUI3.md`

Windows 上一键构建：

```bash
cargo xtask build-winui3 --release
```

### Flutter（chaos-flutter）

文档：`docs/BUILD_CHAOS_FLUTTER.md`

### 弹幕 Overlay 透明悬浮窗（Win11）

WinUI3 的 XAML `Transparent` 在 Win11 上通常无法做到“桌面真透”，所以本项目的弹幕页 “打开 Overlay 悬浮窗” 默认使用 **Win32 layered window** 实现真透明（只绘制文字/表情图，背景透出桌面/游戏画面）。

交互：
- `Esc`：关闭 Overlay
- `F2`：切换模式（`LOCK`/`EDIT`）
  - `LOCK`：不移动（除顶栏按钮），内容区域鼠标穿透；边缘/角落仍可 resize
  - `EDIT`：可拖动顶栏移动窗口；可 resize
- 顶栏右上角 `X`：关闭 Overlay
- 双击顶栏：切换 `LOCK/EDIT`

备注：
- 表情图可能是 WebP：Overlay 内部使用 WinRT `BitmapDecoder` 解码（Win11 支持）。
- 独占全屏游戏通常无法被普通 topmost 窗口覆盖；建议使用无边框窗口化/窗口化。

### 渲染器切换（手动）

- Skia renderer：

```bash
cargo build -p chaos-slint --release --no-default-features --features renderer-skia
```

- Software renderer：

```bash
cargo build -p chaos-slint --release --no-default-features --features renderer-software
```

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

## 直播源解析（调试 / CLI 验证）

```bash
cargo run -p chaos-core --example livestream_dump -- --input 'https://live.bilibili.com/<RID>'
cargo run -p chaos-core --example livestream_dump -- --input 'douyu:<RID>' --dump-json
```

二段解析（补齐特定清晰度的 URL）：

```bash
cargo run -p chaos-core --example livestream_dump -- --input 'huya:<RID>' --resolve '<variant_id>'
```

## 字幕搜索（调试 / CLI 验证）

```bash
cargo run -p chaos-core --example subtitle_search -- --query 'Dune' --limit 10
cargo run -p chaos-core --example subtitle_search -- --query 'Dune' --lang zh --min-score 8.0 --dump-json
```

## 歌词（调试 / CLI 验证）

```bash
cargo run -p chaos-core --example lyrics_search -- --title "Hello" --artist "Adele"
```

输出 JSON（便于复制到 UI/调试）：

```bash
cargo run -p chaos-core --example lyrics_search -- --title "Hello" --artist "Adele" --album "Hello" --limit 5 --strict --services qq,netease,lrclib --timeout-ms 10000 --dump-json
```

## 歌词系统（BetterLyrics 对齐）

- 默认在线源：QQ 音乐 / 网易云 / LRCLIB（按 providers_order 顺序逐个尝试，命中 `matching_threshold` 直接停止）
- 匹配分数：`match_percentage (0~100)`，用于自动阈值判断与 UI 展示
- 播放事件：后端推送 `now_playing_state_changed`（含 position/duration/retrieved_at），前端插值推进；Dock/Float 打开且 playing 时才进行低频 resync
- 显示模式：
  - Dock：贴边侧边栏歌词
  - Float：桌面悬浮歌词挂件（默认可交互，`F2` 切换点击穿透，`Esc` 关闭）
- 智能行为：暂停后按延迟自动隐藏；继续播放自动恢复（仅恢复“自动隐藏”导致的隐藏，不强行打开用户主动关闭的窗口）
- 托盘：一键开关“歌词检测”，并可打开主界面 / Dock / Float

## Windows Tag Release（GitHub Actions）

仓库包含 tag 触发的 Windows Release 构建工作流：push 一个 semver tag（例如 `0.2.0` / `v0.2.0`）后自动构建并上传 Release 产物：

- `chaos-ffi-windows-x86_64.zip`
- `chaos-winui3-windows-x86_64.zip`
- `chaos-winui3-windows-x86_64.zip.sha256`（WinUI3 zip 自更新校验）
- `chaos-tauri-windows-x86_64.zip`

WinUI3 额外支持：
- zip 便携版自更新（见 `docs/WINUI3_AUTO_UPDATE.md`）
- 可选 MSIX + AppInstaller 更新通道（需要配置 CI 签名证书）

推送示例（创建 tag 后一定要 push 到远端，CI 才会触发）：

```bash
# 1) 先确保要打 tag 的提交已在远端（可选，但推荐）
git push origin main

# 2) 创建并推送 tag（建议使用 annotated tag，便于 --follow-tags）
git tag -a "v0.2.0" -m "v0.2.0"
git push origin "v0.2.0"

# 或者：一次性推送提交 + 相关 annotated tags
git push --follow-tags
```

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
