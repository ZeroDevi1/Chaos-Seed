#ifndef CHAOS_FFI_H
#define CHAOS_FFI_H

#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef void (*ChaosDanmakuCallback)(const char *event_json_utf8, void *user_data);

uint32_t chaos_ffi_api_version(void);

char *chaos_ffi_version_json(void);

char *chaos_ffi_last_error_json(void);

void chaos_ffi_string_free(char *s);

char *chaos_subtitle_search_json(const char *query_utf8,
                                 uint32_t limit,
                                 double min_score_or_neg1,
                                 const char *lang_utf8_or_null,
                                 uint32_t timeout_ms);

char *chaos_subtitle_download_item_json(const char *item_json_utf8,
                                        const char *out_dir_utf8,
                                        uint32_t timeout_ms,
                                        uint32_t retries,
                                        uint8_t overwrite);

char *chaos_livestream_decode_manifest_json(const char *input_utf8,
                                            uint8_t drop_inaccessible_high_qualities);

char *chaos_livestream_resolve_variant_json(const char *input_utf8, const char *variant_id_utf8);

void *chaos_danmaku_connect(const char *input_utf8);

int32_t chaos_danmaku_set_callback(void *handle, ChaosDanmakuCallback cb, void *user_data);

char *chaos_danmaku_poll_json(void *handle, uint32_t max_events);

int32_t chaos_danmaku_disconnect(void *handle);

#endif  /* CHAOS_FFI_H */
