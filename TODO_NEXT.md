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
- P1-17 直播源：解析 UI + 新窗口播放器（线路/清晰度切换 + 关闭即停止）：Done @ 2026-02-09（commit: `ee0ef1d`）
- P0-18 Now Playing：勾选“包含封面(base64)”后获取正在播放信息卡死：Done @ 2026-02-10（commit: `78d35c1`）
- P1-19 歌词（core/ffi）：移植 LyricsX/LyricsKit 行为到 chaos-core（多源并发/strict/quality/超时）+ chaos-ffi 导出 + 文档：Done @ 2026-02-10（commit: `78d35c1`）
- P1-20 歌词（Tauri UI）：三段布局（操作区/来源列表/正文区）+ 单选切换 + Chat/Overlay 新窗口显示 + 原文-译文对应：Done @ 2026-02-10（commit: `78d35c1`）
- P0-21 歌词系统对齐 BetterLyrics：QQ/网易云/LRCLIB + match_percentage + 顺序阈值自动搜索 + Dock/Float + 暂停自动隐藏 + 托盘开关 + Windows tag release CI：Done @ 2026-02-10（commit: `e72ab32`）

## Next（近期要交付）

### P1：SMTC 真事件订阅（降低空闲功耗）

**交付目标**
- 将 Now Playing 的“自适应轮询”升级为 WinRT 事件订阅（SMTC/GSMTC）：
  - `SessionsChanged` / `CurrentSessionChanged`
  - `MediaPropertiesChanged` / `PlaybackInfoChanged` / `TimelinePropertiesChanged`
- 以事件为主驱动 UI 刷新，轮询仅作为兜底（resync 低频）

**验收标准**
- 非 playing 或无 Dock/Float 窗口打开时：后台 CPU 占用明显下降（接近空闲）
- 切歌/暂停/恢复播放：歌词与高亮刷新及时且不丢事件

---

### P0：播放器诊断与兼容模式开关

**交付目标**
- 播放器窗口提供“诊断信息”与“兼容模式”：
  - 显示当前引擎（HLS/AvPlayer/Native）、当前实际在播 URL（含候选序号）、最近一次错误原因
  - 提供开关：启用/禁用 AvPlayer 的 Hardware/WebCodecs（遇到黑屏/有声无画时快速切换）

**验收标准**
- 能复现“黑屏/有声无画”时，用户无需打开 DevTools 也能看到关键诊断信息
- 切换兼容模式后无需重启应用即可重建播放器并重新播放

---

### P1：反盗链与请求头注入（按需）

**交付目标**
- 对需要 Referer/UA 的直播源提供注入能力（优先最小实现）：
  - HLS：通过 `hls.js` 的 xhrSetup/headers 注入
  - AvPlayer：通过 `load(..., { http: { headers } })` 注入
- 若仍不够（部分平台强依赖 cookie 或更复杂校验），再引入本地代理方案

**验收标准**
- 对至少一个平台（优先 Huya 或 BiliLive）验证注入生效并稳定播放

---

### P2：播放体验优化

**交付目标**
- 自动重试策略完善（例如优先 backup_urls、失败回退、超时提示）
- 播放控制栏交互优化（快捷键/全屏/音量等）

**验收标准**
- URL 失效/节点不可用时能自动切换到可用节点并给出提示
