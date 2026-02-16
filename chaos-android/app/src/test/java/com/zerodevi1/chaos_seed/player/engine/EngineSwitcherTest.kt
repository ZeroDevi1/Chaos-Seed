package com.zerodevi1.chaos_seed.player.engine

import com.zerodevi1.chaos_seed.player.PlayerEngineType
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test
import android.view.Surface
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

class EngineSwitcherTest {
    private class FakeEngine(private val log: MutableList<String>) : PlayerEngine {
        private val _state = MutableStateFlow(PlayerState())
        override val state: StateFlow<PlayerState> = _state

        override suspend fun open(url: String, headers: Map<String, String>) {
            log += "open:$url:${headers["User-Agent"].orEmpty()}"
        }

        override suspend fun play() {
            log += "play"
        }

        override suspend fun pause() {
            log += "pause"
        }

        override suspend fun setVolume(volume0to100: Int) {
            log += "volume:$volume0to100"
        }

        override fun attachSurface(surface: Surface) {
            log += "attachSurface"
        }

        override fun detachSurface() {
            log += "detachSurface"
        }

        override fun release() {
            log += "release"
        }
    }

    @Test
    fun switchReopensAndRestoresMute() = runTest {
        val log = mutableListOf<String>()
        val old = FakeEngine(log)
        val newEngine = FakeEngine(log)

        val returned = EngineSwitcher.switch(
            current = old,
            create = { _ -> newEngine },
            newType = PlayerEngineType.Mpv,
            surface = null,
            url = "https://example.com/stream.m3u8",
            headers = mapOf("User-Agent" to "ua"),
            muted = true,
        )

        assertEquals(newEngine, returned)
        assertEquals(
            listOf(
                "detachSurface",
                "release",
                "open:https://example.com/stream.m3u8:ua",
                "volume:0",
            ),
            log,
        )
    }
}
