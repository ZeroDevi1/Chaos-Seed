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

返回 API 版本号。当前为 `3`。

### `char* chaos_ffi_version_json(void)`

返回：

```json
{"version":"0.1.0","git":"unknown","api":3}
```

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

返回：

```json
{"path":"C:\\\\out\\\\file.srt","bytes":12345}
```

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

说明：
- 该函数会 **内部先 decode manifest** 来拿到 canonical `room_id`（例如斗鱼真实 rid / B 站长号），再进行二段解析；
  因此性能上比直接传 `(site, room_id, variant_id)` 略慢。

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

## 弹幕（Danmaku）

### 事件语义

事件为 `chaos-core` 的 `DanmakuEvent` JSON 序列化结果。

- `method == "LiveDMServer"`：
  - `text == ""` 表示连接 OK（best-effort，对齐 IINA+ 语义）。
  - `text == "error"` 表示连接失败 / 断线。
- `method == "SendDM"`：实际弹幕消息事件。

### `void* chaos_danmaku_connect(const char* input_utf8)`

返回一个 handle 指针。失败返回 `NULL`（再读取 `last_error_json`）。

### `char* chaos_danmaku_poll_json(void* handle, uint32_t max_events)`

返回最多 `max_events` 条事件的 JSON 数组。如果 `max_events == 0`，默认取 `50`。

### 回调

```c
typedef void (*chaos_danmaku_callback)(const char* event_json_utf8, void* user_data);
int32_t chaos_danmaku_set_callback(void* handle, chaos_danmaku_callback cb, void* user_data);
```

- 传入 `cb = NULL` 可关闭回调。
- `event_json_utf8` 指针仅在**回调执行期间**有效（回调返回后 Rust 会释放）。
- 回调在后台线程触发（不是 UI 线程）。

### `int32_t chaos_danmaku_disconnect(void* handle)`

停止 session、释放 handle，并保证函数返回后不再触发回调。
