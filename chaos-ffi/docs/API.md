# chaos-ffi API（中文说明）

`chaos-ffi` 是 `chaos-core` 的 C ABI 适配层。为了在 WinUI 3 / C# / Qt / C++ 等多语言环境下保持 ABI 稳定，所有对外接口统一采用 **JSON** 作为数据载体。

## 约定

- **编码**：所有 `char*` 均为 **UTF-8**。
- **内存所有权**：DLL/SO 返回的 `char*` 由调用方负责释放，必须调用 `chaos_ffi_string_free`。
- **错误处理**：
  - 失败时返回 `NULL`（或 `int32_t` 返回 `-1`）。
  - 再调用 `chaos_ffi_last_error_json()` 获取最近一次错误的 JSON（获取后会清空）。
- **线程模型**：
  - 库内部使用一个全局 multi-thread tokio runtime。
  - 弹幕回调会在**后台线程**触发；UI 程序需要自行 marshal 到 UI 线程。

## 基础

### `uint32_t chaos_ffi_api_version(void)`

返回 API 版本号。当前为 `7`。

### `char* chaos_ffi_version_json(void)`

返回：

```json
{"version":"0.4.6","git":"unknown","api":8}
```

当前为：
- `api = 8`（新增 bilibili 普通视频下载：登录/刷新/解析/下载任务）。

### `char* chaos_ffi_last_error_json(void)`

如果没有错误信息则返回 `NULL`；否则返回：

```json
{"message":"...","context":"...optional..."}
```

### `void chaos_ffi_string_free(char* s)`

释放本库返回的字符串。

## 系统媒体（Win11 Now Playing）

Windows 11/10+ 提供系统级的媒体会话（GSMTC / Global System Media Transport Controls）。本接口用于获取当前系统中的媒体 sessions，并返回“推荐的正在播放会话（best-effort）”。

### `char* chaos_now_playing_snapshot_json(uint8_t include_thumbnail, uint32_t max_thumbnail_bytes, uint32_t max_sessions)`

签名：

```c
char* chaos_now_playing_snapshot_json(
  uint8_t include_thumbnail,
  uint32_t max_thumbnail_bytes,
  uint32_t max_sessions);
```

- `include_thumbnail`：`1` 表示读取封面缩略图并以 base64 输出；`0` 不读取封面（更快）。
- `max_thumbnail_bytes`：封面最大读取字节数（建议 `262144`=256KB）。
- `max_sessions`：最多返回多少个会话（建议 `32`）。

返回 `NowPlayingSnapshot` JSON（字段形状）：

```json
{
  "supported": true,
  "retrieved_at_unix_ms": 0,
  "picked_app_id": "Spotify",
  "now_playing": {
    "app_id": "Spotify",
    "is_current": true,
    "playback_status": "Playing",
    "title": "Song",
    "artist": "Artist",
    "album_title": "Album",
    "position_ms": 1234,
    "duration_ms": 234567,
    "thumbnail": { "mime": "image/png", "base64": "..." },
    "error": null
  },
  "sessions": []
}
```

说明：
- **非 Windows 平台**：不会报错；返回 `supported=false` 且 `sessions=[]`，`now_playing=null`。
- **无媒体会话**：`sessions=[]`，`now_playing=null`。

## 音乐（歌曲搜索 / 登录 / 下载）

说明：
- 字段形状与 `chaos-proto` 的 music DTO 对齐（`camelCase`）。
- `service` 取值：`qq | kugou | netease | kuwo`。
- 合规边界：仅下载“接口返回的可直接下载 URL”，不包含任何 DRM 解密/绕过逻辑；若无权限/登录失效/接口不返回 URL，会返回明确错误。

### 配置

#### `char* chaos_music_config_set_json(const char* config_json_utf8)`

输入：`MusicProviderConfig` JSON，例如：

```json
{
  "kugouBaseUrl": null,
  "neteaseBaseUrls": ["http://127.0.0.1:3001"],
  "neteaseAnonymousCookieUrl": "/register/anonimous"
}
```

返回：`OkReply` JSON：

```json
{ "ok": true }
```

说明：
- 酷狗：`kugouBaseUrl` 已废弃并被忽略（为向后兼容保留字段）；酷狗能力已内置直连，无需配置。
- 网易云：若 `neteaseBaseUrls` 为空，会使用内置的一组可用 API 列表；也可通过 config 覆盖。

### 搜索 / 列表

- `char* chaos_music_search_tracks_json(const char* params_json_utf8)` -> `MusicTrack[]`
- `char* chaos_music_search_albums_json(const char* params_json_utf8)` -> `MusicAlbum[]`
- `char* chaos_music_search_artists_json(const char* params_json_utf8)` -> `MusicArtist[]`
- `char* chaos_music_album_tracks_json(const char* params_json_utf8)` -> `MusicTrack[]`
- `char* chaos_music_artist_albums_json(const char* params_json_utf8)` -> `MusicAlbum[]`

其中 `params_json_utf8` 为对应的 params DTO（如 `MusicSearchParams` / `MusicAlbumTracksParams`）。

### 播放 URL（预览）

- `char* chaos_music_track_play_url_json(const char* params_json_utf8)` -> `MusicTrackPlayUrlResult`

其中 `params_json_utf8` 为 `MusicTrackPlayUrlParams` JSON。

### 登录（QQ 音乐）

- `char* chaos_music_qq_login_qr_create_json(const char* login_type_utf8)`：`login_type_utf8 = "qq" | "wechat"`，返回 `MusicLoginQr`（含二维码 base64）。
- `char* chaos_music_qq_login_qr_poll_json(const char* session_id_utf8)`：轮询返回 `MusicLoginQrPollResult`；成功时 `cookie` 非空。
- `char* chaos_music_qq_refresh_cookie_json(const char* cookie_json_utf8)`：输入 `QqMusicCookie` JSON，返回更新后的 `QqMusicCookie` JSON。

### 登录（酷狗）

- `char* chaos_music_kugou_login_qr_create_json(const char* login_type_utf8)`：不再依赖 `kugouBaseUrl`；返回 `MusicLoginQr`。
- `char* chaos_music_kugou_login_qr_poll_json(const char* session_id_utf8)`：成功时 `kugouUser` 非空。

### 下载（阻塞）

#### `char* chaos_music_download_blocking_json(const char* start_params_json_utf8)`

输入：`MusicDownloadStartParams` JSON；返回：`MusicDownloadStatus` JSON（包含每个 job 的结果与错误信息）。

说明：
- `target` 字段（`MusicDownloadTarget`）使用 `camelCase`：`albumId` / `artistId`（仍兼容输入 `album_id` / `artist_id`）。
- `options.pathTemplate`：若提供则使用模板生成文件名（与 daemon 行为对齐）。
- 下载音频成功后，会 best-effort 额外下载同名 `.lrc`（不影响音频下载结果）。

### 下载（任务 / 可轮询）

适用于 UI 或移动端：Start 立即返回 `sessionId`，然后通过轮询 `status` 获取进度；可随时 `cancel`。

- `char* chaos_music_download_start_json(const char* start_params_json_utf8)` -> `MusicDownloadStartResult`
- `char* chaos_music_download_status_json(const char* session_id_utf8)` -> `MusicDownloadStatus`
- `char* chaos_music_download_cancel_json(const char* session_id_utf8)` -> `OkReply`

## Bilibili 视频（BV/AV）下载（MVP）

说明：
- MVP 仅支持普通 BV/AV（含多P）；番剧/课程/合集等后续里程碑。
- 字段形状对齐 `chaos-proto` 的 Bili DTO（`camelCase`）。
- 合规边界：仅调用 B 站公开/官方接口获取资源 URL 并下载；不包含任何 DRM 绕过逻辑。

### 登录（WEB 二维码）

- `char* chaos_bili_login_qr_create_json(void)` -> `BiliLoginQr`（含二维码 base64）
- `char* chaos_bili_login_qr_poll_json(const char* session_id_utf8)` -> `BiliLoginQrPollResult`（成功时 `auth` 非空，含 `cookie + refreshToken`）

### Cookie 刷新

- `char* chaos_bili_refresh_cookie_json(const char* params_json_utf8)` -> `BiliRefreshCookieResult`

其中 `params_json_utf8` 为 `BiliRefreshCookieParams` JSON（包含 `auth`）。

### 解析（BV/AV）

- `char* chaos_bili_parse_json(const char* params_json_utf8)` -> `BiliParseResult`

### 下载（任务 / 可轮询）

- `char* chaos_bili_download_start_json(const char* params_json_utf8)` -> `BiliDownloadStartResult`
- `char* chaos_bili_download_status_json(const char* session_id_utf8)` -> `BiliDownloadStatus`
- `char* chaos_bili_download_cancel_json(const char* session_id_utf8)` -> `OkReply`

## 字幕（Thunder）

### `char* chaos_subtitle_search_json(...)`

签名：

```c
char* chaos_subtitle_search_json(
  const char* query_utf8,
  uint32_t limit,
  double min_score_or_neg1,
  const char* lang_utf8_or_null,
  uint32_t timeout_ms);
```

参数：
- `query_utf8`：搜索关键字（必填，UTF-8；`NULL/空串/全空白` 会失败）。
- `limit`：最多返回多少条（最小为 1；内部会 `max(1)`）。
- `min_score_or_neg1`：最小评分过滤；传负数（例如 `-1.0`）表示不启用过滤。
- `lang_utf8_or_null`：语言过滤（可选，UTF-8），如 `"zh"` / `"en"`；传 `NULL/空串` 表示不过滤。
- `timeout_ms`：单次请求超时（ms，最小为 1；内部会 `max(1)`）。

返回 `ThunderSubtitleItem` 的 JSON 数组（直接序列化 `chaos-core` 中的结构）。

示例元素（仅展示字段形状）：

```json
{"name":"...","ext":"srt","url":"...","score":9.8,"languages":["zh","en"]}
```

### `char* chaos_subtitle_download_item_json(...)`

签名：

```c
char* chaos_subtitle_download_item_json(
  const char* item_json_utf8,
  const char* out_dir_utf8,
  uint32_t timeout_ms,
  uint32_t retries,
  uint8_t overwrite);
```

参数：
- `item_json_utf8`：`ThunderSubtitleItem` 的 JSON（必填；通常来自 `chaos_subtitle_search_json` 返回数组中的元素）。
- `out_dir_utf8`：输出目录（必填，UTF-8；目录不存在时行为由 core 决定，建议调用方先创建）。
- `timeout_ms`：下载超时（ms，最小为 1；内部会 `max(1)`）。
- `retries`：失败重试次数（原样传递给 core）。
- `overwrite`：`1` 覆盖同名文件；`0` 不覆盖。

返回：

```json
{"path":"C:\\\\out\\\\file.srt","bytes":12345}
```

## 歌词（Lyrics Search）

从多个歌词源搜索并拉取歌词文本，输出按 `quality` 排序的候选列表（可选 strict match 过滤）。对外仍然保持 **JSON in/out** 的 ABI 稳定设计。

### `char* chaos_lyrics_search_json(...)`

签名：

```c
char* chaos_lyrics_search_json(
  const char* title_utf8,
  const char* album_utf8_or_null,
  const char* artist_utf8_or_null,
  uint32_t duration_ms_or_0,
  uint32_t limit,
  uint8_t strict_match,
  const char* services_csv_utf8_or_null,
  uint32_t timeout_ms);
```

参数：
- `title_utf8`：歌名（必填，UTF-8）。
- `album_utf8_or_null`：专辑名（可选，UTF-8）。
- `artist_utf8_or_null`：歌手名（可选，UTF-8）。为空时会降级为 keyword 搜索。
- `duration_ms_or_0`：期望时长（ms）；传 `0` 表示未知。
- `limit`：最多返回多少条结果（最小为 1）。
- `strict_match`：`1` 表示启用严格匹配过滤（等价 LyricsX 的 strictSearchEnabled：过滤 `matched=false`）。
- `services_csv_utf8_or_null`：指定歌词源（逗号分隔），例如 `"netease,qq,kugou"`；`NULL/空串` 表示默认全部源。
- `timeout_ms`：每个源/请求的超时（ms，最小为 1）。

返回：`LyricsSearchResult` 的 JSON 数组（按 `quality` 降序），元素字段形状示例：

```json
[
  {
    "service": "qq",
    "service_token": "003rJQ7o3S0YdK",
    "title": "Hello",
    "artist": "Adele",
    "album": "Hello",
    "duration_ms": 296000,
    "quality": 1.23,
    "matched": true,
    "has_translation": true,
    "has_inline_timetags": false,
    "lyrics_original": "[00:01.00] ...",
    "lyrics_translation": "[00:01.00] ..."
  }
]
```

说明：
- 任一歌词源失败/超时不会导致整体失败；返回结果可能为空数组。
- `quality`/`matched` 由 core 侧根据请求与返回内容计算并排序。

## 直播源解析（Livestream）

`chaos-core` 内已实现虎牙/斗鱼/B站直播（BiliLive）的直播源解析；`chaos-ffi` 将其以 **JSON in/out** 方式导出，方便 C/C#/Qt 等调用。

### 设计目标

- **ABI 稳定**：对外只暴露 C ABI，数据统一用 JSON 传递。
- **与 UI 解耦**：core 只负责解析直播源；UI 自己决定怎么展示清晰度/线路、怎么播放。
- **可扩展**：未来增加平台只需要在 `chaos-core` 扩展平台模块，FFI 仍然沿用同一套 JSON 结构。

### 关键结构（概念）

一次 `decode_manifest` 返回一个 `LiveManifest`（JSON）：
- `site`：平台（`BiliLive` / `Douyu` / `Huya`）
- `room_id`：canonical room id（例如 BiliLive 会从短号解析成长号）
- `info`：标题/主播头像/封面/是否开播（best-effort）
- `playback`：播放提示（例如 `referer` / `user_agent`，供播放器设置）
- `variants`：清晰度/线路列表（每个元素是 `StreamVariant`）

`StreamVariant`（清晰度/线路项）：
- `id`：稳定 id，用于“二段解析”（例如 `bili_live:2000:原画`）
- `label`：展示名（原画/蓝光/高清…）
- `quality`：排序用数值（BiliLive=qn；Huya=bitrate；Douyu=bit）
- `rate`：斗鱼专用字段（用于二次请求补齐 URL）
- `url` / `backup_urls`：最终可播放地址及备选地址（可能为 `null`，见下文）

### 为什么需要二段解析（resolve variant）

部分平台的接口行为是：
- 第一次请求只返回“当前默认清晰度”的可播放 URL；
- 其它清晰度只给一个标识（比如 `rate/qn`），需要带着这个标识再请求一次才会返回 URL。

因此 FFI 层提供两步：
- `decode_manifest`：拿到 manifest + variants（**可能部分 variant 没有 url**）
- `resolve_variant`：根据 `variant_id` 补齐特定清晰度的 `url/backup_urls`

这样 UI 可以先快速展示清晰度列表，再在用户切换清晰度时按需补全 URL。

补充说明（BiliLive）：
- 部分直播间的 `getRoomPlayInfo` 接口会**忽略**传入的 `qn`（即使请求“原画/蓝光”，回包 `current_qn` 仍是较低档）。
- 因此 core 在补齐指定清晰度 URL 时，会在必要时回退到 `room/v1/Room/playUrl`，并且只在“请求的 `qn` 与回包的 `current_qn` 一致”时才绑定 URL，避免“高标签但低清 URL”的错配。
- 若目标清晰度不可达（例如请求 4K/2K 最终回落到原画），则该清晰度会被视为不可访问：`resolve_variant*` 会返回错误（或该变体 `url` 仍为 `null`，取决于调用路径与参数）。

### `char* chaos_livestream_decode_manifest_json(const char* input_utf8, uint8_t drop_inaccessible_high_qualities)`

- `input_utf8`：支持完整 URL 或平台前缀（复用 `chaos-core` 的解析规则）
  - `<BILILIVE_URL>`
  - `<HUYA_URL>`
  - `bilibili:<ROOM_ID>` / `huya:<ROOM_ID>` / `douyu:<ROOM_ID>`
- `drop_inaccessible_high_qualities`：
  - `1`（默认推荐）：对齐 IINA+ 行为：当已拿到某个画质的可播放 URL 时，丢弃“更高但当前无 URL”的画质项
  - `0`：保留所有画质项（即使 `url == null`）

返回 `LiveManifest` 的 JSON（字段形状）：

```json
{
  "site": "BiliLive",
  "room_id": "<ROOM_ID>",
  "raw_input": "<BILILIVE_URL>",
  "info": {
    "title": "...",
    "name": "...",
    "avatar": "...",
    "cover": "...",
    "is_living": true
  },
  "playback": {
    "referer": "https://live.bilibili.com/",
    "user_agent": null
  },
  "variants": [
    {
      "id": "bili_live:2000:原画",
      "label": "原画",
      "quality": 2000,
      "rate": null,
      "url": "https://...",
      "backup_urls": ["https://..."]
    }
  ]
}
```

### `char* chaos_livestream_resolve_variant_json(const char* input_utf8, const char* variant_id_utf8)`

用于“二段解析”补齐 URL（主要是 BiliLive / Douyu 的部分画质需要二次请求）。

典型流程：
1) 调用 `chaos_livestream_decode_manifest_json(input, 1)` 获取 `variants`
2) 选择一个 `variants[i].id`（例如 `bili_live:2000:原画` 或 `douyu:2:原画`）
3) 调用 `chaos_livestream_resolve_variant_json(input, variant_id)` 获取补齐后的 `StreamVariant`

参数：
- `input_utf8`：与 `decode_manifest` 相同（必填，UTF-8；空串会失败）。
- `variant_id_utf8`：从 `manifest.variants[i].id` 中取（必填，UTF-8；空串会失败）。

说明：
- 该函数会 **内部先 decode manifest** 来拿到 canonical `room_id`（例如斗鱼真实 rid / B 站长号），再进行二段解析；
  因此性能上比直接传 `(site, room_id, variant_id)` 略慢。

返回：`StreamVariant` JSON（字段形状同 `resolve_variant2_json` 的示例）。

### `char* chaos_livestream_resolve_variant2_json(const char* site_utf8, const char* room_id_utf8, const char* variant_id_utf8)`

推荐使用的“二段解析”接口：当你已经从 `LiveManifest` 中拿到了 `site` + canonical `room_id` 时，直接用它们解析指定清晰度的 URL。

参数说明：
- `site_utf8`：推荐直接传 `manifest.site`（例如 `BiliLive` / `Douyu` / `Huya`）；同时也兼容 `bili_live` / `douyu` / `huya` 等小写别名。
- `room_id_utf8`：必须是 canonical room id（推荐直接传 `manifest.room_id`）。
- `variant_id_utf8`：从 `manifest.variants[i].id` 中取。

典型流程：
1) `decode_manifest` 得到 `manifest.site` + `manifest.room_id` + `variants[i].id`
2) 调用 `chaos_livestream_resolve_variant2_json(manifest.site, manifest.room_id, variants[i].id)`

返回 `StreamVariant` JSON（字段形状）：

```json
{
  "id": "douyu:2:原画",
  "label": "原画",
  "quality": 2000,
  "rate": 2,
  "url": "https://...",
  "backup_urls": ["https://..."]
}
```

### 典型调用流程（伪代码）

1) 解析 manifest：

```c
char* s = chaos_livestream_decode_manifest_json("<BILILIVE_URL>", 1);
// s 是 LiveManifest JSON，解析后拿到 variants 列表
```

2) 用户选择清晰度（拿到 `variant_id`），补齐 URL：

```c
char* v = chaos_livestream_resolve_variant_json("<BILILIVE_URL>", "bili_live:2000:原画");
// v 是 StreamVariant JSON，包含 url + backup_urls
```

3) 播放器侧建议：
- 使用 `manifest.playback.referer` / `manifest.playback.user_agent` 作为请求头配置
- 优先使用 `variant.url`；失败时依次尝试 `backup_urls`

### 常见问题

#### 返回 `NULL` 怎么排查？

任何失败都会返回 `NULL`。请立即调用：

```c
char* err = chaos_ffi_last_error_json();
```

拿到 JSON 错误信息（message/context），并在读取后调用 `chaos_ffi_string_free(err)` 释放。

#### 真实站点集成测试为什么要 feature gate？

真实站点可能受网络波动/风控/开播状态影响。默认 `cargo test` 必须稳定、离线可跑；
因此真实 URL 测试放在 `--features live-tests` 下，按需手动运行。

## 直播目录（Live Directory）

用于实现 WinUI3 的“首页/分类”目录能力：平台 Tab + 推荐/分区列表 + 站内搜索，输出统一的卡片数据结构。

约定：
- `site`：平台字符串：`bili_live` / `huya` / `douyu`（与 WinUI3/daemon 的约定一致）。
- `page`：从 `1` 开始。
- `input`：可直接传给现有直播解析/播放链路的字符串（例如 `bilibili:<rid>` / `huya:<rid>` / `douyu:<rid>`）。

### `char* chaos_live_dir_categories_json(const char* site_utf8)`

参数：
- `site_utf8`：平台字符串（必填，UTF-8）。支持：`bili_live` / `huya` / `douyu`（也兼容 `bili`/`bl`/`bililive`、`hy`、`dy` 等别名）。

返回 `LiveDirCategory[]` JSON（字段形状）：

```json
[
  {
    "id": "1",
    "name": "网游",
    "children": [
      { "id": "11", "parentId": "1", "name": "英雄联盟", "pic": "https://...@100w.png" }
    ]
  }
]
```

### `char* chaos_live_dir_recommend_rooms_json(const char* site_utf8, uint32_t page)`

参数：
- `site_utf8`：同上。
- `page`：页码（从 `1` 开始；内部会 `max(1)`）。

返回 `LiveDirRoomListResult` JSON（字段形状）：

```json
{
  "hasMore": true,
  "items": [
    {
      "site": "bili_live",
      "roomId": "100",
      "input": "bilibili:100",
      "title": "标题",
      "cover": "https://...@400w.jpg",
      "userName": "主播",
      "online": 12345
    }
  ]
}
```

### `char* chaos_live_dir_category_rooms_json(const char* site_utf8, const char* parent_id_utf8_or_null, const char* category_id_utf8, uint32_t page)`

按分区拉取房间卡片列表。

- B 站（`bili_live`）需要 `parent_id` 与 `category_id`（对应 “一级分区 id / 二级分区 id”）。
- 虎牙/斗鱼通常只需要 `category_id`；`parent_id` 可传 `NULL`。

参数：
- `site_utf8`：同上。
- `parent_id_utf8_or_null`：一级分区 id（可选，UTF-8；B 站推荐必传）。
- `category_id_utf8`：分区 id（必填，UTF-8；空串会失败）。
- `page`：页码（从 `1` 开始；内部会 `max(1)`）。

返回：同 `LiveDirRoomListResult`。

#### B 站风控/拦截说明（-352 / -412）

在部分网络环境下，B 站接口可能返回 `code = -352` 或 `code = -412`（常见于请求被拦截/签名或设备校验）。

`chaos-core` 在实现上会做 best-effort 的自动处理（例如刷新 buvid / wbi key，并在必要时回退到更稳定的接口）；如果仍然失败：
- 请稍后重试（可能是临时风控/频控）。
- 尝试更换网络环境（公司代理/透明代理有时会触发拦截）。

### `char* chaos_live_dir_search_rooms_json(const char* site_utf8, const char* keyword_utf8, uint32_t page)`

站内搜索（仅当前平台）。

参数：
- `site_utf8`：同上。
- `keyword_utf8`：关键字（必填，UTF-8；空串会失败）。
- `page`：页码（从 `1` 开始；内部会 `max(1)`）。

返回：同 `LiveDirRoomListResult`。

## 弹幕（Danmaku）

### 事件语义

事件为 `chaos-core` 的 `DanmakuEvent` JSON 序列化结果。

- `method == "LiveDMServer"`：
  - `text == ""` 表示连接 OK（best-effort，对齐 IINA+ 语义）。
  - `text == "error"` 表示连接失败 / 断线。
- `method == "SendDM"`：实际弹幕消息事件。

### `void* chaos_danmaku_connect(const char* input_utf8)`

返回一个 handle 指针。失败返回 `NULL`（再读取 `last_error_json`）。

参数：
- `input_utf8`：直播间输入（必填，UTF-8；支持完整 URL 或平台前缀，复用 core 的解析规则）。

返回值：
- 成功：非 `NULL` 的 handle 指针（后续传给 `set_callback` / `poll_json` / `disconnect`）。
- 失败：`NULL`；调用 `chaos_ffi_last_error_json()` 获取错误 JSON。

### `char* chaos_danmaku_poll_json(void* handle, uint32_t max_events)`

返回最多 `max_events` 条事件的 JSON 数组。如果 `max_events == 0`，默认取 `50`。

参数：
- `handle`：必须是 `chaos_danmaku_connect` 返回的非空指针；传 `NULL` 会失败。
- `max_events`：最多返回多少条；`0` 表示默认 `50`。

返回值：
- 成功：事件数组 JSON（可能是 `[]`）。
- 失败：返回 `NULL`；调用 `chaos_ffi_last_error_json()` 获取错误 JSON。

### 回调

```c
typedef void (*chaos_danmaku_callback)(const char* event_json_utf8, void* user_data);
int32_t chaos_danmaku_set_callback(void* handle, chaos_danmaku_callback cb, void* user_data);
```

- 传入 `cb = NULL` 可关闭回调。
- 关闭回调示例：

```c
chaos_danmaku_set_callback(handle, NULL, NULL);
```
- `event_json_utf8` 指针仅在**回调执行期间**有效（回调返回后 Rust 会释放）。
- 回调在后台线程触发（不是 UI 线程）。
- 返回值：
  - `0`：成功。
  - `-1`：失败；请调用 `chaos_ffi_last_error_json()` 获取错误 JSON。

### `int32_t chaos_danmaku_disconnect(void* handle)`

停止 session、释放 handle，并保证函数返回后不再触发回调。

参数：
- `handle`：必须是 `chaos_danmaku_connect` 返回的非空指针；调用成功后 handle 失效，不可复用。

返回值：
- `0`：成功。
- `-1`：失败；请调用 `chaos_ffi_last_error_json()` 获取错误 JSON（读取后会清空），并使用 `chaos_ffi_string_free` 释放。

补充说明：
- `disconnect` 内部会先停止 core session，再 join 后台转发线程，因此 **保证返回后不再触发回调**。
