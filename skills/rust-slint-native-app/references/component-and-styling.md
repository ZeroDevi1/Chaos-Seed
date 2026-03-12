# Rust + Slint 组件与样式选型手册

## 默认策略

- 主题源：`AppTheme`
- 外观壳层：自定义组件
- 复杂编辑交互：按需混用 `std-widgets`

## 优先自定义的组件

- `AppButton`
- `Sidebar`
- `Card` / `Panel`
- 页面标题、字段标签、状态条
- 结果行、列表项、带 hover/selected 状态的容器

这些组件的共同点是：视觉一致性比原生编辑能力更重要。

## 优先使用 std-widgets 的场景

- 文本输入框
- 需要稳定光标、选区、复制粘贴、IME 的控件
- 键盘编辑行为复杂的控件

原因：

- 这些能力自己重写成本高，且容易在鼠标、光标、输入法细节上踩坑。
- 使用 `LineEdit` 之类的内建控件，通常能更快得到可用行为。

## 主题令牌建议

至少定义这些令牌：

- `accent`
- `app_bg`
- `panel_bg`
- `sidebar_bg`
- `card_bg`
- `hover_bg`
- `selected_bg`
- `text_primary`
- `text_secondary`
- `text_muted`
- `border_color`
- `input_bg`
- `input_bg_disabled`
- `input_border`
- `button_primary_*`
- `button_secondary_*`

## 暗黑模式同步

- 不要假设 `.slint` 能把局部状态直接绑定回全局主题。
- 让 Rust 显式把 `dark_mode` 推进 `AppTheme`。
- 如果混用了 `std-widgets`，同时同步 `Palette.color_scheme`。

## 多窗口样式规则

- 子窗口继承主窗口主题。
- Overlay 即使背景透明，也要继承文字颜色、强调色和边框策略。
- 聊天窗、浮窗、Dock 窗口可以换布局，但不要自创第二套颜色系统。

## 何时例外

只有在下面场景才偏离默认策略：

- 当前控件高度依赖 Slint 内建可编辑行为
- 自定义实现会明显损害可用性
- 用户明确要求完全跟随 `std-widgets` 原生外观

## 一句话决策法

如果问题是“外观要统一”，优先自定义。
如果问题是“编辑体验要靠谱”，优先 `std-widgets`。
