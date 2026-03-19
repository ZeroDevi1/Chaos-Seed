---
name: rust-slint-native-app
description: 为 Rust + Slint Native 桌面应用提供架构、主题、组件与 Rust 绑定指导。用于从零搭建或迭代现有 Slint 项目，尤其适合页面新增、主题统一、组件选型、多窗口、状态与模型绑定、Overlay 或透明窗口、Windows 原生感 UI 调整等任务。优先采用 AppTheme 统一设计令牌、自定义基础组件承载外观，在文本输入等复杂交互上按需混用 std-widgets。
---

# Rust Slint Native App

## Overview

用这个 Skill 为 Rust + Slint Native 应用设计或扩展页面结构、主题系统、组件分层和 Rust 侧绑定方式。
默认沿用一套稳定策略：`AppTheme` 负责视觉令牌，自定义组件负责壳层外观，`std-widgets` 只承担 Slint 内建控件更擅长的复杂交互。

## Workflow

1. 先判断任务是在“新建应用”还是“迭代现有项目”。
2. 新建时先定义 `theme -> component -> view/window -> rust binding` 的顺序，不要先堆页面细节。
3. 迭代时先定位现有主题入口、主窗口、子窗口和 Rust 绑定层，再决定是复用还是补新组件。
4. 做任何 UI 方案前，优先读取 `references/architecture-and-layout.md`。
5. 做组件选型或样式统一时，优先读取 `references/component-and-styling.md`。
6. 落地前用 `references/checklist-and-anti-patterns.md` 复核主题同步、模型更新和多窗口一致性。

## Structure The App

- 优先把 Slint UI 分成 `app/window/view/component/model/theme` 六层。
- 把路由级或页级编排放在 `app` 或主窗口文件，不要把整站逻辑塞进单个 view。
- 把可复用视觉原语放在 `components/`，例如按钮、侧栏、字段标签、面板壳层。
- 把纯展示页面放在 `views/`，让页面只组合组件与属性，不承载复杂副作用。
- 把多窗口单独建 `windows/`，不要把 Overlay、Chat、Dock 之类的窗口混成页面条件分支。
- 把共享数据结构和 UI 行模型抽到 `models.slint` 或等价文件，避免每个 view 自己定义一份。
- 把颜色、边框、按钮状态、输入框状态统一收敛到 `theme.slint` 的全局对象。

## Choose Components And Styling

- 优先自定义这些组件：按钮、侧栏、卡片、信息面板、结果列表、页面标题、分隔条、简易切换器。
- 优先复用 `std-widgets` 处理这些能力：文本输入、光标/选区、IME、键盘编辑、原生文本行为。
- 如果 `std-widgets` 和自定义主题混用，始终同步 `Palette`，否则会出现浅色页面 + 深色输入框之类的割裂感。
- 让 `AppTheme` 成为唯一视觉真相源，不要在各个组件里散落硬编码颜色。
- 先定义语义令牌，再写组件状态。推荐至少覆盖：`app_bg`、`panel_bg`、`sidebar_bg`、`card_bg`、`hover_bg`、`selected_bg`、`text_*`、`border_*`、`button_*`。
- 只在必须依赖 Slint 内建可用性时让 `std-widgets` 主导外观，其余情况下优先保持自定义壳层一致性。

## Bind Rust To Slint

- 从 Rust 暴露最小而明确的属性、模型和 callback，不要把业务流程写进 `.slint`。
- 对列表和表格优先保持稳定模型实例，只更新内容，不频繁替换模型指针。
- 用 `Weak` / `as_weak()` 持有窗口句柄，避免闭包和定时器把窗口生命周期绑死。
- 把 UI 线程更新集中到统一消息泵或统一回调层，避免多个后台线程直接竞争 UI 状态。
- 在 callback 中只做参数收集、消息发送和轻量同步，不把耗时工作塞进主线程。

## Handle Multi-Window Cases

- 让主窗口成为主题源；子窗口在创建时显式同步 `AppTheme.dark_mode` 和 `Palette.color_scheme`。
- 透明、Overlay、悬浮、聊天窗口也要做主题同步，不要因为背景透明就跳过。
- 如果窗口有独立图标、always-on-top、透明模式或尺寸策略，把这些逻辑留在 Rust 侧窗口创建流程。
- 如果任务涉及多窗口入口判断、不同窗口布局或可见性联动，先画清“窗口类型表”和“共享状态表”再动手。

## Respond In This Style

- 优先给出可落地的目录结构、主题令牌建议、组件切分和 Rust 绑定方式。
- 默认不要展开多套 UI 框架比选；除非用户明确要求，否则沿用本 Skill 的组件策略。
- 如果用户只问“某个控件怎么做”，也要顺手检查它会不会破坏主题统一、模型更新或多窗口同步。
- 输出代码前，先说明准备复用哪些层，准备新增哪些层。

## References

- 需要目录分层、主窗口编排、视图边界时，读取 `references/architecture-and-layout.md`
- 需要决定控件是否自定义、是否混用 `std-widgets`、如何统一主题时，读取 `references/component-and-styling.md`
- 需要做落地前复核、排查更新不生效或多窗口样式不一致时，读取 `references/checklist-and-anti-patterns.md`
