# Chaos Seed

一个"多 UI 壳 + 纯 Rust 核心"的跨平台直播/弹幕/歌词工具。

## 项目分支结构

本项目已按前端实现拆分为多个独立分支：

| 分支 | 说明 | 技术栈 |
|------|------|--------|
| `main-core` | **核心仓库** - 包含 core/ffi/proto/daemon/app 等核心库 | Rust |
| `winui3` | WinUI3 桌面端 | C# / WinUI 3 / .NET 8 |
| `tauri` | Tauri 桌面端 | Rust / Tauri v2 / TypeScript |
| `slint` | Slint Native UI | Rust / Slint |
| `flutter` | Flutter 跨平台 | Dart / Flutter |
| `android` | Android 原生 | Kotlin / Jetpack Compose |
| `cli` | 命令行工具 | Rust / TUI |

## 核心架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         前端实现层                                │
│  winui3 / tauri / slint / flutter / android / cli              │
└──────────────────────────▲──────────────────────────────────────┘
                           │
┌──────────────────────────┴──────────────────────────────────────┐
│                         chaos-daemon                             │
│              Windows 后端：NamedPipe + JSON-RPC                  │
└──────────────────────────▲──────────────────────────────────────┘
                           │
┌──────────────────────────┴──────────────────────────────────────┐
│                          chaos-app                               │
│              应用编排层（会话/任务/缓存/事件）                      │
└──────────────────────────▲──────────────────────────────────────┘
                           │
┌──────────────────────────┴──────────────────────────────────────┐
│                          chaos-core                              │
│     纯 Rust 业务核心（字幕/弹幕/直播源/歌词/NowPlaying）          │
└─────────────────────────────────────────────────────────────────┘
```

## 功能特性

- **字幕搜索**：Thunder 搜索、列表展示、单条下载
- **弹幕系统**：BiliLive / Douyu / Huya 连接与解析；支持 Chat 窗口和 Overlay 悬浮窗
- **直播源解析**：多平台 `manifest/variants` 解析 + `resolve_variant` 二段补全
- **直播目录**：首页/分类浏览（平台 Tab + 站内搜索 + 卡片列表 + 分页）
- **歌词系统**：
  - 三源搜索（QQ 音乐 / 网易云 / LRCLIB）
  - 自动匹配阈值
  - Now Playing 检测（Windows SMTC）
  - 多窗口模式：主界面 / 停靠（Dock）/ 桌面悬浮（Float）
  - 轻量特效背景（fluid / fan3d / snow）

## 快速开始

### 克隆特定分支

```bash
# WinUI3 桌面端
git clone -b winui3 https://github.com/ZeroDevi1/Chaos-Seed.git

# Tauri 桌面端
git clone -b tauri https://github.com/ZeroDevi1/Chaos-Seed.git

# Slint Native UI
git clone -b slint https://github.com/ZeroDevi1/Chaos-Seed.git

# Flutter 跨平台
git clone -b flutter https://github.com/ZeroDevi1/Chaos-Seed.git

# Android 原生
git clone -b android https://github.com/ZeroDevi1/Chaos-Seed.git

# CLI 命令行
git clone -b cli https://github.com/ZeroDevi1/Chaos-Seed.git
```

### 构建前提

本仓库通过 `rust-toolchain.toml` 固定 Rust 工具链版本（当前为 `1.93.0`）。

```bash
rustup toolchain install 1.93.0
rustup override set 1.93.0
```

## 各分支文档

- [BUILD_WINUI3.md](BUILD_WINUI3.md) - WinUI3 构建指南
- [FFI_API.md](FFI_API.md) - FFI API 文档
- [Daemon_API.md](Daemon_API.md) - Daemon JSON-RPC API
- [DEVLOG.md](DEVLOG.md) - 开发日志

## 调试示例

### 弹幕

```bash
cargo run -p chaos-core --example danmaku_dump -- 'https://live.bilibili.com/<RID>'
```

### 直播源解析

```bash
cargo run -p chaos-core --example livestream_dump -- --input 'https://live.bilibili.com/<RID>'
```

### 字幕搜索

```bash
cargo run -p chaos-core --example subtitle_search -- --query 'Dune' --limit 10
```

### 歌词搜索

```bash
cargo run -p chaos-core --example lyrics_search -- --title "Hello" --artist "Adele"
```

## 版本号同步

所有分支版本号与 `main-core` 核心库保持一致，通过 Git 子树或手动同步更新。

## 贡献

请根据目标平台选择对应分支进行开发：
- Windows 原生体验 → `winui3`
- 跨平台桌面 → `tauri` 或 `slint`
- 移动端 → `flutter` 或 `android`
- 终端/脚本 → `cli`

## 许可证

MIT License
