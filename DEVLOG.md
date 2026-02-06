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

