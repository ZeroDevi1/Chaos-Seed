# chaos-cli

Chaos-Seed 的命令行工具和 TUI 界面，支持直播源解析和弹幕监控。

## 功能特性

- **跨平台**：支持 Windows、macOS 和 Linux
- **CLI 模式**：命令行快速操作
- **TUI 模式**：交互式终端界面
- **外部播放器**：支持调用 IINA+、PotPlayer、VLC

## 安装

```bash
cargo build -p chaos-cli --release
```

编译完成后，二进制文件位于 `target/release/chaos-cli`。

## 使用方法

### CLI 模式

#### 解析直播源

```bash
# 表格形式输出（默认）
chaos-cli resolve "https://live.bilibili.com/12345"

# JSON 格式输出
chaos-cli resolve "https://live.bilibili.com/12345" --format json

# 仅打印播放地址
chaos-cli resolve "https://live.bilibili.com/12345" --format url
```

#### 实时显示弹幕

```bash
# 纯文本格式
chaos-cli danmaku "https://live.bilibili.com/12345"

# JSON 格式输出
chaos-cli danmaku "https://live.bilibili.com/12345" --format json

# 保存到文件
chaos-cli danmaku "https://live.bilibili.com/12345" -o ./danmaku.log

# 使用正则过滤弹幕
chaos-cli danmaku "https://live.bilibili.com/12345" --filter "^【"
```

#### 使用外部播放器播放

```bash
# 自动检测可用播放器
chaos-cli play "https://live.bilibili.com/12345"

# 指定播放器
chaos-cli play "https://live.bilibili.com/12345" --player vlc
chaos-cli play "https://live.bilibili.com/12345" --player potplayer
chaos-cli play "https://live.bilibili.com/12345" --player iina
```

### TUI 模式

```bash
chaos-cli tui
```

#### 快捷键

**全局快捷键：**
- `1` - 切换到"解析直播"标签页
- `2` - 切换到"弹幕监控"标签页
- `3` 或 `?` - 切换到"帮助"标签页
- `Tab` - 下一个标签页
- `Shift+Tab` - 上一个标签页
- `q` 或 `Esc` - 退出程序

**解析直播页面：**
- `Enter` - 开始解析输入的 URL
- `p` - 使用外部播放器播放
- `↑/↓` - 选择画质
- `c` - 复制选中画质的 URL

**弹幕监控页面：**
- `Enter` - 连接/断开弹幕
- `Space` - 暂停/继续滚动
- `f` - 打开过滤对话框
- `s` - 保存弹幕到文件
- `↑/↓` - 滚动弹幕列表
- `PageUp/PageDown` - 快速滚动

## 支持的平台

### 直播源解析
- Bilibili 直播
- 斗鱼直播
- 虎牙直播

### 弹幕监控
- Bilibili 直播弹幕
- 斗鱼弹幕
- 虎牙弹幕

### 外部播放器
- **macOS**: IINA+（首选）、VLC
- **Windows**: PotPlayer（首选）、VLC
- **Linux**: VLC

## 环境变量

- `CHAOS_LOG` - 设置日志级别（如 `info`、`debug`）

## 技术栈

- **CLI 框架**: clap
- **TUI 框架**: ratatui + crossterm
- **异步运行时**: tokio
- **核心库**: chaos-core

## 许可证

MIT
