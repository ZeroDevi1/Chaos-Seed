# TODO（长期路线图 / 方向）

> 说明：这里放“可能需要数周以上”的大方向/大功能；以 Title 为主，最多一行补充说明。
> 近期几天内要交付的 bugfix/小功能请放到 `TODO_NEXT.md`。

## UI/UX（Win11 现代化）
- Win11 现代化布局适配（侧边栏、标题栏、动效、主题一致性）
- Fluent 风格 icon（字体或矢量资源）替换占位符
- 侧边栏折叠状态持久化（下次启动保持）
- 系统深色/浅色偏好跟随（可手动覆盖）
- 主题与 Backdrop（Mica）在浅/深色下的一致性（避免“割裂/透底”）
- Fluent Web Components 视觉/交互进一步贴近系统（间距、字体、边框、hover）

## 播放（直播源解析与播放）
- 直播源解析（core/ffi）已支持 Huya / Douyu / BiliLive；Tauri UI 已接入：解析（manifest/variants）+ 清晰度/线路切换 + 新窗口播放（Hls.js/AvPlayer）
- 直播源解析：支持常见聚合格式（例如 m3u / json），并提供分组/搜索
- 播放器接入（持续演进）：更完善的播放控制栏/快捷键/状态面板（码率、延迟、丢帧等）
- 播放体验：错误提示、自动重试与 CDN failover 策略完善（不同平台/线路的兼容性）
- 反盗链（必要时）：本地代理注入 Referer/UA/Cookie；或对 HLS 请求做 header 注入
- 获取实时播放的音频/媒体信息（Win11 System Media Session），用于本地音乐自动匹配在线歌词

## 歌词（实时 / 桌面歌词）
- 歌词页（Tauri）：BetterLyrics 三源对齐（QQ/网易云/LRCLIB）+ 匹配阈值自动搜索 + time-line 高亮（已完成基线；持续优化交互/观感）
- 桌面歌词：Dock（贴边侧边栏）/ Float（悬浮挂件）作为主力显示方式（已完成基线；后续做多屏/快捷键/更精细的字体与排版控制）
- 低功耗播放事件（Windows）：从“自适应轮询”升级为 WinRT 事件订阅（SMTC/GSMTC SessionsChanged/TimelineChanged 等），进一步降低空闲功耗
- 歌词特效：更丰富/更稳定的背景与布局效果（在不引入大依赖的前提下持续演进）

## 弹幕（悬浮窗/滚动）
- 弹幕源接入（BiliLive / Douyu / Huya 核心连接与解析已完成 2026-02-07，commit: `a37fce7`）
- 弹幕 UI 接入（Chat / Overlay）与交互（已完成；持续优化性能/观感）
- 直播页：画面内 Overlay 弹幕（开关 + 显示区域/透明度/字号/同屏密度；已完成 `0.2.3`），悬浮窗后续补齐同款设置
- 悬浮窗：置顶、透明度、字体/速度/密度可调
- 多屏/多显示器适配与快捷键控制

## 字幕下载（长期增强方向）
- 字幕下载页基础可用性：搜索结果列表显示 / 输入鼠标交互聚焦（已完成 2026-02-07，详见 `TODO_NEXT.md`）
- 字幕下载体验完善（排序、过滤、下载管理、失败重试、历史记录）
- 下载后动作（打开目录、复制路径、关联视频文件等）

## 工程化 / 发布
- 配置系统（保存主题/侧边栏/常用设置）
- chaos-ffi：导出 chaos-core 为 dll/so（跨语言稳定 ABI，供 WinUI3/Qt 等调用）
- WinUI3（已可用）：直播页 + 播放（Flyleaf/FFmpeg）+ 弹幕；弹幕页（独立连接/断开，支持 Chat/Overlay 两窗口）；歌词页（Now Playing + 多源歌词搜索，支持 daemon/FFI）。后续再做：WinRT 真事件订阅（SMTC/GSMTC）、UI polish、更多窗口形态（Dock/Float）
- CI（Windows 原生构建 + WSL 交叉构建校验）
- Release 打包（版本号、校验和、更新日志、tag 触发自动上传 Release 产物）
