package com.zerodevi1.chaos_seed.core.model

import kotlinx.serialization.Serializable

data class LyricsSearchParams(
    val title: String,
    val album: String? = null,
    val artist: String? = null,
    val durationMs: Long? = null,
    val limit: Int = 5,
    val strictMatch: Boolean = false,
    val services: List<String> = listOf("qq", "netease", "lrclib"),
    val timeoutMs: Long = 8_000,
)

@Serializable
data class LyricsSearchResult(
    val service: String,
    val serviceToken: String,
    val title: String? = null,
    val artist: String? = null,
    val album: String? = null,
    val durationMs: Long? = null,
    val matchPercentage: Int,
    val quality: Double,
    val matched: Boolean,
    val hasTranslation: Boolean,
    val hasInlineTimetags: Boolean,
    val lyricsOriginal: String,
    val lyricsTranslation: String? = null,
)

