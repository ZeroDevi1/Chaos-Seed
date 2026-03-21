# Chaos-CLI 使用文档

Chaos-CLI 是 Chaos-Seed 的命令行工具和 TUI 界面，提供直播源解析、弹幕监控和外部播放器调用功能。

## 功能特性

- **跨平台支持**: Windows、macOS、Linux
- **CLI 模式**: 命令行快速操作
- **TUI 模式**: 交互式终端界面
- **外部播放器**: 支持 IINA+、PotPlayer、VLC
- **实时弹幕**: 支持多平台弹幕监控

## 安装

### 从源码构建

```bash
# 克隆仓库
git clone <repository-url>
cd Chaos-Seed

# 构建 chaos-cli
cargo build -p chaos-cli --release

# 二进制文件位于 target/release/chaos-cli
```

### 添加到 PATH

```bash
# Linux/macOS
sudo cp target/release/chaos-cli /usr/local/bin/

# Windows (PowerShell 管理员)
Copy-Item target\release\chaos-cli.exe C:\Windows\System32\
```

## 使用方法

### CLI 模式

#### 1. 解析直播源

```bash
# 表格形式输出（默认）
chaos-cli resolve "https://live.bilibili.com/12345"

# JSON 格式输出
chaos-cli resolve "https://live.bilibili.com/12345" --format json

# 仅打印播放地址
chaos-cli resolve "https://live.bilibili.com/12345" --format url

# 纯文本格式
chaos-cli resolve "https://live.bilibili.com/12345" --format plain
```

**支持的 URL 格式：**
- Bilibili: `https://live.bilibili.com/xxxx` 或 `bilibili.com/xxxx`
- 斗鱼: `https://www.douyu.com/xxxx` 或 `douyu.com/xxxx`
- 虎牙: `https://www.huya.com/xxxx` 或 `huya.com/xxxx`

**输出示例（表格）：**
```
╔════════════════════════════════════════════════════════════╗
║                    直播间信息                              ║
╚════════════════════════════════════════════════════════════╝
 平台:  bili_live
 房间:  12345
 标题:  直播间标题
 主播:  主播名称
 状态:  🟢 直播中

┌──────┬──────────┬─────────────────────┬──────────┬─────────────────────────────────────────┐
│ 序号 │ 画质     │ ID                  │ 码率     │ 主地址                                  │
├──────┼──────────┼─────────────────────┼──────────┼─────────────────────────────────────────┤
│ 1    │ 1080P    │ 10000               │ 3000kbps │ https://d1--xxxx.mcdn.bilivideo.cn/...│
│ 2    │ 720P     │ 400                 │ 1500kbps │ https://d1--xxxx.mcdn.bilivideo.cn/...│
└──────┴──────────┴─────────────────────┴──────────┴─────────────────────────────────────────┘

💡 提示: 使用 `chaos-cli play "https://live.bilibili.com/12345"` 使用外部播放器播放
```

#### 2. 实时显示弹幕

```bash
# 纯文本格式（默认）
chaos-cli danmaku "https://live.bilibili.com/12345"

# JSON 格式输出
chaos-cli danmaku "https://live.bilibili.com/12345" --format json

# 保存到文件（JSONL 格式）
chaos-cli danmaku "https://live.bilibili.com/12345" -o ./danmaku.jsonl

# 使用正则表达式过滤弹幕
chaos-cli danmaku "https://live.bilibili.com/12345" --filter "^【.*】"
```

**输出示例：**
```
✅ 已连接到 bili_live 直播间: 12345
按 Ctrl+C 退出...

[bili_live] 用户1: 主播好厉害！
[bili_live] 用户2: 66666
[bili_live] 用户3: 这是什么游戏？
```

#### 3. 使用外部播放器播放

```bash
# 自动检测可用播放器（默认最高画质）
chaos-cli play "https://live.bilibili.com/12345"

# 指定播放器
chaos-cli play "https://live.bilibili.com/12345" --player vlc
chaos-cli play "https://live.bilibili.com/12345" --player potplayer
chaos-cli play "https://live.bilibili.com/12345" --player iina
```

**支持的播放器：**

| 平台 | 首选播放器 | 备选播放器 |
|------|-----------|-----------|
| macOS | IINA+ | VLC |
| Windows | PotPlayer | VLC |
| Linux | VLC | - |

### TUI 模式

启动交互式终端界面：

```bash
chaos-cli tui
```

#### 界面截图

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              Chaos-Seed CLI                                  │
│解析直播 | 弹幕监控 | 帮助                                                    │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│ [输入直播间 URL]                                                             │
│ https://live.bilibili.com/12345                                              │
│                                                                              │
│ ╔══════════════════════════════════════════════════════════════════════════╗ │
│ ║                              直播间信息                                  ║ │
│ ╚══════════════════════════════════════════════════════════════════════════╝ │
│  平台:  bili_live                                                            │
│  房间:  12345                                                                │
│  标题:  直播间标题                                                           │
│  主播:  主播名称                                                             │
│  状态:  🟢 直播中                                                            │
│                                                                              │
│ ┌────┬──────────┬─────────────────────┬──────────┬─────────────────────────┐ │
│ │选择│ 画质     │ ID                  │ 码率     │ 主地址                  │ │
│ ├────┼──────────┼─────────────────────┼──────────┼─────────────────────────┤ │
│ │ ▶  │ 1080P    │ 10000               │ 3000kbps │ https://...             │ │
│ │    │ 720P     │ 400                 │ 1500kbps │ https://...             │ │
│ └────┴──────────┴─────────────────────┴──────────┴─────────────────────────┘ │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  按 ? 查看帮助, q 退出                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

#### 快捷键

**全局快捷键：**

| 按键 | 功能 |
|------|------|
| `1` | 切换到"解析直播"标签页 |
| `2` | 切换到"弹幕监控"标签页 |
| `3` 或 `?` | 切换到"帮助"标签页 |
| `Tab` | 下一个标签页 |
| `Shift+Tab` | 上一个标签页 |
| `q` 或 `Esc` | 退出程序 |

**解析直播页面：**

| 按键 | 功能 |
|------|------|
| `Enter` | 开始解析输入的 URL |
| `p` | 使用外部播放器播放 |
| `↑` / `↓` | 选择画质 |
| `c` | 复制选中画质的 URL |

**弹幕监控页面：**

| 按键 | 功能 |
|------|------|
| `Enter` | 连接/断开弹幕 |
| `Space` | 暂停/继续滚动 |
| `f` | 打开过滤对话框 |
| `s` | 保存弹幕到文件 |
| `↑` / `↓` | 滚动弹幕列表 |
| `PageUp` / `PageDown` | 快速滚动 |
| `Home` | 滚动到顶部 |
| `End` | 滚动到底部 |

## 命令行选项

### `resolve` 命令

```
解析直播源地址

Usage: chaos-cli resolve [OPTIONS] <INPUT>

Arguments:
  <INPUT>  直播间 URL 或房间号

Options:
  -f, --format <FORMAT>    输出格式 [default: table] [possible values: table, json, url, plain]
  -q, --quality <QUALITY>  选择画质（默认最高画质）
  -h, --help               Print help
```

### `danmaku` 命令

```
实时显示弹幕

Usage: chaos-cli danmaku [OPTIONS] <INPUT>

Arguments:
  <INPUT>  直播间 URL 或房间号

Options:
  -f, --format <FORMAT>          输出格式 [default: plain] [possible values: table, json, url, plain]
  -o, --output <OUTPUT>          输出到文件（JSONL 格式）
      --filter <FILTER_REGEX>    使用正则表达式过滤弹幕
  -h, --help                     Print help
```

### `play` 命令

```
解析并使用外部播放器播放

Usage: chaos-cli play [OPTIONS] <INPUT>

Arguments:
  <INPUT>  直播间 URL 或房间号

Options:
  -p, --player <PLAYER>    指定播放器（auto/iina/potplayer/vlc） [default: auto]
  -q, --quality <QUALITY>  选择画质（默认最高画质）
  -h, --help               Print help
```

### `tui` 命令

```
启动 TUI 界面

Usage: chaos-cli tui

Options:
  -h, --help  Print help
```

## 环境变量

| 变量名 | 说明 | 示例 |
|--------|------|------|
| `CHAOS_LOG` | 设置日志级别 | `info`, `debug`, `trace` |
| `RUST_LOG` | Rust tracing 日志级别 | `chaos_cli=debug` |

## 配置文件

chaos-cli 目前不需要配置文件，所有选项通过命令行参数传递。

## 故障排除

### 播放器无法启动

**问题：** 提示播放器未找到

**解决方案：**
1. 确认播放器已正确安装
2. 确保播放器在系统 PATH 中
3. Windows 用户：PotPlayer 默认安装路径为 `C:\Program Files\DAUM\PotPlayer\`

### 解析失败

**问题：** 提示 Failed to decode manifest

**解决方案：**
1. 检查网络连接
2. 确认直播间 URL 正确
3. 确认直播间正在直播（未开播的直播间可能无法解析）

### 弹幕连接失败

**问题：** 无法连接到弹幕服务器

**解决方案：**
1. 检查网络连接
2. 某些平台可能需要特定的认证（如登录）
3. 尝试重新连接

## 技术细节

### 架构

chaos-cli 采用分层架构：

```
┌─────────────────────────────────────┐
│           chaos-cli                 │
│  ┌──────────────┬──────────────┐   │
│  │   CLI模式     │   TUI模式     │   │
│  │  (clap)      │  (ratatui)   │   │
│  └──────────────┴──────────────┘   │
│           ↓ 直连 (in-process)        │
│      chaos-app / chaos-core         │
│           (直播源解析/弹幕)           │
└─────────────────────────────────────┘
```

### 依赖

- **CLI 框架**: clap v4.5
- **TUI 框架**: ratatui v0.28 + crossterm v0.27
- **异步运行时**: tokio v1.x
- **序列化**: serde + serde_json
- **核心库**: chaos-core + chaos-app

### 跨平台实现

- **播放器检测**: 根据操作系统自动选择最佳播放器
- **路径处理**: 使用标准库处理跨平台路径
- **终端控制**: 通过 crossterm 实现跨平台终端操作

## 更新日志

### v0.9.0 (2026-03-21)

- ✨ 初始版本发布
- ✨ 支持直播源解析（Bilibili、斗鱼、虎牙）
- ✨ 支持实时弹幕监控
- ✨ 支持外部播放器调用（IINA+、PotPlayer、VLC）
- ✨ 提供 CLI 和 TUI 两种模式
- ✨ 跨平台支持（Windows、macOS、Linux）

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

MIT License
