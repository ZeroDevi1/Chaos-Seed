package com.zerodevi1.chaos_seed.core.backend

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
import kotlinx.coroutines.flow.Flow

interface ChaosBackend {
    val name: String

    // Live directory.
    suspend fun categories(site: String): List<LiveDirCategory>
    suspend fun recommendRooms(site: String, page: Int): LiveDirRoomListResult
    suspend fun categoryRooms(site: String, parentId: String?, categoryId: String, page: Int): LiveDirRoomListResult
    suspend fun searchRooms(site: String, keyword: String, page: Int): LiveDirRoomListResult

    // Live playback.
    suspend fun decodeManifest(input: String): LivestreamDecodeManifestResult
    suspend fun resolveVariant2(site: String, roomId: String, variantId: String): LivestreamVariant
    suspend fun openLive(input: String, variantId: String? = null): LiveOpenResult
    suspend fun closeLive(sessionId: String)
    fun danmakuStream(sessionId: String): Flow<DanmakuMessage>

    // Music.
    suspend fun musicConfigSet(cfg: MusicProviderConfig)
    suspend fun searchTracks(p: MusicSearchParams): List<MusicTrack>
    suspend fun qqLoginQrCreate(loginType: String): MusicLoginQr
    suspend fun qqLoginQrPoll(sessionId: String): MusicLoginQrPollResult
    suspend fun downloadStart(p: MusicDownloadStartParams): MusicDownloadStartResult
    suspend fun downloadStatus(sessionId: String): MusicDownloadStatus
    suspend fun downloadCancel(sessionId: String)

    // Lyrics.
    suspend fun lyricsSearch(p: LyricsSearchParams): List<LyricsSearchResult>
}

