# 多壳 Rust 分层地图

## 目标

把“能力源头、应用编排、跨边界 DTO、ABI 适配、传输适配、UI 壳”拆开，让每层都只有一个主要职责。

## 通用依赖方向

- `*-core`
  - 业务核心与可复用能力源头
  - 不应依赖 UI、FFI、daemon
- `*-proto`
  - 独立 DTO / 方法名 / 事件形状
  - 不应依赖 orchestration/app、UI、传输实现
- `*-app` 或 `*-orchestration`
  - 依赖 `*-core`，必要时依赖 `*-proto`
  - 持有会话、订阅、任务、缓存、事件聚合
- `*-ffi`
  - 依赖 `*-core`，必要时依赖 `*-proto`
  - 暴露稳定 C ABI + JSON 边界
- `*-daemon` 或 `*-server`
  - 依赖 orchestration/app、`*-core`，必要时依赖 `*-proto`
  - 暴露 JSON-RPC、stdio、NamedPipe、socket 或其他传输协议
- UI crates / 壳工程
  - 进程内 UI 壳优先直连 Rust 核心
  - 仓外壳层消费者按场景走 daemon 或 FFI

## 每层该放什么

- `core`
  - 纯算法、纯规则、纯模型、纯数据变换
  - 可被多个壳复用的能力实现
- `app`
  - 长生命周期状态机
  - 后台任务、订阅、缓存、会话、事件编排
- `proto`
  - 稳定请求参数、返回值、通知事件、错误形状
- `ffi`
  - `extern "C"` 函数
  - UTF-8 字符串、内存所有权、回调桥接
- `daemon`
  - 鉴权、传输 framing、请求分发、会话桥接、通知推送
- UI
  - 状态投影、交互反馈、窗口体验、主题与布局

## 命名建议

- 把这些名字当作“角色名”而不是“必须照抄的 crate 名”
- 新项目优先使用符合自身语义的名字，例如：
  - `media-core`
  - `media-orchestration`
  - `media-proto`
  - `media-ffi`
  - `media-daemon`
- 只要职责边界一致，就不必沿用某个历史项目的前缀

## 允许与禁止

- 允许 `app` 编排 `core`
- 允许 `ffi` 和 `daemon` 暴露同一份核心能力
- 允许 UI 壳按模块选择进程内、FFI 或 daemon
- 不要让 `ffi` 或 `daemon` 成为业务逻辑源头
- 不要让 UI 页面自己发明 DTO 或方法名
- 不要让 `core` 反向依赖 `proto` 作为运行时前提，除非确实在抽象边界上无法避免
- 不要把会话、订阅和后台任务塞回 UI 或 `core`

## Chaos-Seed 示例

- `chaos-tauri/src-tauri` 直接调用 `chaos_core::subtitle`、`lyrics`、`now_playing`
- `chaos-daemon` 通过 `chaos-proto` 暴露 `daemon.ping`、`lyrics.search`、`tts.sft.*` 等方法
- `chaos-ffi` 统一使用 C ABI + JSON，暴露 `char *` 并要求调用方释放
- `chaos-flutter` 在 Windows 端可按模块选择 FFI 或 daemon，在 Android 端优先 FFI

## 常见反模式

- 为了让某个 UI 先跑通，把业务规则直接写进窗口层
- 因为某个 daemon 方法急用，就在 `daemon` 里复制一份核心逻辑
- 因为 FFI 快捷，就让 `ffi` 自己拥有会话和状态机
- 因为 DTO 要跨边界，就让 `proto` 反过来主导核心算法设计
- 因为现有 `*-core` 已经存在，就把所有新能力都直接堆进去，不再区分纯算法层和适配层
