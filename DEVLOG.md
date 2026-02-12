# 开发日志（DEVLOG）

## 2026-02-06

### 已完成（当前版本达成度）
- 初始化 Rust + Slint 项目骨架（Home / 字幕下载 / About 三页 + 侧边栏导航）。
- 实现主题切换（黑夜/白天）基础结构（由 Rust 驱动 Slint global 的 `dark_mode`）。
- 将 `thunder-subtitle` 的核心逻辑以 Rust 方式移植为业务层模块：
  - 搜索接口：`https://api-shoulei-ssl.xunlei.com/oracle/subtitle?name=...`
  - 结果解析 gate：`code == 0 && result == "ok"`
  - 排序/过滤/limit + 下载（含重试、超时、文件名 sanitize、同名不覆盖的 unique path）
- 构建矩阵准备：
  - Windows 原生构建（MSVC）
  - WSL 交叉编译到 Windows：`x86_64-pc-windows-gnu`（mingw-w64）与 `x86_64-pc-windows-msvc`（cargo-xwin）
  - 默认使用 software renderer 作为稳定兜底（Skia 在部分 MSVC 环境存在链接问题）
- 添加构建脚本与基础说明（README 已中文化）。

### 已知问题（待修复/待完善）
- 搜索后可能闪退（需要在 Windows Debug 下复现并拿到 backtrace，做无 panic 的错误呈现）。
- 字幕下载页交互需要改为：回车/按钮搜索 -> 列表展示 -> 每条“下载”按钮 -> 每条下载都弹出选择目录（不复用）。
- 白天模式下部分按钮对比度不足，存在“不可见”情况。
- Windows `.exe` 目前未嵌入应用图标（Explorer/任务栏/Alt-Tab）。
- 侧边栏希望进一步贴近 Win11 风格（折叠/展开动画、icon 占位、交互动效）。

## 2026-02-07

### 变更（当前版本迭代）
- 文档分层：
  - `TODO.md` 调整为长期路线图（大方向/大功能）
  - `TODO_NEXT.md` 调整为近期 1~5 天交付的修复清单（少而详尽，带验收与回归）
- 弹幕（先功能后 UI）：
  - 清空并重构 `src/danmaku`，对齐 IINA+ 的事件模型（`SendDM` / `LiveDMServer`）与平台抽象。
  - 平台支持：BiliLive（WBI 签名 + token + ws + dm_v2(pb) + emoticon 元数据）/ Douyu（HTML 解析 room_id + ws + blocklist）/ Huya（HNF_GLOBAL_INIT 解析 + ws + blocklist）。
  - 提供调试用示例：`examples/danmaku_dump.rs`。
- 字幕下载页交互调整为：回车/按钮搜索 → 列表展示 → 每条“下载”按钮 → 每次下载弹出目录选择（不复用上次目录）。
- UI 统一控件：
  - 自定义按钮/输入框组件接入主题（改善白天模式对比度与可见性）
  - 输入框支持 `Enter`/accepted 回调
- 稳定性与体验：
  - 后台异步改为独立 tokio runtime 线程 + UI 消息泵（减少 UI 线程阻塞与崩溃风险）
  - 为 release 增加 panic 日志落地（`logs/panic_*.log`）
- Windows 图标：
  - 生成并嵌入 Windows 资源图标（`.rc + .ico`，通过 build.rs 处理）

## 2026-02-08

### 变更（当前版本迭代）
- 弹幕 UI 接入：
  - 新增弹幕相关页面与交互：支持从输入 URL/房间号发起连接，并在 UI 中展示弹幕（Chat / Overlay）。
  - Overlay 弹幕文字调整为白色（更适合覆盖在视频上层）。
- 工程结构升级（为多 UI 与跨语言调用铺路）：
  - 拆分 `chaos-core`（纯业务）与 UI 层（`chaos-slint` / `chaos-tauri`）。
  - 新增 `chaos-ffi`：以 C ABI + JSON 形式导出 `chaos-core` 能力，便于 WinUI3/Qt 调用。
- 关闭文件日志落地：不再写入 `logs/app.log` 与 `logs/panic_*.log`。

## 2026-02-09

### 变更（当前版本迭代）
- 直播源解析（core）：
  - `chaos-core` 新增 `livestream` 模块（`LiveManifest/StreamVariant` + `resolve_variant`），支持 Huya / Douyu / BiliLive。
  - 全离线 `httpmock` 测试覆盖关键解析链路与签名/排序逻辑。
- 直播源解析（FFI）：
  - `chaos-ffi` 导出 livestream JSON API：`decode_manifest` + `resolve_variant`。
  - `chaos-ffi` 内置 cbindgen 头文件生成器（`gen_header`），可随时生成/更新 `include/chaos_ffi_bindings.h`。
  - feature gated 的 live-check（真实 URL 运行时参数传入，可 `--dump-json` 输出解析结果）。
- Tauri UI（弹幕 / 多窗口 / 主题）：
  - Chat/Overlay 多窗口彻底可用（Windows/WebView2）：修复子窗口白屏/卡死/无法关闭、同源 URL 加载与启动参数注入；并将 child window 的调试输出改为可控开关。
  - 弹幕按窗口订阅推送：高频 `danmaku_msg` 只发给订阅窗口；打开 Chat/Overlay 后主窗停止刷新，降低压力。
  - 弹幕渲染低延迟：从“定时 flush”改为尽快渲染（并在高压时分帧排空），减少“延时感”。
  - Settings 页布局更原生、弹幕滚动行为修正、浅/深色主题 + Mica 下减少割裂。
- Tauri UI（直播源 / 播放器）：
  - 直播源页接入解析能力：输入 URL → 解析 manifest/variants → 下拉选择线路/清晰度；文本区实时显示直连 URL（含 backup_urls）。
  - 新窗口播放器（Master 风格）：Hls.js + Libmedia AvPlayer 多内核；支持 failover（候选 URL 顺序与重试）、播放器窗口关闭即销毁并终止播放。
  - Windows dev 兼容性：libmedia 相关依赖从 Vite optimizeDeps 排除；建议遇到异常时删除 `chaos-tauri/node_modules/.vite` 后重启。
- commits（关键节点）：
  - livestream core/ffi：`68a4017`
  - windows 多窗口稳定：`1be9028`
  - 弹幕按窗口订阅/状态保持：`5f4434b`
  - UI 精修（Chat/Overlay/Settings/滚动/主题基线）：`aa22f62`
  - 弹幕低延迟 + 主窗停推兜底：`bfadcd4`
  - 主题：Mica 下浅色不透底 + 深色融合：`4d05c31`
  - livestream UI + player：`ee0ef1d`

## 2026-02-10

### 变更（当前版本迭代）
- 歌词（core，clean-room 重写）：
  - 新增 `chaos-core::lyrics`：对齐 LyricsX/LyricsKit 的“多源搜索 → 拉取 → strict match → quality 排序 → 超时降级”行为。
  - Provider 支持：NetEase / QQMusic / Kugou（并提供离线 httpmock 测试覆盖 JSONP/base64/KRC 解密等关键链路）。
  - 新增 CLI 测试入口：`cd chaos-core && cargo run -- test ...`（支持位置参数与 `--title/--artist/...` 模式）。
- 歌词（FFI）：
  - `chaos-ffi` 新增导出：`chaos_lyrics_search_json`（JSON in/out 风格），并更新 `API.md` / `CSharp.md`；API_VERSION bump 到 4。
- Tauri UI（歌词页/多窗口）：
  - 修复：勾选“包含封面(base64)”后点击获取 Now Playing 卡死（不再渲染超大 JSON；限制 maxSessions=1，仅预览封面）。
  - 新增 `lyrics_search` command（默认仅使用 netease/qq/kugou 以保证响应速度），并改造 LyricsPage 为三段布局：操作区 / 来源列表（无正文）/ 正文区（原文-译文对应）。
  - 新增两个歌词显示窗口：Chat（不透明）/ Overlay（透明置顶），通过事件 `lyrics_current_changed` 实时刷新内容。
- Now Playing（Tauri backend 加固）：
  - `now_playing_snapshot` 改为 async 返回结构体（spawn_blocking），避免主线程阻塞并减少传输/渲染风险。

### 变更（歌词系统对齐 BetterLyrics）
- 在线歌词源策略对齐 BetterLyrics：
  - 默认仅启用并优先使用 QQ 音乐 / 网易云 / LRCLIB（旧源保留但默认不参与自动流程）
  - 新增 BetterLyrics 风格 match score（Jaro-Winkler + duration curve + 权重），在结果中暴露 `match_percentage (0~100)`
  - 新增 LRCLIB provider（直接消费 `syncedLyrics`）
- Now Playing 元数据补强：
  - session 增加 `genres` 与 `song_id`（从 `NCM-xxxx` / `QQ-xxxx` 解析，便于后续更精确匹配）
- 播放监控与自动搜词（Tauri backend）：
  - 增加“歌词检测”开关与持久化设置；开启后收到歌曲变更即按 providers 顺序逐个搜索，命中阈值直接停止
  - 广播事件：`now_playing_state_changed` / `lyrics_detection_state_changed`（前端用 `retrieved_at` 插值推进时间轴）
  - 低功耗策略：当前以自适应轮询为主（Dock/Float 打开且 playing 时才会更积极 resync）；后续可升级为 WinRT 事件订阅
- 显示模式重做（Tauri frontend）：
  - 新增 Dock（贴边侧边栏）与 Float（桌面悬浮挂件）两种歌词窗口
  - 暂停自动隐藏、恢复播放自动恢复显示（仅恢复因自动隐藏导致的隐藏）
  - 轻量特效系统：fluid 背景 / fan3d 布局 / snow 粒子（可在设置切换）
- 工程化：
  - 新增 tag 触发 Windows Release 工作流：构建 `chaos-ffi` + `chaos-tauri` 并上传到对应 GitHub Release

## 2026-02-12

### 变更（WinUI 3：直播播放体验）
- 全屏行为：
  - 修复播放开始后画面“先铺满→瞬间缩到左上角小图”的布局问题（播放器宿主强制拉伸）。
  - 默认全屏播放时，应用内全屏覆盖层提升到 `MainWindow` 根，视觉上隐藏导航/标题栏；并在可配置延迟后进入系统全屏。
  - 新增设置项：“直播：全屏切换延迟(ms)（调试）”（默认 `350`，范围 `0..2000`），用于观察/缓解切换卡顿。
- 弹幕表情（图片弹幕）：
  - FFI 后端的图片拉取补齐 URL 规范化、SSRF 阻断、B 站/hdslb Referer/UA 注入，并将上限对齐到 `2.5MB`。
  - Debug 播放提示增加表情加载统计（req/ok/fail + lastErr，2s 节流），便于判断是接收还是渲染问题。
