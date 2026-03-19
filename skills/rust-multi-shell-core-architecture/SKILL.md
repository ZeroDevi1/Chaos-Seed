---
name: rust-multi-shell-core-architecture
description: 为多 crate Rust 工作区提供“纯核心 + 编排层 + 协议层 + FFI 适配层 + daemon 传输层 + 多 UI 壳”分层与接入指导。用于决定新功能应放在 core crate、orchestration/app crate、proto crate、ffi crate、daemon crate 还是 UI 壳，选择 Slint/Tauri 进程内接入、WinUI3/Flutter/外部语言通过 FFI 或 daemon 接入，以及准备把部分逻辑抽成 alloc-friendly 共享 crate 供 STM32/ESP32 Rust HAL 复用时。
---

# Rust Multi-Shell Core Architecture

## Overview

用这个 Skill 为多壳 Rust 工作区决定功能落位、边界稳定化和接入路线。
默认沿用一套可迁移策略：业务规则收敛到核心，跨边界形状收敛到协议，ABI 和传输层只做适配，不复制业务。

## Workflow

1. 先把任务归类为“新功能落位”“跨壳接入”“抽核给 MCU 共享”三类之一。
2. 先回答“哪一层拥有这段逻辑”，再回答“哪些层只暴露它”。
3. 只在跨进程、跨语言或跨仓边界需要稳定化时新增 `proto`、`ffi` 或 `daemon`。
4. 涉及 MCU 共享时，额外判断是否应从现有应用核心 crate 中抽出新的 dependency-light crate，而不是继续堆到原有大核心 crate。
5. 输出结论时固定按“放哪层 -> 为什么不放别层 -> 是否需要 proto/ffi/daemon -> 是否应抽新 crate”组织。

## Place Logic In The Right Layer

- 把纯算法、纯规则、纯模型、纯数据变换放进 `core` 风格层；优先让这部分不感知 UI、IPC、ABI 和平台壳。
- 把长生命周期状态、后台任务、订阅、缓存、事件聚合和会话管理放进 `app` 风格层；让它编排核心能力，但不要让 UI 直接拥有这些状态机。
- 把跨进程稳定 DTO、方法名、通知事件和错误形状放进 `proto`；不要让传输实现各自发明一套 JSON 结构。
- 把 `ffi` 仅当作稳定 ABI 适配层；让它负责 `extern "C"`、字符串/内存所有权、JSON 边界和回调桥接，不承载业务决策。
- 把 `daemon` 仅当作传输适配层；让它负责鉴权、会话桥接、请求分发、通知推送和 framing，不复制核心逻辑。
- 把 UI 壳限制在状态投影、交互和平台体验；不要在 Slint、Tauri、WinUI3、Flutter 页面里偷塞业务规则或协议发明。

## Choose The Transport

- 对 Rust in-process 壳，默认优先直连核心；当前仓库中 Slint 和 Tauri 更接近这条路线。
- 对 WinUI3、Flutter Windows、Qt、C#、其他语言壳，先判断是否需要跨语言直接嵌入；需要时优先考虑 `ffi`。
- 对需要独立进程、统一后台、流式通知、鉴权、可重启恢复或多客户端共享状态的场景，优先考虑 `daemon`。
- 如果只是同步请求/返回、调用面窄、宿主进程愿意承担崩溃或加载成本，优先考虑 `ffi`，不要先上 `daemon`。
- 如果一个壳既要直接调用少量同步能力，又要共享后台任务或事件流，可以按模块混用 FFI 与 daemon，但仍保持 DTO 和能力源头唯一。

## Keep MCU Reuse Possible

- 不要默认把整个应用核心 crate 视为 MCU 可复用核心；先区分其中的纯算法子层和桌面、网络、OS 子层。
- 默认把未来 MCU 共享目标设计成 `alloc` 优先、尽量 `no_std-friendly`；允许 `Vec`、`String` 等分配，但避免偷带 `tokio`、`reqwest`、`windows`、文件系统、子进程。
- 把时间、随机、网络、日志、存储、音频 I/O 通过 trait 或窄接口注入；不要把桌面运行时硬编码进共享核心。
- 把 `serde`、`std`、平台 API 和重依赖放到 feature gate 或外层适配 crate；不要让协议 DTO 反向污染纯算法 crate。
- 当一段逻辑同时服务桌面与 MCU 时，优先新建更小的共享 crate，再让应用核心 crate 与 HAL 侧共同依赖它。

## Respond In This Style

- 先直接给出“应放哪层”的结论，不先做框架比较。
- 同时说明“为什么不该放在相邻层”，避免实现时继续摇摆。
- 涉及跨边界时，明确回答是否要新增 `proto`、是否要开 `ffi`、是否要补 `daemon` 方法或事件。
- 涉及 MCU 共享时，明确回答“继续留在现有核心 crate”还是“抽新 crate”，并指出必须留在桌面适配层的依赖。
- 如果任务落在现有 UI 壳，顺手指出它应走进程内、FFI 还是 daemon，而不是只讨论 crate 边界。

## References

- 需要看当前工作区的分层职责、依赖方向和反模式时，读取 `references/architecture-map.md`
- 需要在 in-process、FFI、daemon 之间做选择时，读取 `references/transport-selection.md`
- 需要判断哪些逻辑可抽给 STM32/ESP32 Rust HAL 复用时，读取 `references/embedded-sharing.md`
