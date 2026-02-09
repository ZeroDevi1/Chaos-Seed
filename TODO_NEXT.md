# TODO_NEXT（近期 1~5 天交付）

> 说明：这里仅放“最近几天内必须交付”的 bugfix/小功能/风险修复。条目数量保持少（建议 3~6 个），但每条要写清：现象、实现方向（概括）、验收标准、回归清单。

## Done（已完成）

> 日期统一按你确认（目前包含：2026-02-07 / 2026-02-08）。commit hash 为“最贴近该功能完成”的提交（允许多个条目引用同一个提交）。

- P0-1 启动白屏 / 首屏延迟 2~3s：Done @ 2026-02-07（commit: `6e4d711`）
- P0-2 搜索按钮点击后闪退：Done @ 2026-02-07（commit: `6e4d711`）
- P0-3 字幕下载页：搜索有结果但列表不显示 + 输入框鼠标点击不聚焦：Done @ 2026-02-07（commit: `77ac64b`）
- P1-4 白天模式按钮不可见 + 输入框/说明不清晰：Done @ 2026-02-07（commit: `6e4d711`）
- P2-5 Windows 图标（Explorer/任务栏/Alt-Tab）：Done @ 2026-02-07（commit: `6e4d711`）
- P2-6 About 链接 + 侧边栏 Win11 观感增强：Done @ 2026-02-07（commit: `6e4d711`）
- P1-7 弹幕：BiliLive / Douyu / Huya 核心连接与解析移植（先功能后 UI）：Done @ 2026-02-07（commit: `a37fce7`）
- P1-8 新增页面占位（Settings / 直播源 / 弹幕）+ 侧边栏导航调整：Done @ 2026-02-08（commit: `6dea058`）
- P1-9 弹幕 - UI 接入（Chat / Overlay）与交互：Done @ 2026-02-08（commit: `6dea058`）
- P0-10 工程重构：拆分 chaos-core / chaos-slint / chaos-tauri + 新增 chaos-ffi（dll/so 导出层）：Done @ 2026-02-08（commit: `a0b9ff5`）
- P1-11 直播源解析：Huya / Douyu / BiliLive core + chaos-ffi 导出 + header 生成器 + live-check：Done @ 2026-02-09（commit: `68a4017`）

## Next（近期要交付）

### P1：Tauri 前端方案调研（必要时切换/放弃）

**交付目标**
- 梳理并复现目前 tauri UI 的问题来源：FAST design-token recursion、页面布局对不齐（“像 WebView”）。
- 产出结论：继续 tauri（选定前端栈/组件库与落地方案）或切换/放弃（列出替代方案与取舍）。

**验收标准**
- 形成一份可执行的决策记录（写入 `TODO_NEXT.md` 或 `DEVLOG.md`）：包含结论、原因、以及后续实施步骤。

---

### P1：Slint 主题 - 跟随系统 / 浅色 / 深色（下拉框）

**交付目标**
- 将 Slint 中的主题从 `bool dark_mode` 改为三态：`跟随系统 / 浅色 / 深色`。
- 跟随系统时自动同步系统深浅色偏好；手动选择时持久化并覆盖系统。

**验收标准**
- 三态主题可切换且重启后保持；系统主题变化时（跟随系统模式）能同步更新。

---

### P1：直播源 UI 接入（仅展示 + 清晰度/线路切换；播放器后置）

**交付目标**
- UI 侧接入 `chaos-core/chaos-ffi` 的直播解析能力：
  - 输入 URL/房间号 → 解析出 `title / is_living / variants`
  - 列表展示清晰度/线路（variants）
  - 用户点击某个 variant：
    - 若 `url` 已存在：直接使用/复制/交给播放器层
    - 若 `url` 为空：调用 `resolve_variant` 补齐后再使用

**验收标准**
- 能输入 URL → 显示标题与开播状态 → 显示 variants 列表（含 label/quality）
- 切换 variant 时，若需要二段解析能正确补齐 URL（BiliLive/Douyu）
- 能将最终 URL 显示出来（至少支持复制到剪贴板），并保留 referer/user-agent 等 playback hints 供播放层使用
