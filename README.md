# Chaos Seed

macOS 原生直播源解析应用，复刻 Chaos-Seed（Rust 核心）在 BiliLive / Douyu / Huya 三大平台的目录浏览与直播源解析能力，并调用本地 IINA 播放。

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
├── Networking/     LiveKit 协议 + MockLiveKit（真实网络层后续替换）
├── Parsing/        InputParser（移植 chaos_core::danmaku::sites::parse_target_hint）
├── Player/         IINA 桥（NSWorkspace / Process 骨架）
├── Theme/          平台徽章配色与主题
├── Components/     SwiftUI 共享组件
└── Features/       Home / Detail / History / Settings 页面
```

## 状态

第 1 版：SwiftUI 全屏骨架 + Mock 数据层（移植自原型 `data.jsx`）。真实接口解析、SwiftData 持久化、IINA 实际拉起为后续迭代。
