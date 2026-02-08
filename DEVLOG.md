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
