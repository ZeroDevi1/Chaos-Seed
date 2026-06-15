# Chaos Seed

macOS 26 原生直播源解析应用，复刻 Chaos-Seed（Rust 核心）在 BiliLive / Douyu / Huya 三大平台的目录浏览与直播源解析能力，支持 AVFoundation HLS、WebKit HTTP-FLV、本地 IINA 与 BiliLive 实时弹幕。

- 设计文档：[`docs/superpowers/specs/2026-06-13-chaos-seed-macos-design.md`](docs/superpowers/specs/2026-06-13-chaos-seed-macos-design.md)
- 高保真原型：[`designs/ChaosSeed-macOS/`](designs/ChaosSeed-macOS/)
- Rust 协议蓝本：[github.com/ZeroDevi1/Chaos-Seed](https://github.com/ZeroDevi1/Chaos-Seed) `main` 分支 `chaos-core`（`live_directory` + `livestream` 模块）

## 运行

```bash
swift run              # 编译并启动
swift build            # 仅编译
swift test             # 运行单元测试
```

使用 Xcode 打开：直接双击 `Package.swift`。

## 目录结构

```
Sources/ChaosSeedApp/
├── App/            应用入口、根视图、全局状态（Toast/主题）
├── Models/         对齐 Rust chaos-core 的数据模型（Site/LiveCategory/LiveRoomCard/LiveManifest/StreamVariant/PlaybackHints）
├── Networking/     LiveKit 协议、三平台真实解析与 BiliLive 弹幕协议
├── Parsing/        InputParser（移植 chaos_core::danmaku::sites::parse_target_hint）
├── Player/         AVFoundation / WebKit 双后端内置播放器与 IINA 桥
├── Theme/          平台徽章配色与主题
├── Components/     SwiftUI 共享组件
└── Features/       Home / Detail / History / Settings 页面
```

## 状态

当前版本：真实目录与直播源解析、首页状态记忆、macOS 26 Liquid Glass、三平台内置播放（HLS / HTTP-FLV）、IINA 与 BiliLive 实时弹幕。

HTTP-FLV 后端内置 [mpegts.js 1.8.0](https://github.com/xqq/mpegts.js/releases/tag/v1.8.0)，
按 Apache-2.0 许可证分发；P2P `.xs` 线路仍需外部播放器。
