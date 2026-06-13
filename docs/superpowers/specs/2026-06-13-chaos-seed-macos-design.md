# Chaos Seed macOS 原生直播源解析应用设计文档

## 1. 目标

为 macOS 构建一款原生 Swift / AppKit 桌面应用，复用 Chaos-Seed 的 Rust 核心能力，提供以下功能：

- 首页浏览 BiliLive / Douyu / Huya 三大平台的直播间目录（分类 + 搜索 + 分页）。
- 点击直播间卡片进入详情页，解析并展示清晰度 / 线路列表。
- 选择清晰度后，调用本地安装的 IINA 播放器进行播放。
- 支持直接粘贴直播间 URL 进行解析（兜底入口）。

## 2. 非目标

- 不内置视频渲染（复用 IINA）。
- 第一轮不接入弹幕、歌词、歌曲下载等 Chaos-Seed 其他能力。
- 不重新实现直播源解析协议；所有平台协议逻辑由 Rust 核心负责。

## 3. 架构

```text
┌──────────────────────────────────────────────────────┐
│            Swift / AppKit macOS 应用                  │
│  HomeViewController → RoomGrid → RoomDetail → IINA   │
└──────────────┬───────────────────────────────────────┘
               │ JSON-RPC 2.0 over Unix Domain Socket
               ▼
┌──────────────────────────────────────────────────────┐
│              chaos-daemon (macOS 本地进程)             │
│  live_directory.list_rooms / search                   │
│  livestream.resolve / resolve_variant                 │
└──────────────────────────────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────────────┐
│                   chaos-core (Rust)                   │
│        BiliLive / Douyu / Huya 协议实现               │
└──────────────────────────────────────────────────────┘
```

### 3.1 为什么选择 chaos-daemon + Unix Domain Socket

- 与 WinUI3 分支的 IPC 方案一致，降低跨前端维护成本。
- Swift 侧无需维护 C header，只需按 `chaos-proto` 的 JSON 形状序列化/反序列化。
- daemon 崩溃不影响 UI 进程，可自动重启。
- 未来升级核心时，只需替换 daemon 二进制。

### 3.2 备选方案

- **in-process FFI**：启动更快，但需维护 `chaos_ffi` 的 C header 与 Swift 桥接头；后续升级容易因 ABI 变化而破坏兼容性。保留为第二阶段优化项。

## 4. 数据流

### 4.1 首页加载直播间列表

1. UI 发送 `live_directory.list_rooms`：
   - 参数：`{ platform: "bili_live" | "douyu" | "huya", category?: string, page: number, page_size: number }`
   - 返回：`{ rooms: [{ id, title, cover_url, streamer_name, viewer_count, platform, room_url }], total, has_more }`
2. UI 以瀑布流/网格展示卡片。

### 4.2 搜索

1. UI 发送 `live_directory.search`：
   - 参数：`{ platform, keyword: string, page, page_size }`
   - 返回与 list_rooms 相同。

### 4.3 进入直播间详情

1. UI 发送 `livestream.resolve`：
   - 参数：`{ input: room_url }`
   - 返回：`{ room: { id, title, streamer }, manifests: [{ name, variants: [{ variant_id, name, url?: string }] }] }`
2. UI 展示清晰度列表；若 variant 已带 `url`，可直接播放；否则需要二段解析。

### 4.4 二段解析与播放

1. 用户选择某个 variant，UI 发送 `livestream.resolve_variant`：
   - 参数：`{ input: room_url, manifest_name, variant_id }`
   - 返回：`{ url: string, headers?: { Referer?, User-Agent?, Cookie? } }`
2. UI 启动 IINA：
   - 优先使用 `NSWorkspace.shared.open(_:)` 打开 URL。
   - 若需要注入 Referer/UA/Cookie，使用 `Process` 执行 `/Applications/IINA.app/Contents/MacOS/iina-cli` 并传入 `--mpv-http-header-fields` 或等效参数。

## 5. 页面结构

### 5.1 主窗口

- 最小尺寸：900 × 600。
- 左侧导航栏（可收起）：首页、历史、设置。
- 右侧内容区根据导航切换。

### 5.2 首页

- 顶部工具栏：
  - 平台 Segmented Control：BiliLive / Douyu / Huya。
  - 搜索框 + 搜索按钮。
  - 刷新按钮。
- 主体：
  - 直播间卡片网格（2–4 列自适应）。
  - 每个卡片显示封面、标题、主播名、在线人数。
  - 底部翻页：上一页 / 下一页 / 页码。
- 空状态：首次加载提示、搜索无结果提示、网络错误重试。

### 5.3 直播间详情页

- 左侧/顶部：封面、标题、主播、平台标识、在线人数。
- 右侧/主体：
  - 清晰度/线路列表（TableView）。
  - 每个条目显示：清晰度名称、线路名称、是否已解析出直连 URL。
  - 选中后底部“在 IINA 中播放”按钮启用。
- 额外操作：
  - 复制直连 URL。
  - 显示解析日志 / 错误信息。

### 5.4 设置页

- daemon 可执行文件路径（默认 `Contents/Resources/chaos-daemon`）。
- IINA 应用路径（默认 `/Applications/IINA.app`）。
- 默认清晰度偏好（例如优先 1080P）。
- 网络超时秒数。
- daemon 启动/停止/重启按钮。

## 6. 本地数据

- 使用 Core Data 或 SwiftData 存储：
  - 历史记录：房间 ID、平台、标题、主播、最后播放时间、封面 URL。
  - 收藏：房间 ID、平台、自定义备注。
- 解析历史不保存真实流 URL（避免过期）。

## 7. 错误处理

- daemon 未启动：提示并引导用户在设置中配置/启动。
- 解析失败：展示 Rust 核心返回的错误信息，并提供“重试”。
- IINA 未安装：提示下载安装。
- 网络超时：可配置重试次数，默认 3 次。

## 8. 视觉方向

- 遵循 macOS 原生设计语言：侧边栏、工具栏、半透明效果、系统强调色。
- 深色/浅色模式自适应。
- 卡片使用轻量阴影与圆角，信息层级清晰。
- 字体：系统默认（SF Pro / PingFang SC）。

## 9. 安全与合规

- 不在客户端硬编码任何平台签名/密钥。
- 所有网络请求由 Rust 核心处理，Swift 侧仅透传用户输入。
- 不记录用户观看内容到远程服务器。

## 10. 验收标准

- [ ] 启动应用后自动检测/启动 daemon。
- [ ] 切换平台 Tab 可加载对应直播间列表。
- [ ] 搜索关键词可返回结果。
- [ ] 点击卡片进入详情页并展示清晰度列表。
- [ ] 选择清晰度后成功调用 IINA 播放。
- [ ] 支持直接粘贴 URL 解析。
- [ ] 深色/浅色模式外观正常。
- [ ] 历史记录与收藏可持久化。
