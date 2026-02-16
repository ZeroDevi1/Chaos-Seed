package com.zerodevi1.chaos_seed.player.engine

import android.view.Surface
import kotlinx.coroutines.flow.StateFlow

data class PlayerState(
    val playing: Boolean = false,
    val buffering: Boolean = false,
    val videoWidth: Int = 0,
    val videoHeight: Int = 0,
    val error: String? = null,
)

interface PlayerEngine {
    val state: StateFlow<PlayerState>

    suspend fun open(url: String, headers: Map<String, String>)
    suspend fun play()
    suspend fun pause()
    suspend fun setVolume(volume0to100: Int)

    fun attachSurface(surface: Surface)
    fun detachSurface()

    fun release()
}

