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
