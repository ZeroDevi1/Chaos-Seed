package com.zerodevi1.chaos_seed.core.ffi

import com.sun.jna.Library
import com.sun.jna.Pointer

/**
 * JNA bindings for chaos-ffi C ABI.
 *
 * IMPORTANT:
 * - All returned `char*` are UTF-8 and must be freed by `chaos_ffi_string_free`.
 * - Do NOT load this interface eagerly at class init; use [ChaosFfi.api] for lazy loading.
 */
interface ChaosFfiApi : Library {
    fun chaos_ffi_last_error_json(): Pointer?
    fun chaos_ffi_string_free(s: Pointer?)

    // Live directory.
    fun chaos_live_dir_categories_json(site_utf8: String): Pointer?
    fun chaos_live_dir_recommend_rooms_json(site_utf8: String, page: Int): Pointer?
    fun chaos_live_dir_category_rooms_json(
        site_utf8: String,
        parent_id_utf8_or_null: String?,
        category_id_utf8: String,
        page: Int,
    ): Pointer?

    fun chaos_live_dir_search_rooms_json(site_utf8: String, keyword_utf8: String, page: Int): Pointer?

    // Live playback.
    fun chaos_livestream_decode_manifest_json(input_utf8: String, drop_inaccessible_high_qualities: Byte): Pointer?
    fun chaos_livestream_resolve_variant2_json(site_utf8: String, room_id_utf8: String, variant_id_utf8: String): Pointer?

    // Danmaku.
    fun chaos_danmaku_connect(input_utf8: String): Pointer?
    fun chaos_danmaku_poll_json(handle: Pointer?, max_events: Int): Pointer?
    fun chaos_danmaku_disconnect(handle: Pointer?): Int

    // Music.
    fun chaos_music_config_set_json(config_json_utf8: String): Pointer?
    fun chaos_music_search_tracks_json(params_json_utf8: String): Pointer?
    fun chaos_music_qq_login_qr_create_json(login_type_utf8: String): Pointer?
    fun chaos_music_qq_login_qr_poll_json(session_id_utf8: String): Pointer?
    fun chaos_music_download_start_json(start_params_json_utf8: String): Pointer?
    fun chaos_music_download_status_json(session_id_utf8: String): Pointer?
    fun chaos_music_download_cancel_json(session_id_utf8: String): Pointer?

    // Lyrics.
    fun chaos_lyrics_search_json(
        title_utf8: String,
        album_utf8_or_null: String?,
        artist_utf8_or_null: String?,
        duration_ms_or_0: Int,
        limit: Int,
        strict_match: Byte,
        services_csv_utf8_or_null: String?,
        timeout_ms: Int,
    ): Pointer?
}
