package com.zerodevi1.chaos_seed.player

enum class PlayerEngineType(
    val persistedValue: String,
    val label: String,
) {
    Exo("exo", "EXO"),
    Mpv("mpv", "MPV"),
    ;

    companion object {
        fun fromPersisted(raw: String?): PlayerEngineType? {
            val s = raw?.trim().orEmpty()
            return entries.firstOrNull { it.persistedValue == s }
        }
    }
}

