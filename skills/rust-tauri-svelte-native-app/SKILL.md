---
name: rust-tauri-svelte-native-app
description: 为 Rust + Tauri + Svelte + TypeScript Native 应用提供架构、组件、样式和桥接指导。用于从零搭建或迭代现有 Tauri 桌面项目，尤其适合页面新增、主题统一、Fluent 组件选型、多窗口、特殊窗口、前后端 command 或 event 桥接、原生感 UI 调整等任务。优先采用 Fluent Web Components 承担原生感控件，CSS 变量和自定义 Svelte 布局负责应用壳层、页面结构与窗口差异。
---

# Rust Tauri Svelte Native App

## Overview

用这个 Skill 为 Rust + Tauri + Svelte + TypeScript Native 应用设计或扩展前端目录、组件策略、主题同步和 Rust 桥接。
默认沿用一套稳定策略：Fluent Web Components 负责导航、表单和系统感控件，CSS token + Svelte 组件负责页面壳层、多窗口布局和视觉统一。

## Workflow

1. 先判断任务是在“新建应用”还是“改造现有 Tauri UI”。
2. 新建时先定 `src-tauri` 与 `src` 的职责，再定窗口类型、主题源和组件策略。
3. 迭代时先定位入口文件、窗口解析、store、主题初始化和 command 边界。
4. 做整体结构决策前，优先读取 `references/architecture-and-layout.md`。
5. 做组件或样式决策前，优先读取 `references/component-and-styling.md`。
6. 合并前用 `references/checklist-and-anti-patterns.md` 复核主题、窗口、Fluent token 和桥接边界。

## Structure The App

- 把 Tauri Rust 能力放在 `src-tauri/`，把 Svelte UI 放在 `src/`。
- 把 `src/` 至少拆成 `app/`、`shared/`、`stores/`、`ui/`。
- 把 `app/` 用于页面、布局、路由和窗口专属 UI。
- 把 `shared/` 用于跨页面 API、DTO、窗口解析、桥接工具和纯逻辑。
- 把 `stores/` 用于偏用户态和跨窗口共享状态，例如主题、窗口存在性、用户偏好。
- 把 `ui/` 用于 Fluent 注册、早期主题应用、原子级通用 UI 辅助。

## Choose Components And Styling

- 优先让 Fluent Web Components 处理：按钮、文本输入、数字输入、下拉框、工具栏、树形导航、菜单、提示层、骨架屏。
- 优先让自定义 Svelte + CSS 处理：应用壳层、侧栏宽度动画、结果列表、卡片排版、页面容器、Overlay 画布、复杂视觉特效。
- 只注册实际使用到的 Fluent 组件，避免无意义的启动和运行时开销。
- 让 `style.css` 或等价全局样式成为壳层 token 映射层，不要把主题变量散落到每个页面组件。
- 把 `html[data-theme=...]` 当作页面壳层主题入口，把 Fluent design tokens 当作控件层主题入口。

## Handle Theme And Window Modes

- 在应用挂载前先应用早期主题，避免白屏闪烁或二级窗口缺主题变量。
- 主题至少区分 `light`、`dark`、`system` 三种模式。
- 如果支持系统 accent，失败时保留稳定回退色，不要让主题初始化失败阻断启动。
- 对 Overlay、Player、Dock、透明窗口单独定义背景和 backdrop 规则，不要直接套主窗口表面色。
- 多窗口应用先定义窗口类型枚举和 `resolveView` 规则，再决定每个窗口挂载哪个 Svelte 入口。

## Bridge Frontend And Rust

- 把 Tauri command 当作能力边界，不要让前端直接猜 Rust 内部状态。
- 把 `invoke`、`listen`、窗口解析、DTO 适配收敛到 `shared/`，不要在页面组件里到处散落桥接细节。
- 把“当前窗口是谁、该挂什么视图、能不能用 backdrop”这类规则做成独立模块。
- 在 `src-tauri` 中让 command 名称、参数和返回结构稳定、明确、可测试。

## Respond In This Style

- 默认给出基于 Fluent + CSS token 的落地方案，而不是泛泛前端框架比较。
- 如果用户只问页面样式，也要同时检查它会不会影响窗口分类、主题同步和桥接边界。
- 输出方案时优先说明哪些部分应该是 Fluent，哪些部分应该继续自定义。
- 如果任务涉及多窗口，优先列出窗口表、入口表和共享状态表。

## References

- 需要目录边界、窗口入口和前后端职责时，读取 `references/architecture-and-layout.md`
- 需要决定 Fluent 与自定义 CSS/Svelte 的分工时，读取 `references/component-and-styling.md`
- 需要做落地前复核、排查主题闪烁、token 失效或多窗口规则混乱时，读取 `references/checklist-and-anti-patterns.md`
