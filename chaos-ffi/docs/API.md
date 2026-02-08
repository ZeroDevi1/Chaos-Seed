# chaos-ffi API

`chaos-ffi` is a small C ABI adapter over `chaos-core`. All public APIs are JSON-based to keep ABI stable across languages (WinUI 3 / C# / Qt / C++).

## Conventions

- **Encoding**: all `char*` are **UTF-8**.
- **Ownership**: any `char*` returned by this DLL/SO must be freed with `chaos_ffi_string_free`.
- **Errors**:
  - If a function fails, it returns `NULL` (or `-1` for `int32_t`).
  - Call `chaos_ffi_last_error_json()` to retrieve the last error as JSON (and clear it).
- **Threading**:
  - The library uses a global multi-thread tokio runtime internally.
  - Danmaku callbacks are invoked on a **background thread**. UI apps must marshal to the UI thread themselves.

## Base

### `uint32_t chaos_ffi_api_version(void)`

Returns the API version. Current: `1`.

### `char* chaos_ffi_version_json(void)`

Returns:

```json
{"version":"0.1.0","git":"unknown","api":1}
```

### `char* chaos_ffi_last_error_json(void)`

Returns `NULL` if no error is stored. Otherwise returns:

```json
{"message":"...","context":"...optional..."}
```

### `void chaos_ffi_string_free(char* s)`

Frees strings returned by this library.

## Subtitle (Thunder)

### `char* chaos_subtitle_search_json(...)`

Signature:

```c
char* chaos_subtitle_search_json(
  const char* query_utf8,
  uint32_t limit,
  double min_score_or_neg1,
  const char* lang_utf8_or_null,
  uint32_t timeout_ms);
```

Returns JSON array of `ThunderSubtitleItem` (directly from `chaos-core`).

Example element (shape only):

```json
{"name":"...","ext":"srt","url":"...","score":9.8,"languages":["zh","en"]}
```

### `char* chaos_subtitle_download_item_json(...)`

Signature:

```c
char* chaos_subtitle_download_item_json(
  const char* item_json_utf8,
  const char* out_dir_utf8,
  uint32_t timeout_ms,
  uint32_t retries,
  uint8_t overwrite);
```

Returns:

```json
{"path":"C:\\\\out\\\\file.srt","bytes":12345}
```

## Danmaku

### Event semantics

Events are JSON-serialized `DanmakuEvent` from `chaos-core`.

- `method == "LiveDMServer"`:
  - `text == ""` means connection OK (best-effort, IINA+-style semantics).
  - `text == "error"` means connection failure / disconnect.
- `method == "SendDM"`: actual danmaku message payload.

### `void* chaos_danmaku_connect(const char* input_utf8)`

Returns a handle pointer. On failure returns `NULL` (then read `last_error_json`).

### `char* chaos_danmaku_poll_json(void* handle, uint32_t max_events)`

Returns a JSON array of up to `max_events` events. If `max_events == 0`, a default of `50` is used.

### Callback

```c
typedef void (*chaos_danmaku_callback)(const char* event_json_utf8, void* user_data);
int32_t chaos_danmaku_set_callback(void* handle, chaos_danmaku_callback cb, void* user_data);
```

- Pass `cb = NULL` to disable callbacks.
- `event_json_utf8` pointer is only valid **during the callback call**.
- Callback is invoked from a background thread (not a UI thread).

### `int32_t chaos_danmaku_disconnect(void* handle)`

Stops the session, frees the handle, and guarantees no more callbacks after return.

