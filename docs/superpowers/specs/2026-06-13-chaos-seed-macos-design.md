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
- 不引入 `chaos-ffi` 或 `chaos-daemon` 作为运行依赖；macOS 应用使用纯 Swift 自包含实现。

## 3. 架构

```text
┌──────────────────────────────────────────────────────┐
│            Swift / AppKit macOS 应用                  │
│  HomeViewController → CategoryBar → RoomGrid          │
│       → RoomDetail → StreamResolver → IINA           │
└──────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────┐
│              LiveKit (纯 Swift 网络层)                │
│   BiliLive / Douyu / Huya 目录 + 直播源解析            │
│   URLSession · CryptoKit · Regex · JSONDecoder        │
└──────────────────────────────────────────────────────┘
```

### 3.1 为什么用 Swift 重写而不复用 Rust 核心

- macOS 应用零额外依赖，用户无需配置/启动任何后台进程。
- 避免 FFI/daemon 跨平台差异带来的调试成本。
- Swift 原生支持 `URLSession`、`async/await`、`Codable`、`CryptoKit`，在 macOS 上开发和调试体验一致。
- 参考来源：以 Chaos-Seed 的 `chaos-core` 为协议实现蓝本（`live_directory` + `livestream` 模块），保持 API 行为与数据形状对齐。

### 3.2 参考实现范围

从 `chaos-core` 迁移到 Swift 的核心模块包括：

| 平台 | 目录分类 | 推荐/分类房间 | 直播源解析 |
|---|---|---|---|
| BiliLive | `room/v1/Area/getList` | `second/getListByArea` + WBI 签名 | `room/v1/Room/playUrl` + codec 选择 |
| Douyu | 固定分类/搜索 | `cache.php?m=LiveList` | HTML 提取 room_id + `getH5Play` 加密接口 |
| Huya | `liveconfig/game/bussLive` | `cache.php?m=LiveList` | `cache.php?m=Live&do=profileRoom` + anti-code URL |

需同步迁移的辅助模块：
- Bili WBI 签名（`bili_wbi.rs`）。
- Douyu 加密/auth（`douyu_auth.rs`）。
- Huya anti-code URL 拼接（`huya_url.rs`）。
- 通用 HTML JSON 对象提取（`brace_extract.rs`）。

## 4. 数据流

### 4.1 加载分类列表

1. 切换平台 Tab 时，调用 `LiveKit.getCategories(platform)`。
2. 返回 `LiveCategory` 数组，每个分类包含 `id`、`name`、`children`（子分类）。
3. UI 在平台 Tab 下方渲染一级/二级分类 Tab；默认选中「推荐」或第一个分类。

### 4.2 首页加载直播间列表

1. UI 调用 `LiveKit.getRooms(platform, category, subCategory?, page, pageSize)`：
   - 参数：`platform: .biliLive | .douyu | .huya`, `categoryId?: String`, `subCategoryId?: String`, `page: Int`, `pageSize: Int`
   - 返回：`{ rooms: [{ id, title, coverUrl, streamerName, viewerCount, platform, input }], hasMore }`
2. UI 以瀑布流/网格展示卡片。

### 4.3 搜索

1. UI 调用 `LiveKit.searchRooms(platform, keyword, page, pageSize)`。
2. 返回与 `getRooms` 相同。
3. 搜索时隐藏分类 Tab，仅保留平台 Tab 与结果列表。

### 4.4 进入直播间详情

1. UI 调用 `LiveKit.decodeManifest(input)`：
   - 参数：`input` 为直播间 URL 或平台前缀（如 `bilibili:12345`）。
   - 返回：`LiveManifest { site, roomId, title, streamer, isLiving, variants: [StreamVariant] }`。
2. UI 展示清晰度/线路列表；每个 variant 可能已带 `url`。

### 4.5 直播流播放

1. 用户选择 variant：
   - 若 variant 已有 `url`，直接使用。
   - 若只有 `variantId`（如 BiliLive 的 qn），调用 `LiveKit.resolveVariant(site, roomId, variantId)` 获取最终 URL 与 `PlaybackHints`。
2. UI 启动 IINA：
   - 优先使用 `NSWorkspace.shared.open(_:)` 打开 URL。
   - 若 `PlaybackHints` 包含 Referer/UA/Cookie，使用 `Process` 执行 IINA CLI 并传入对应 HTTP header。

## 5. 页面结构

### 5.1 主窗口

- 最小尺寸：900 × 600。
- 左侧导航栏（可收起）：首页、历史、设置。
- 右侧内容区根据导航切换。

### 5.2 首页

- 顶部工具栏：
  - 平台 Segmented Control：BiliLive / Douyu / Huya。
  - 搜索框（回车触发搜索）。
  -「解析 URL」按钮。
  - 刷新按钮。
- 分类栏（平台 Tab 下方）：
  - 一级分类横向滚动 Tab（如 推荐 / 网游 / 手游 / 单机 / 娱乐）。
  - 选中一级分类后，下方或右侧展示二级子分类（如 网游 → 英雄联盟 / DOTA2 / CS2）。
  - 默认选中「推荐」，展示该平台推荐房间。
- 主体：
  - 直播间卡片网格（2–4 列自适应）。
  - 每个卡片显示封面、标题、主播名、在线人数。
  - 底部翻页：上一页 / 下一页 / 页码。
- 空状态：首次加载提示、分类无房间、搜索无结果、网络错误重试。

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

- IINA 应用路径（默认 `/Applications/IINA.app`）。
- 默认清晰度偏好（例如优先 原画 / 蓝光 / 超清）。
- 网络超时秒数。
- 调试日志开关。

## 6. 本地数据

- 使用 Core Data 或 SwiftData 存储：
  - 历史记录：房间 ID、平台、标题、主播、最后播放时间、封面 URL。
  - 收藏：房间 ID、平台、自定义备注。
- 解析历史不保存真实流 URL（避免过期）。

## 7. 错误处理

- 解析失败：展示 Swift 网络层返回的错误信息，并提供“重试”。
- IINA 未安装：提示下载安装。
- 网络超时：可配置重试次数，默认 3 次。
- 平台反爬拦截：BiliLive 遇到 -352/-412 时自动刷新 buvid + WBI key 并重试一次。

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

- [ ] 切换平台 Tab 后自动加载分类栏。
- [ ] 选择一级/二级分类可加载对应直播间列表。
- [ ] 搜索关键词可返回结果。
- [ ] 点击卡片进入详情页并展示清晰度列表。
- [ ] 选择清晰度后成功调用 IINA 播放。
- [ ] 支持直接粘贴 URL 解析。
- [ ] 深色/浅色模式外观正常。
- [ ] 历史记录与收藏可持久化。
- [ ] Swift 网络层可独立编译运行，不依赖 Rust 核心。
