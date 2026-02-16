package com.zerodevi1.chaos_seed.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
enum class MusicService {
    @SerialName("qq") Qq,
    @SerialName("kugou") Kugou,
    @SerialName("netease") Netease,
    @SerialName("kuwo") Kuwo,
}

@Serializable
data class MusicQuality(
    val id: String,
    val label: String,
    val format: String,
    val bitrateKbps: Int? = null,
    val lossless: Boolean,
)

@Serializable
data class MusicTrack(
    val service: MusicService,
    val id: String,
    val title: String,
    val artists: List<String> = emptyList(),
    val artistIds: List<String> = emptyList(),
    val album: String? = null,
    val albumId: String? = null,
    val durationMs: Long? = null,
    val coverUrl: String? = null,
    val qualities: List<MusicQuality> = emptyList(),
)

@Serializable
data class MusicProviderConfig(
    val kugouBaseUrl: String? = null,
    val neteaseBaseUrls: List<String> = emptyList(),
    val neteaseAnonymousCookieUrl: String? = null,
)

@Serializable
data class MusicSearchParams(
    val service: MusicService,
    val keyword: String,
    val page: Int = 1,
    val pageSize: Int = 20,
)

@Serializable
data class MusicLoginQr(
    val sessionId: String,
    val loginType: String,
    val mime: String,
    val base64: String,
    val identifier: String,
    val createdAtUnixMs: Long,
)

@Serializable
data class QqMusicCookie(
    val openid: String? = null,
    val refreshToken: String? = null,
    val accessToken: String? = null,
    val expiredAt: Long? = null,
    val musicid: String? = null,
    val musickey: String? = null,
    val uid: String? = null,
    val token: String? = null,
    val rawCookie: String? = null,
)

@Serializable
data class MusicLoginQrPollResult(
    val sessionId: String,
    val state: String,
    val message: String? = null,
    val cookie: QqMusicCookie? = null,
)

@Serializable
data class MusicAuthState(
    val qq: QqMusicCookie? = null,
    // Some providers may return/require provider-specific auth blobs; keep it flexible.
    // We don't currently use this in the Android port, but it must remain serializable.
    val kugou: JsonElement? = null,
    val neteaseCookie: String? = null,
)

@Serializable
@Suppress("unused")
sealed class MusicDownloadTarget {
    @Serializable
    @SerialName("track")
    data class Track(val track: MusicTrack) : MusicDownloadTarget()

    @Serializable
    @SerialName("album")
    data class Album(val service: MusicService, val albumId: String) : MusicDownloadTarget()

    @Serializable
    @SerialName("artist_all")
    data class ArtistAll(val service: MusicService, val artistId: String) : MusicDownloadTarget()
}

@Serializable
data class MusicDownloadOptions(
    val qualityId: String,
    val outDir: String,
    val pathTemplate: String? = null,
    val overwrite: Boolean = false,
    val concurrency: Int = 3,
    val retries: Int = 2,
)

@Serializable
data class MusicDownloadStartParams(
    val config: MusicProviderConfig,
    val auth: MusicAuthState = MusicAuthState(),
    val target: MusicDownloadTarget,
    val options: MusicDownloadOptions,
)

@Serializable
data class MusicDownloadStartResult(
    val sessionId: String,
)

@Serializable
enum class MusicJobState {
    @SerialName("pending") Pending,
    @SerialName("running") Running,
    @SerialName("done") Done,
    @SerialName("failed") Failed,
    @SerialName("skipped") Skipped,
    @SerialName("canceled") Canceled,
}

@Serializable
data class MusicDownloadTotals(
    val total: Int,
    val done: Int,
    val failed: Int,
    val skipped: Int,
    val canceled: Int,
)

@Serializable
data class MusicDownloadJobResult(
    val index: Int,
    val trackId: String? = null,
    val state: MusicJobState,
    val path: String? = null,
    val bytes: Long? = null,
    val error: String? = null,
)

@Serializable
data class MusicDownloadStatus(
    val done: Boolean,
    val totals: MusicDownloadTotals,
    val jobs: List<MusicDownloadJobResult> = emptyList(),
)
