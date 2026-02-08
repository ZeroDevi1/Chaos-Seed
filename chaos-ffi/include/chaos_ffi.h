#pragma once

#include <stdint.h>

#ifdef _WIN32
#  define CHAOS_FFI_EXPORT __declspec(dllimport)
#else
#  define CHAOS_FFI_EXPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif

// All returned `char*` are UTF-8 and must be freed by `chaos_ffi_string_free`.

CHAOS_FFI_EXPORT uint32_t chaos_ffi_api_version(void);
CHAOS_FFI_EXPORT char* chaos_ffi_version_json(void);
CHAOS_FFI_EXPORT char* chaos_ffi_last_error_json(void);
CHAOS_FFI_EXPORT void chaos_ffi_string_free(char* s);

// Subtitle (Thunder) - JSON in/out
CHAOS_FFI_EXPORT char* chaos_subtitle_search_json(
    const char* query_utf8,
    uint32_t limit,
    double min_score_or_neg1,
    const char* lang_utf8_or_null,
    uint32_t timeout_ms);

CHAOS_FFI_EXPORT char* chaos_subtitle_download_item_json(
    const char* item_json_utf8,
    const char* out_dir_utf8,
    uint32_t timeout_ms,
    uint32_t retries,
    uint8_t overwrite);

// Danmaku - handle-based API
typedef void (*chaos_danmaku_callback)(const char* event_json_utf8, void* user_data);

CHAOS_FFI_EXPORT void* chaos_danmaku_connect(const char* input_utf8);
CHAOS_FFI_EXPORT int32_t chaos_danmaku_set_callback(void* handle, chaos_danmaku_callback cb, void* user_data);
CHAOS_FFI_EXPORT char* chaos_danmaku_poll_json(void* handle, uint32_t max_events);
CHAOS_FFI_EXPORT int32_t chaos_danmaku_disconnect(void* handle);

#ifdef __cplusplus
} // extern "C"
#endif

