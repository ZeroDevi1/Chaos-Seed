package com.zerodevi1.chaos_seed.player.engine

import android.content.Context
import com.zerodevi1.chaos_seed.player.PlayerEngineType

class PlayerEngineFactory(private val appContext: Context) {
    fun create(type: PlayerEngineType): PlayerEngine {
        return when (type) {
            PlayerEngineType.Exo -> Media3ExoEngine(appContext)
            PlayerEngineType.Mpv -> MpvEngine(appContext)
        }
    }
}

