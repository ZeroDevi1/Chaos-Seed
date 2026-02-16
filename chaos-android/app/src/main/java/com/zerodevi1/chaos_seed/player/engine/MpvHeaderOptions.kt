package com.zerodevi1.chaos_seed.player.engine

data class MpvHeaderOptions(
    val userAgent: String?,
    val referer: String?,
    val httpHeaderFields: String?,
)

object MpvHeaderOptionsBuilder {
    /**
     * Build mpv http header options from an HTTP header map.
     *
     * Strategy:
     * - User-Agent -> mpv option "user-agent"
     * - Referer -> mpv option "referrer"
     * - Other headers -> mpv option "http-header-fields" with '\n' separated "Key: Value"
     */
    fun fromHeaders(headers: Map<String, String>): MpvHeaderOptions {
        var ua: String? = null
        var ref: String? = null
        val others = linkedMapOf<String, String>()

        headers.forEach { (kRaw, vRaw) ->
            val k = kRaw.trim()
            val v = vRaw.trim()
            if (k.isEmpty() || v.isEmpty()) return@forEach
            when (k.lowercase()) {
                "user-agent" -> ua = v
                "referer", "referrer" -> ref = v
                else -> others[k] = v
            }
        }

        val fields = if (others.isEmpty()) null else {
            // mpv expects raw header lines; keep it simple and deterministic for tests.
            others.entries.joinToString(separator = "\n") { (k, v) -> "$k: $v" }
        }
        return MpvHeaderOptions(userAgent = ua, referer = ref, httpHeaderFields = fields)
    }
}

