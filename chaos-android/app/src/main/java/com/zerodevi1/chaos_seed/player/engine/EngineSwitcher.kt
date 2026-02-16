package com.zerodevi1.chaos_seed.player.engine

import android.view.Surface
import com.zerodevi1.chaos_seed.player.PlayerEngineType

/**
 * Pure switching logic (engine-agnostic). Useful for unit tests.
 */
object EngineSwitcher {
    suspend fun switch(
        current: PlayerEngine?,
        create: (PlayerEngineType) -> PlayerEngine,
        newType: PlayerEngineType,
        surface: Surface?,
        url: String?,
        headers: Map<String, String>,
        muted: Boolean,
    ): PlayerEngine {
        current?.detachSurface()
        current?.release()

        val next = create(newType)
        surface?.let { next.attachSurface(it) }
        if (!url.isNullOrBlank()) {
            next.open(url, headers)
            next.setVolume(if (muted) 0 else 100)
        }
        return next
    }
}

