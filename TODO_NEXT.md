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
- P0-12 Tauri：Chat/Overlay 子窗口白屏/无法关闭/遮挡主窗点击（多窗口 URL/权限/创建死锁等）：Done @ 2026-02-09（commit: `1be9028`）
- P1-13 弹幕：按窗口订阅推送（高频 msg 不再广播）+ 弹幕页 keepAlive（跨页面保持状态）：Done @ 2026-02-09（commit: `5f4434b`）
- P1-14 UI：Chat/Overlay 精简 + 弹幕外层滚动条修复 + Settings 原生化布局 + 主题基线：Done @ 2026-02-09（commit: `aa22f62`）
- P1-15 弹幕渲染：低延迟（Rust 推送后尽快渲染）+ 打开 Chat/Overlay 后主窗停止刷新：Done @ 2026-02-09（commit: `bfadcd4`）
- P1-16 主题：Mica 下浅色不透底 + 深色表面融合（减少割裂）：Done @ 2026-02-09（commit: `4d05c31`）

## Next（近期要交付）

### P1：直播源解析 UI 设计与接入（manifest/variants）

**交付目标**
- UI 侧接入 `chaos-core/chaos-ffi` 的直播解析能力：
  - 输入 URL/房间号 → 解析出 `title / is_living / variants`
  - 列表展示清晰度/线路（variants），并能点击切换
  - 对需要二段解析的平台：点击 variant 时调用 `resolve_variant` 补齐最终 URL
- “先可用、后播放器”：本阶段只需要把最终 URL + playback hints（UA/Referer 等）展示出来并支持复制到剪贴板。

**验收标准**
- 能输入 URL → 显示标题与开播状态 → 显示 variants 列表（含 label/quality）
- 切换 variant 时，若需要二段解析能正确补齐 URL（BiliLive/Douyu）
- 能将最终 URL 显示出来（至少支持复制到剪贴板），并一并显示/复制 referer/user-agent 等 playback hints 供播放层使用

---

### P2：直播播放层（后置）

**交付目标**
- 选定可分发的播放方案（内置播放器 / WebView / 外部播放器），并打通“选 variant → 播放”的最短路径。

**验收标准**
- 至少能用选定方案播放一个平台的直播流（先 POC，再工程化）。
