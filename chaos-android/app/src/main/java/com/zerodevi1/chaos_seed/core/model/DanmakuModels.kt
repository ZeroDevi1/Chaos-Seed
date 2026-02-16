package com.zerodevi1.chaos_seed.core.model

import com.zerodevi1.chaos_seed.core.json.pickArray
import com.zerodevi1.chaos_seed.core.json.pickIntOrNull
import com.zerodevi1.chaos_seed.core.json.pickLongOrNull
import com.zerodevi1.chaos_seed.core.json.pickObject
import com.zerodevi1.chaos_seed.core.json.pickString
import com.zerodevi1.chaos_seed.core.json.pickStringOrNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject

data class DanmakuMessage(
    val sessionId: String,
    val receivedAtMs: Long,
    val user: String,
    val text: String,
    val imageUrl: String?,
    val imageWidth: Int?,
) {
    companion object {
        /**
         * Mirrors Flutter's DanmakuMessage.fromFfiEventJson logic.
         *
         * FFI emits chaos-core DanmakuEvent (snake_case). The "dms" array may carry image/text parts.
         */
        fun fromFfiEventJson(sessionId: String, obj: JsonObject): DanmakuMessage {
            var imageUrl: String? = null
            var imageWidth: Int? = null

            val dms = obj.pickArray(listOf("dms"))
            val dm0 = dms?.firstOrNull()?.let { (it as? JsonObject) }
            if (dm0 != null) {
                imageUrl = dm0.pickStringOrNull(listOf("image_url"))
                imageWidth = dm0.pickIntOrNull(listOf("image_width"))
            }

            var text = obj.pickString(listOf("text"), fallback = "")
            if (text.isBlank() && dms != null) {
                val parts = mutableListOf<String>()
                for (it in dms) {
                    val m = it as? JsonObject ?: continue
                    val s = m.pickStringOrNull(listOf("text"))
                    if (!s.isNullOrBlank()) parts += s.trim()
                }
                if (parts.isNotEmpty()) text = parts.joinToString(separator = "")
            }

            val received = obj.pickLongOrNull(listOf("received_at_ms", "receivedAtMs")) ?: 0L
            return DanmakuMessage(
                sessionId = sessionId,
                receivedAtMs = received,
                user = obj.pickString(listOf("user"), fallback = ""),
                text = text,
                imageUrl = imageUrl,
                imageWidth = imageWidth,
            )
        }
    }
}

