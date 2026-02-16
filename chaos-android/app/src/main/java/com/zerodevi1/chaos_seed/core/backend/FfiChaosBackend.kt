package com.zerodevi1.chaos_seed.core.backend

import android.content.Context
import com.zerodevi1.chaos_seed.core.ffi.ChaosFfi
import com.zerodevi1.chaos_seed.core.ffi.FfiDispatcher
import com.zerodevi1.chaos_seed.core.ffi.FfiStrings
import com.zerodevi1.chaos_seed.core.model.DanmakuMessage
import com.zerodevi1.chaos_seed.core.model.LiveDirCategory
import com.zerodevi1.chaos_seed.core.model.LiveDirRoomListResult
import com.zerodevi1.chaos_seed.core.model.LivestreamDecodeManifestResult
import com.zerodevi1.chaos_seed.core.model.LiveOpenResult
import com.zerodevi1.chaos_seed.core.model.LivestreamVariant
import com.zerodevi1.chaos_seed.core.model.LyricsSearchParams
import com.zerodevi1.chaos_seed.core.model.LyricsSearchResult
import com.zerodevi1.chaos_seed.core.model.MusicDownloadStartParams
import com.zerodevi1.chaos_seed.core.model.MusicDownloadStartResult
import com.zerodevi1.chaos_seed.core.model.MusicDownloadStatus
import com.zerodevi1.chaos_seed.core.model.MusicLoginQr
import com.zerodevi1.chaos_seed.core.model.MusicLoginQrPollResult
import com.zerodevi1.chaos_seed.core.model.MusicProviderConfig
import com.zerodevi1.chaos_seed.core.model.MusicSearchParams
import com.zerodevi1.chaos_seed.core.model.MusicTrack
import com.sun.jna.Pointer
import kotlinx.coroutines.delay
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject

class FfiChaosBackend(
    private val appContext: Context,
) : ChaosBackend {
    override val name: String = "FFI"

    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    // Decode manifest cache (mirrors Flutter).
    @Volatile
    private var lastManifest: LivestreamDecodeManifestResult? = null

    // Danmaku handle map.
    private val dmHandles = mutableMapOf<String, Pointer>()

    override suspend fun categories(site: String): List<LiveDirCategory> {
        val s = callString { ChaosFfi.api().chaos_live_dir_categories_json(site) }
        return json.decodeFromString(s)
    }

    override suspend fun recommendRooms(site: String, page: Int): LiveDirRoomListResult {
        val s = callString { ChaosFfi.api().chaos_live_dir_recommend_rooms_json(site, page) }
        return json.decodeFromString(s)
    }

    override suspend fun categoryRooms(
        site: String,
        parentId: String?,
        categoryId: String,
        page: Int,
    ): LiveDirRoomListResult {
        val s = callString {
            ChaosFfi.api().chaos_live_dir_category_rooms_json(site, parentId, categoryId, page)
        }
        return json.decodeFromString(s)
    }

    override suspend fun searchRooms(site: String, keyword: String, page: Int): LiveDirRoomListResult {
        val s = callString { ChaosFfi.api().chaos_live_dir_search_rooms_json(site, keyword, page) }
        return json.decodeFromString(s)
    }

    override suspend fun decodeManifest(input: String): LivestreamDecodeManifestResult {
        val s = callString { ChaosFfi.api().chaos_livestream_decode_manifest_json(input, 0) }
        val obj = json.parseToJsonElement(s).jsonObject
        val man = LivestreamDecodeManifestResult.fromJson(obj)
        lastManifest = man
        return man
    }

    override suspend fun resolveVariant2(site: String, roomId: String, variantId: String): LivestreamVariant {
        val s = callString { ChaosFfi.api().chaos_livestream_resolve_variant2_json(site, roomId, variantId) }
        val obj = json.parseToJsonElement(s).jsonObject
        return LivestreamVariant.fromJson(obj)
    }

    override suspend fun openLive(input: String, variantId: String?): LiveOpenResult {
        val inTrim = input.trim()
        require(inTrim.isNotEmpty()) { "input is empty" }

        var man = lastManifest
        val manTrim = man?.rawInput?.trim().orEmpty()
        if (man == null || manTrim.isEmpty() || manTrim != inTrim || man.roomId.trim().isEmpty()) {
            man = decodeManifest(inTrim)
        }

        val picked = pickVariant(man, variantId)
        val finalV = if (hasUrl(picked)) {
            picked
        } else {
            val rid = man.roomId.trim()
            require(rid.isNotEmpty()) { "roomId is empty; please decode again" }
            resolveVariant2(man.site, rid, picked.id)
        }

        val url = finalV.url?.trim().orEmpty()
        val backups = finalV.backupUrls.map { it.trim() }.filter { it.isNotEmpty() }
        require(url.isNotEmpty() || backups.isNotEmpty()) { "empty url" }

        val sessionId = "ffi-" + System.nanoTime()
        val handle = callPointer { ChaosFfi.api().chaos_danmaku_connect(inTrim) }
        synchronized(dmHandles) { dmHandles[sessionId] = handle }

        return LiveOpenResult(
            sessionId = sessionId,
            site = man.site,
            roomId = man.roomId,
            title = man.info.title,
            variantId = finalV.id.ifBlank { picked.id },
            variantLabel = if (finalV.label.isNotBlank()) finalV.label else picked.label,
            url = if (url.isNotEmpty()) url else backups.first(),
            backupUrls = backups,
            referer = man.playback.referer,
            userAgent = man.playback.userAgent,
        )
    }

    override suspend fun closeLive(sessionId: String) {
        val h = synchronized(dmHandles) { dmHandles.remove(sessionId) } ?: return
        withContext(FfiDispatcher.dispatcher) {
            runCatching { ChaosFfi.api().chaos_danmaku_disconnect(h) }
        }
    }

    override fun danmakuStream(sessionId: String): Flow<DanmakuMessage> = callbackFlow {
        val handle = synchronized(dmHandles) { dmHandles[sessionId] }
        if (handle == null) {
            close()
            return@callbackFlow
        }

        val job = launch(FfiDispatcher.dispatcher) {
            while (isActive) {
                val eventsJson = runCatching {
                    val ptr = ChaosFfi.api().chaos_danmaku_poll_json(handle, 50)
                    FfiStrings.takeUtf8(ptr)
                }.getOrElse {
                    // Poll errors are best-effort; keep going.
                    delay(250)
                    continue
                }

                if (eventsJson != "[]") {
                    runCatching {
                        val el = json.parseToJsonElement(eventsJson)
                        val arr = el as? JsonArray ?: return@runCatching
                        for (ev in arr) {
                            val obj = ev as? JsonObject ?: continue
                            trySend(DanmakuMessage.fromFfiEventJson(sessionId, obj))
                        }
                    }
                }
                delay(180)
            }
        }

        awaitClose { job.cancel() }
    }

    override suspend fun musicConfigSet(cfg: MusicProviderConfig) {
        val raw = json.encodeToString(MusicProviderConfig.serializer(), cfg)
        callString { ChaosFfi.api().chaos_music_config_set_json(raw) }
    }

    override suspend fun searchTracks(p: MusicSearchParams): List<MusicTrack> {
        val raw = json.encodeToString(MusicSearchParams.serializer(), p)
        val s = callString { ChaosFfi.api().chaos_music_search_tracks_json(raw) }
        return json.decodeFromString(s)
    }

    override suspend fun qqLoginQrCreate(loginType: String): MusicLoginQr {
        val s = callString { ChaosFfi.api().chaos_music_qq_login_qr_create_json(loginType) }
        return json.decodeFromString(s)
    }

    override suspend fun qqLoginQrPoll(sessionId: String): MusicLoginQrPollResult {
        val s = callString { ChaosFfi.api().chaos_music_qq_login_qr_poll_json(sessionId) }
        return json.decodeFromString(s)
    }

    override suspend fun downloadStart(p: MusicDownloadStartParams): MusicDownloadStartResult {
        val raw = json.encodeToString(MusicDownloadStartParams.serializer(), p)
        val s = callString { ChaosFfi.api().chaos_music_download_start_json(raw) }
        return json.decodeFromString(s)
    }

    override suspend fun downloadStatus(sessionId: String): MusicDownloadStatus {
        val s = callString { ChaosFfi.api().chaos_music_download_status_json(sessionId) }
        return json.decodeFromString(s)
    }

    override suspend fun downloadCancel(sessionId: String) {
        callString { ChaosFfi.api().chaos_music_download_cancel_json(sessionId) }
    }

    override suspend fun lyricsSearch(p: LyricsSearchParams): List<LyricsSearchResult> {
        val servicesCsv = p.services.joinToString(separator = ",").ifBlank { null }
        val s = callString {
            ChaosFfi.api().chaos_lyrics_search_json(
                p.title,
                p.album,
                p.artist,
                (p.durationMs ?: 0L).toInt(),
                p.limit,
                if (p.strictMatch) 1.toByte() else 0.toByte(),
                servicesCsv,
                p.timeoutMs.toInt(),
            )
        }
        return json.decodeFromString(s)
    }

    // ---- helpers ----

    private suspend fun callString(block: () -> Pointer?): String {
        return withContext(FfiDispatcher.dispatcher) {
            FfiStrings.takeUtf8(block())
        }
    }

    private suspend fun callPointer(block: () -> Pointer?): Pointer {
        return withContext(FfiDispatcher.dispatcher) {
            block() ?: throw IllegalStateException("FFI returned null pointer")
        }
    }

    private fun hasUrl(v: LivestreamVariant): Boolean {
        return (!v.url.isNullOrBlank()) || v.backupUrls.any { it.isNotBlank() }
    }

    private fun pickVariant(man: LivestreamDecodeManifestResult, requestedId: String?): LivestreamVariant {
        val rid = requestedId?.trim().orEmpty()
        if (rid.isNotEmpty()) {
            return man.variants.firstOrNull { it.id == rid }
                ?: throw IllegalStateException("variant not found: $rid")
        }

        val sorted = man.variants.sortedByDescending { it.quality }
        for (v in sorted) {
            if (hasUrl(v)) return v
        }
        return sorted.firstOrNull() ?: throw IllegalStateException("no variants")
    }
}
