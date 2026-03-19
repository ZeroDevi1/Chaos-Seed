# Rust + Tauri + Svelte 架构与目录模式

## 目标

把 Rust 原生能力、窗口生命周期和 Svelte UI 分清边界，让多窗口 Native 应用仍然保持可维护。

## 推荐目录

```text
src/
  app/
  shared/
  stores/
  ui/
  main.ts
  style.css
src-tauri/
  src/
  tauri.conf.json
```

## 分层职责

- `src-tauri/`
  - 提供 command、event、窗口创建、原生特效和系统能力。
- `src/app/`
  - 放页面、布局、窗口专属入口组件和路由装配。
- `src/shared/`
  - 放 DTO、API 包装、窗口解析、桥接辅助和纯逻辑。
- `src/stores/`
  - 放偏好设置、主题状态、跨窗口共享状态。
- `src/ui/`
  - 放 Fluent 注册、早期主题应用、禁用缩放之类的 UI 级基础设施。
- `style.css`
  - 放全局设计 token、应用壳层和跨页面公用样式。

## 推荐启动顺序

1. 先应用早期主题。
2. 再初始化 Fluent 组件与 token。
3. 再解析当前窗口类型。
4. 再安装主题同步、跨窗口同步和窗口状态监听。
5. 最后挂载对应的 Svelte 入口。

## 多窗口模式

- 先定义 `BootView` 或等价窗口类型枚举。
- 让窗口解析逻辑独立存在，不要散落在每个页面组件里。
- 主窗口、聊天窗、Overlay、Player、Dock、Float 往往不是同一套背景和交互规则。
- 窗口共享状态优先进入 `stores/`，窗口专属状态留在对应入口组件。

## 前后端职责

- Rust：系统能力、原生窗口、网络与解码重活、命令边界。
- Svelte：页面编排、状态投影、交互反馈、视觉层。
- `shared/`：两边之间的窄桥，不承担页面视觉。
