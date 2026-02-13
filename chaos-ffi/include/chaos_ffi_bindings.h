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

char *chaos_now_playing_snapshot_json(uint8_t include_thumbnail,
                                      uint32_t max_thumbnail_bytes,
                                      uint32_t max_sessions);

char *chaos_subtitle_search_json(const char *query_utf8,
                                 uint32_t limit,
                                 double min_score_or_neg1,
                                 const char *lang_utf8_or_null,
                                 uint32_t timeout_ms);

char *chaos_lyrics_search_json(const char *title_utf8,
                               const char *album_utf8_or_null,
                               const char *artist_utf8_or_null,
                               uint32_t duration_ms_or_0,
                               uint32_t limit,
                               uint8_t strict_match,
                               const char *services_csv_utf8_or_null,
                               uint32_t timeout_ms);

char *chaos_subtitle_download_item_json(const char *item_json_utf8,
                                        const char *out_dir_utf8,
                                        uint32_t timeout_ms,
                                        uint32_t retries,
                                        uint8_t overwrite);

char *chaos_livestream_decode_manifest_json(const char *input_utf8,
                                            uint8_t drop_inaccessible_high_qualities);

char *chaos_live_dir_categories_json(const char *site_utf8);

char *chaos_live_dir_recommend_rooms_json(const char *site_utf8, uint32_t page);

char *chaos_live_dir_category_rooms_json(const char *site_utf8,
                                         const char *parent_id_utf8_or_null,
                                         const char *category_id_utf8,
                                         uint32_t page);

char *chaos_live_dir_search_rooms_json(const char *site_utf8,
                                       const char *keyword_utf8,
                                       uint32_t page);

char *chaos_livestream_resolve_variant_json(const char *input_utf8, const char *variant_id_utf8);

/**
 * Resolve a stream variant using explicit `(site, room_id, variant_id)`.
 *
 * Prefer this over `chaos_livestream_resolve_variant_json(input, variant_id)` when you already
 * have the canonical room id from `LiveManifest.room_id`.
 */
char *chaos_livestream_resolve_variant2_json(const char *site_utf8,
                                             const char *room_id_utf8,
                                             const char *variant_id_utf8);

void *chaos_danmaku_connect(const char *input_utf8);

int32_t chaos_danmaku_set_callback(void *handle, ChaosDanmakuCallback cb, void *user_data);

char *chaos_danmaku_poll_json(void *handle, uint32_t max_events);

int32_t chaos_danmaku_disconnect(void *handle);

#endif  /* CHAOS_FFI_H */
