# ChaosSeed.WinUI3 CLI 使用指南

## 概述

ChaosSeed.WinUI3 现在支持命令行界面 (CLI) 模式，可以在不启动 GUI 的情况下使用直播源解析和弹幕获取功能。

## 快速开始

### 显示帮助
```bash
ChaosSeed.WinUI3.exe --help
```

### 解析直播源
```bash
# 解析直播间信息并显示所有可用清晰度
ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123

# 获取指定清晰度的播放地址（JSON 格式）
ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123 --variant "bili_live:10000:原画" --json
```

### 查看清晰度列表
```bash
ChaosSeed.WinUI3.exe stream variants https://live.bilibili.com/123
```

### 连接弹幕
```bash
# 连接弹幕 60 秒
ChaosSeed.WinUI3.exe danmaku connect https://live.bilibili.com/123 --duration 60

# 连接弹幕并过滤关键词（JSON 格式输出）
ChaosSeed.WinUI3.exe danmaku connect https://live.bilibili.com/123 --duration 60 --filter "广告" --filter "推广" --json
```

### TUI 交互模式
```bash
ChaosSeed.WinUI3.exe --tui
```

## 全局选项

| 选项 | 说明 | 示例 |
|------|------|------|
| `-h, --help` | 显示帮助信息 | `--help` |
| `-j, --json` | 输出 JSON 格式 | `--json` |
| `-t, --tui` | 使用 TUI 交互模式 | `--tui` |
| `-i, --interactive` | 交互式模式 | `--interactive` |
| `--backend <模式>` | 后端模式: ffi, daemon, auto | `--backend ffi` |

## 后端模式

### FFI 模式（推荐）
直接调用 `chaos_ffi.dll`，性能更好，启动更快。

```bash
# 显式指定 FFI 模式
ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123 --backend ffi
```

### 环境变量
可以通过环境变量设置默认后端：

```powershell
$env:CHAOS_CLI_BACKEND = "ffi"
ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123
```

## 命令详解

### stream resolve

解析直播源信息。

**参数：**
- `URL` - 直播间 URL（必需）

**选项：**
- `--variant <ID>` - 指定清晰度 ID
- `--json` - 输出 JSON 格式

**示例：**
```bash
# 显示所有清晰度选项
ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123

# 获取指定清晰度的播放地址
ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123 --variant "bili_live:10000:原画"

# JSON 格式输出
ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123 --json
```

### stream variants

仅列出可用清晰度。

**参数：**
- `URL` - 直播间 URL（必需）

**示例：**
```bash
ChaosSeed.WinUI3.exe stream variants https://live.bilibili.com/123
```

### danmaku connect

连接弹幕服务器并实时接收弹幕。

**参数：**
- `URL` - 直播间 URL（必需）

**选项：**
- `--duration <秒>` - 连接持续时间（0 或省略表示无限）
- `--filter <关键词>` - 过滤关键词（可多次使用）
- `--json` - 输出 JSON 格式

**示例：**
```bash
# 连接 60 秒
ChaosSeed.WinUI3.exe danmaku connect https://live.bilibili.com/123 --duration 60

# 无限连接（按 Ctrl+C 停止）
ChaosSeed.WinUI3.exe danmaku connect https://live.bilibili.com/123

# 过滤特定关键词
ChaosSeed.WinUI3.exe danmaku connect https://live.bilibili.com/123 --filter "广告" --filter "推广"
```

## 输出格式

### 人类可读格式（默认）

```
╔════════════════════════════════════════════════════════╗
║                   直播源解析结果                        ║
╠════════════════════════════════════════════════════════╣
║ 平台:    BiliLive                                      ║
║ 房间号:  46936                                         ║
║ 标题:    直播间标题                                     ║
║ 主播:    主播名称                                       ║
║ 状态:    直播中                                         ║
╠════════════════════════════════════════════════════════╣
║ 可用清晰度:                                            ║
║   * [1] 原画         (ID: bili_live:10000:原画       ) ║
╚════════════════════════════════════════════════════════╝
```

### JSON 格式

```json
{
  "site": "BiliLive",
  "room_id": "46936",
  "info": {
    "title": "直播间标题",
    "name": "主播名称",
    "is_living": true
  },
  "variants": [
    {
      "id": "bili_live:10000:原画",
      "label": "原画",
      "quality": 10000,
      "url": "https://..."
    }
  ]
}
```

## 实现架构

```
Program.cs
    ├── CLI Mode
    │     ├── CliParser (命令行解析)
    │     ├── CliRunner (命令执行)
    │     ├── ICliBackend (后端抽象)
    │     │     ├── FfiCliBackend (FFI 调用)
    │     │     └── DaemonCliBackend (Daemon 调用)
    │     ├── Commands
    │     │     ├── StreamCommand (直播源命令)
    │     │     └── DanmakuCommand (弹幕命令)
    │     └── CliTui (TUI 界面)
    └──
        GUI Mode (原有 WinUI3 界面)
```

## 注意事项

1. **依赖文件**: CLI 模式需要 `chaos_ffi.dll` 在同一目录下
2. **编码问题**: 在部分终端中可能遇到中文显示问题，建议使用 UTF-8 编码
3. **Ctrl+C**: 弹幕连接模式下按 Ctrl+C 可以安全退出
4. **错误处理**: 所有错误信息都会输出到 stderr，便于脚本处理

## 常见问题

### Q: CLI 模式无法启动？
A: 确保 `chaos_ffi.dll` 存在于程序目录下。

### Q: JSON 输出格式不正确？
A: 确保使用 `--json` 标志，并且命令语法正确。

### Q: 如何停止弹幕连接？
A: 按 `Ctrl+C` 或等待 `--duration` 指定的时间结束。

## 技术细节

- **入口检测**: 通过 `CliParser.IsCliMode()` 检测是否为 CLI 模式
- **双模式支持**: 同时支持 GUI 和 CLI 模式，根据参数自动切换
- **FFI 调用**: 直接调用 Rust FFI 接口，无需启动 Daemon 进程
- **取消支持**: 所有异步操作都支持 CancellationToken，可通过 Ctrl+C 取消
