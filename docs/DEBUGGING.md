# 调试指南

本项目使用 `os.Logger`（统一日志系统）记录网络请求与解析过程，可通过多种方式查看。本文档说明如何 debug 各类问题。

## 1. 日志系统总览

所有日志走 `Log` 门面（`Sources/ChaosSeedApp/App/Log.swift`），subsystem 为 `com.zerodevi1.chaosseed`，按类别分类：

| 类别 | 用途 |
|---|---|
| `network` | HTTP 请求/响应、WBI 签名、buvid 获取 |
| `parsing` | JSON 解析、manifest 组装、InputParser |
| `directory` | 分类/推荐/搜索 |
| `player` | IINA 启动 |
| `app` | 导航、状态、Toast |

**开关**：受「设置 > 调试日志」控制（`UserDefaults.debugLogging`）。
- 关闭（默认）：只输出 `error` 级（错误总会记录）。
- 开启：额外输出 `debug`/`info` 级（每次 HTTP 请求、解析步骤、签名过程）。

## 2. 查看日志的方式

### 方式 A：Console.app（最直观）

1. 打开「控制台」应用（Console.app）
2. 左侧选当前 Mac
3. 搜索框输入 `subsystem:com.zerodevi1.chaosseed`
4. 点「开始」流式查看
5. 在应用里触发操作（如进入直播间），实时看请求链路

> 默认 Console 只显示 `notice`/`error`。要看 `debug` 级：菜单「操作 > 包含信息消息」「包含调试消息」勾上。

### 方式 B：终端 `log stream`（实时）

```bash
# 实时流式输出（包含 debug 级）
log stream --predicate 'subsystem == "com.zerodevi1.chaosseed"' --level debug

# 只看网络类
log stream --predicate 'subsystem == "com.zerodevi1.chaosseed" AND category == "network"' --level debug
```

### 方式 C：终端 `log show`（回看）

```bash
# 回看最近 5 分钟所有日志（含 debug）
log show --predicate 'subsystem == "com.zerodevi1.chaosseed"' --last 5m --info --debug
```

### 方式 D：swift run 控制台

`swift run` 启动时，`notice`/`error` 级会直接打印到终端。`debug` 级需配合上述 Console/log 查看。

## 3. 开启调试日志

应用内：「设置」页 → 打开「调试日志」开关 → 重新进入直播间即可看到完整链路。

**调试构建强制开启**（不影响代码）：smoke 测试或临时调试可用编译标志：
```bash
swift test -Xswiftc -DSMOKE -Xswiftc -DFORCE_LOG --filter SmokeLiveKitTests
```
`FORCE_LOG` 让 `Log.verbose` 恒为 true，便于在测试进程里观察日志。

## 4. 典型 debug 场景

### 场景 1：解析失败（最常见）

进入直播间报错时：
1. 打开调试日志开关
2. 重新进入房间
3. 在 Console / `log stream` 看最后几行：
   - 某个 `GET` 返回非 200？→ 看错误 body 片段判断是风控/参数错
   - `bili get_info` 没有 `canonical_rid`？→ room_id 无效
   - `bili resolve: qn=xxx 未能获取可播放 URL`？→ 该清晰度被限流

**详情页日志框**也会显示解析全过程（房间信息、清晰度列表、Referer），无需查系统日志即可看大概。

### 场景 2：WBI 签名 / buvid 问题

BiliLive 接口返回 `code=-352/-412` 表示被反爬拦截。日志会显示：
```
GET getRecommendRooms → 200  但 code=-352
```
此时 RealLiveKit 内部会自动刷新 WBI key + buvid 重试一次。如仍失败，看 `network` 类日志里 buvid 获取是否成功。

### 场景 3：IINA 不启动

- 查 `player` 类日志：`IINA 未安装` / `IINA 启动失败`
- 确认 `/Applications/IINA.app` 存在
- 需传 Referer 的流走 `iina-cli`（`/Applications/IINA.app/Contents/MacOS/iina-cli`），日志会记录启动命令

## 5. Xcode 断点调试（推荐用于深层问题）

1. 用 Xcode（Xcode-beta）打开 `Package.swift`
2. 顶部 scheme 选 `ChaosSeed`
3. `Cmd+.` 运行
4. 在以下位置设断点：
   - `HTTPClient.perform`（每个请求的入口）
   - `biliDecodeManifest`（解析主流程）
   - `BiliWbi.signQuery`（签名计算）
   - `InputParser.parse`（输入解析）
5. 用 LLDB 查看变量：`po json`、`po url.absoluteString`、`po vars`

## 6. 离线调试（Mock 数据）

设置页打开「使用内置演示数据」→ 用本地样本数据，不访问网络，便于：
- 单独调试 UI（布局、状态切换、分页）
- 在无网络环境验证交互
- 复现纯前端 bug

## 7. 单元测试

```bash
# 全部（62 测试，含 WBI 签名向量、mbga 排序、douyu auth、InputParser 等）
swift test

# 仅跑某类
swift test --filter BiliWbiTests
swift test --filter MbgaTests

# 实时接口 smoke（访问真实 BiliLive/Huya，默认跳过）
swift test -Xswiftc -DSMOKE --filter SmokeLiveKitTests
```

## 8. 常用命令速查

```bash
# 构建
swift build

# 打包 .app
bash scripts/build-app.sh

# 启动 GUI 调试
swift run

# 跑测试
swift test

# 看实时日志
log stream --predicate 'subsystem == "com.zerodevi1.chaosseed"' --level debug
```

> **注意**：本机命令行 `swift` 缺 `BuildServerProtocol.framework`，需用 Xcode-beta 工具链：
> ```bash
> export DEVELOPER_DIR="/Applications/Xcode-beta.app/Contents/Developer"
> export PATH="/Applications/Xcode-beta.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:$PATH"
> ```
