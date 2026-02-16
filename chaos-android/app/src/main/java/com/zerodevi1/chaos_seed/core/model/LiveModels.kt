package com.zerodevi1.chaos_seed.core.model

import com.zerodevi1.chaos_seed.core.json.pickArray
import com.zerodevi1.chaos_seed.core.json.pickBool
import com.zerodevi1.chaos_seed.core.json.pickIntOrNull
import com.zerodevi1.chaos_seed.core.json.pickObject
import com.zerodevi1.chaos_seed.core.json.pickString
import com.zerodevi1.chaos_seed.core.json.pickStringOrNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive

data class LivestreamPlaybackHints(
    val referer: String?,
    val userAgent: String?,
) {
    companion object {
        fun fromJson(obj: JsonObject): LivestreamPlaybackHints {
            return LivestreamPlaybackHints(
                referer = obj.pickStringOrNull(listOf("referer")),
                userAgent = obj.pickStringOrNull(listOf("userAgent", "user_agent")),
            )
        }
    }
}

data class LivestreamInfo(
    val title: String,
    val name: String?,
    val avatar: String?,
    val cover: String?,
    val isLiving: Boolean,
) {
    companion object {
        fun fromJson(obj: JsonObject): LivestreamInfo {
            return LivestreamInfo(
                title = obj.pickString(listOf("title")),
                name = obj.pickStringOrNull(listOf("name")),
                avatar = obj.pickStringOrNull(listOf("avatar")),
                cover = obj.pickStringOrNull(listOf("cover")),
                isLiving = obj.pickBool(listOf("isLiving", "is_living"), fallback = false),
            )
        }
    }
}

data class LivestreamVariant(
    val id: String,
    val label: String,
    val quality: Int,
    val rate: Int?,
    val url: String?,
    val backupUrls: List<String>,
) {
    companion object {
        fun fromJson(obj: JsonObject): LivestreamVariant {
            val backups = obj.pickArray(listOf("backupUrls", "backup_urls"))
                ?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull?.trim()?.takeIf { s -> s.isNotBlank() } }
                ?: emptyList()
            return LivestreamVariant(
                id = obj.pickString(listOf("id")),
                label = obj.pickString(listOf("label")),
                quality = obj.pickIntOrNull(listOf("quality")) ?: 0,
                rate = obj.pickIntOrNull(listOf("rate")),
                url = obj.pickStringOrNull(listOf("url")),
                backupUrls = backups,
            )
        }
    }
}

data class LivestreamDecodeManifestResult(
    val site: String,
    val roomId: String,
    val rawInput: String,
    val info: LivestreamInfo,
    val playback: LivestreamPlaybackHints,
    val variants: List<LivestreamVariant>,
) {
    companion object {
        fun fromJson(obj: JsonObject): LivestreamDecodeManifestResult {
            val infoObj = obj.pickObject(listOf("info")) ?: JsonObject(emptyMap())
            val pbObj = obj.pickObject(listOf("playback")) ?: JsonObject(emptyMap())
            val rawVariants = obj.pickArray(listOf("variants")) ?: emptyList()
            val variants = rawVariants.mapNotNull { it as? JsonObject }.map { LivestreamVariant.fromJson(it) }
            return LivestreamDecodeManifestResult(
                site = obj.pickString(listOf("site")),
                roomId = obj.pickString(listOf("roomId", "room_id")),
                rawInput = obj.pickString(listOf("rawInput", "raw_input")),
                info = LivestreamInfo.fromJson(infoObj),
                playback = LivestreamPlaybackHints.fromJson(pbObj),
                variants = variants,
            )
        }
    }
}

data class LiveOpenResult(
    val sessionId: String,
    val site: String,
    val roomId: String,
    val title: String,
    val variantId: String,
    val variantLabel: String,
    val url: String,
    val backupUrls: List<String>,
    val referer: String?,
    val userAgent: String?,
)
