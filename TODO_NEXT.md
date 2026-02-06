# TODO（下一阶段）

> 说明：本文仅作为后续开发计划（暂不改代码）。重点覆盖：字幕下载页交互重做、白天模式可见性、搜索闪退定位、Windows 图标、侧边栏 Win11 风格增强，以及三种构建方式的持续可用性回归。

## 0. 当前已知问题清单（按优先级）

### P0（必须优先）
- **搜索后闪退**：点击搜索按钮后可能直接崩溃退出（Windows release 下更难看见报错）。
- **启动后主界面延迟加载/疑似白屏**：打开 `.exe` 后先出现窗口与标题栏，但主页面内容似乎未立即渲染（能“看到后面的窗口”），约 2~3 秒后首页才加载出来。
- **字幕下载页交互不符合预期**：
  - 输入关键词后，应支持 **回车触发搜索**（旁边保留“搜索按钮”）。
  - 搜索结果应以列表展示在当前页面。
  - **每条结果**右侧一个“下载”按钮（单条下载，不做多选/批量）。
  - 点击“下载”后 **再弹出选择目录**；并且 **每条下载都必须重新选择目录**（不复用上一次目录）。

### P1（尽快）
- **白色模式按钮不可见/对比度不足**（hover/pressed/disabled 也需要可见）。
- **输入框只有点击才显色**：建议加 label/说明文案，并确保未聚焦态仍清晰可辨。

### P2（增强/收尾）
- **应用无 icon**：生成的 `.exe` 在 Explorer/任务栏/Alt-Tab 缺少自定义图标。
- **About 链接**：固定为 `https://github.com/ZeroDevi1/Chaos-Seed`。
- **侧边栏 Win11 风格增强**：折叠/展开更像 `wsl-dashboard`（icon 占位 + 动画更灵动）。

---

## 1. Milestone A：定位并修复“搜索闪退”（P0）

### 目标
- 任意关键词搜索：应用不崩溃。
- 网络/解析错误：以 UI 文本提示（status）呈现，busy 状态可复位。

### 定位步骤（建议按顺序）
1. **Windows Debug 复现 + 拿到 panic 栈**
   - 以 Debug 启动（可用 `cargo run`）确保能看到控制台输出。
   - 设置：`RUST_BACKTRACE=1`（必要时 `RUST_BACKTRACE=full`）。
2. **检查常见崩溃点**
   - 搜索 `unwrap()` / `expect()` / 越界索引。
   - 检查 `slint::invoke_from_event_loop(...).unwrap()`。
   - 检查 `app_weak.upgrade()` 失败时是否被 unwrap。
3. **后台任务到 UI 的更新路径统一“无 panic + 可见错误”**
   - 后台任务返回 `Result`。
   - UI 更新仅通过 `invoke_from_event_loop`，并把错误写到 status 文本。
4. **Release（`windows_subsystem`）下也能看到错误**
   - 规划：写入日志文件（如 `logs/app.log`）或提供“错误详情”弹窗（MVP 先用文本即可）。

### 验收
- 连续搜索 10 次、空关键词、快速切页：不崩溃；提示正确；busy 不会卡死。

---

## 1.5 Milestone A2：排查启动白屏/延迟渲染（P0）

### 目标
- 启动后 0~300ms 内至少渲染出可见的“骨架/占位”界面（比如标题、侧边栏框架或 Loading 文案）。
- 将首屏可交互时间（TTI）从 2~3s 降到可接受范围（先以 < 1s 为目标）。

### 排查方向（建议按顺序）
1. **确认是否被同步初始化阻塞 UI 线程**
   - tokio runtime 创建、模型初始化、字体/资源加载、读取环境信息等是否在启动时同步执行。
2. **确认渲染后端/驱动导致的首帧延迟**
   - software/skia renderer 对比；winit 事件循环启动前后是否有阻塞段。
3. **加观测点**
   - 在关键初始化点打时间戳日志（启动 -> 创建 window -> set model -> show -> 首帧回调/事件）。
4. **兜底体验**
   - 首屏先显示 Loading（或 skeleton），把耗时初始化移动到后台任务，完成后再切换到完整 Home。

### 验收
- 打开 `.exe` 不出现“透明/未渲染导致能看到后面窗口”的体验；首屏很快可见且稳定。

---

## 2. Milestone B：字幕下载页交互重做（P0）

### 目标交互（锁定）
- 输入关键词后：
  - **回车**触发搜索；
  - 旁边保留 **搜索按钮**。
- 搜索结果显示为列表：
  - 每条有 **下载**按钮（单条下载）。
- 点击某条下载：
  - **先弹出目录选择**（每条下载都要选目录，不复用）。
  - 选择后开始下载，状态显示“Downloading i/N ... / 完成 / 失败原因”。

### UI 结构建议
- 顶部：关键词输入（带 label）+ 搜索按钮
- 中部：结果列表（行内展示 score/name/ext/languages/extra_name + 下载按钮）
- 底部：busy + status 文本

### 实现注意点（后续编码时）
- 回车触发优先用 TextInput 的 accepted/enter 回调；若不支持则捕获 Enter key。
- 目录选择对话框尽量在 UI 线程触发（避免死锁/崩溃）。
- “取消选择目录”应当给出提示且不开始下载。

### 验收
- 回车/点击搜索都能出列表；点击任意条下载必弹目录选择；下载成功落地；失败可见。

---

## 3. Milestone C：主题与白天模式可见性修复（P1）

### 目标
- 白天模式下：按钮/输入框/列表行/hover/pressed/disabled 全部清晰可见，不“隐形”。

### 工作项
- 梳理并补齐主题 token（建议至少）：
  - `bg / fg / panel_bg / border / accent`
  - `button_bg / button_fg / button_bg_hover / button_bg_pressed`
  - `input_bg / input_fg / placeholder_fg / focus_border`
- 所有按钮显式使用主题色，不依赖控件默认配色。

### 验收
- 黑/白主题切换后，按钮始终可见；hover/pressed 有反馈；禁用态仍可区分。

---

## 4. Milestone D：Windows 应用图标（P2）

### 目标
- `.exe` 在 Explorer/任务栏/Alt-Tab 显示自定义 icon。

### 实施路线（需兼顾三种构建）
- Windows 原生 MSVC：`.rc` + `embed-resource`/`winres`（二选一）
- WSL 交叉 GNU（mingw）：`windres` 资源编译（确保工具链可用）
- WSL 交叉 MSVC（cargo-xwin）：确认资源编译链路与输出路径

### 验收
- 三种产物（win-msvc / wsl-gnu / wsl-xwin-msvc）图标一致可见。

---

## 5. Milestone E：About / 侧边栏细节（P2）

### About
- 链接固定为：`https://github.com/ZeroDevi1/Chaos-Seed`
- （可选）显示版本号 `CARGO_PKG_VERSION`

### 侧边栏 Win11 风格增强
- 折叠/展开动画更自然：宽度过渡、选中态更明显、icon 占位
- hover 高亮、切页过渡（轻量即可）

---

## 6. 构建与回归（每个里程碑都要跑）

> 目标：Win 原生 + WSL 交叉构建持续可用；renderer 可切换兜底。

- Windows 原生（MSVC）：`cargo build --release`
- WSL -> windows-gnu：`cargo build --release --target x86_64-pc-windows-gnu`
- WSL -> windows-msvc（xwin）：`cargo xwin build --release --target x86_64-pc-windows-msvc`
- Renderer fallback：
  - 默认 software（稳定优先）
  - 可选 skia（单独脚本/feature），失败不阻塞主线

---

## 7. 建议交付顺序（最短路径）
1. Milestone A：先修复“搜索闪退”
2. Milestone B：字幕下载页交互重做（回车搜索 + 单条下载 + 每次选目录）
3. Milestone C：白天模式对比度/可见性
4. Milestone D：应用 icon
5. Milestone E：侧边栏 Win11 动画增强 + About 细节
