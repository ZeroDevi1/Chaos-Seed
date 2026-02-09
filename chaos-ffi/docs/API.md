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

返回 API 版本号。当前为 `1`。

### `char* chaos_ffi_version_json(void)`

返回：

```json
{"version":"0.1.0","git":"unknown","api":1}
```

### `char* chaos_ffi_last_error_json(void)`

如果没有错误信息则返回 `NULL`；否则返回：

```json
{"message":"...","context":"...optional..."}
```

### `void chaos_ffi_string_free(char* s)`

释放本库返回的字符串。

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
