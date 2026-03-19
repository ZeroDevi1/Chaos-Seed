# Rust + Tauri + Svelte 组件与样式选型手册

## 默认策略

- 控件层：Fluent Web Components
- 壳层层：CSS token + 自定义 Svelte 组件
- 主题入口：`html[data-theme=...]`
- 控件 token：Fluent design tokens

## 优先使用 Fluent 的场景

- 按钮
- 文本输入
- 数字输入
- 下拉选择
- 工具栏
- 树形导航
- 提示层
- 菜单
- 骨架屏

这些控件的共同点是：系统感和成熟交互比完全定制外观更重要。

## 优先自定义的场景

- 应用整体布局
- 侧栏折叠与宽度动画
- 页面容器
- 卡片与结果列表
- 面板分栏
- Overlay / Canvas / Player 壳层
- 歌词或弹幕等特殊视觉效果

这些区域的共同点是：应用识别度和窗口差异化比标准控件复用更重要。

## Fluent 使用规则

- 只注册实际使用到的组件。
- 把注册放到独立模块，避免每个页面重复 import 和注册。
- 把 design token 写入稳定 host，减少 DOM 波动带来的重复写入。
- 如果 token 写入可能引发性能或兼容问题，保留显式降级或 kill switch。

## 主题与 token 规则

- 用 CSS 变量维护 `app_bg`、`panel_bg`、`sidebar_bg`、`card_bg`、`hover_bg`、`selected_bg`、`text_*`、`border_*`、`button_*`。
- Fluent token 负责控件层，CSS token 负责应用壳层；两者不要互相替代。
- 支持系统 accent 时提供回退色，保证非 Windows 或读取失败时外观稳定。
- 多窗口应用必须在挂载前就有正确主题，避免二级窗口闪白。

## 特殊窗口规则

- `overlay`、`player`、`lyrics_overlay`、`dock`、`float` 这类窗口通常不应直接复用主窗口 backdrop。
- 透明窗口优先保持背景策略独立，但文本色、强调色和交互令牌仍应统一。
- 对真正的系统特效，例如 Mica，Rust/Tauri 负责 OS 侧效果，CSS 只负责 webview 侧透明度和表面色。

## 一句话决策法

如果用户要的是“像原生控件”，先选 Fluent。
如果用户要的是“像完整桌面应用壳层”，先选自定义 Svelte + CSS。
