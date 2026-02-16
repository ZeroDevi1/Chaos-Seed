package com.zerodevi1.chaos_seed.player.engine

import android.content.Context
import android.view.Surface
import dev.jdtech.mpv.MPVLib
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext

class MpvEngine(appContext: Context) : PlayerEngine {
    private val appCtx = appContext.applicationContext
    private val _state = MutableStateFlow(PlayerState())
    override val state: StateFlow<PlayerState> = _state

    private var surfaceAttached = false

    private val observer = object : MPVLib.EventObserver {
        override fun eventProperty(name: String) {
            // no-op for "unset"
        }

        override fun eventProperty(name: String, value: Long) {
            when (name) {
                "video-params/w" -> _state.value = _state.value.copy(videoWidth = value.toInt())
                "video-params/h" -> _state.value = _state.value.copy(videoHeight = value.toInt())
            }
        }

        override fun eventProperty(name: String, value: Double) {
            // ignore
        }

        override fun eventProperty(name: String, value: Boolean) {
            when (name) {
                "pause" -> _state.value = _state.value.copy(playing = !value, error = null)
                "paused-for-cache" -> _state.value = _state.value.copy(buffering = value)
            }
        }

        override fun eventProperty(name: String, value: String) {
            // ignore
        }

        override fun event(eventId: Int) {
            if (eventId == MPVLib.MPV_EVENT_VIDEO_RECONFIG) {
                // Best-effort refresh.
                val w = MPVLib.getPropertyInt("video-params/w") ?: 0
                val h = MPVLib.getPropertyInt("video-params/h") ?: 0
                _state.value = _state.value.copy(videoWidth = w, videoHeight = h)
            }
        }
    }

    init {
        MpvGlobal.acquireOrThrow()
        // MPV init must happen exactly once for this engine instance.
        MPVLib.create(appCtx)
        MPVLib.init()
        MPVLib.addObserver(observer)
        MPVLib.observeProperty("pause", MPVLib.MPV_FORMAT_FLAG)
        MPVLib.observeProperty("paused-for-cache", MPVLib.MPV_FORMAT_FLAG)
        MPVLib.observeProperty("video-params/w", MPVLib.MPV_FORMAT_INT64)
        MPVLib.observeProperty("video-params/h", MPVLib.MPV_FORMAT_INT64)
    }

    override suspend fun open(url: String, headers: Map<String, String>) {
        withContext(Dispatchers.Main.immediate) {
            val opts = MpvHeaderOptionsBuilder.fromHeaders(headers)
            if (!opts.userAgent.isNullOrBlank()) {
                MPVLib.setOptionString("user-agent", opts.userAgent)
            }
            if (!opts.referer.isNullOrBlank()) {
                MPVLib.setOptionString("referrer", opts.referer)
            }
            if (!opts.httpHeaderFields.isNullOrBlank()) {
                MPVLib.setOptionString("http-header-fields", opts.httpHeaderFields)
            }
            // "replace" so switching lines/engine reuses the same instance.
            MPVLib.command(arrayOf("loadfile", url, "replace"))
        }
    }

    override suspend fun play() {
        withContext(Dispatchers.Main.immediate) {
            MPVLib.setPropertyBoolean("pause", false)
        }
    }

    override suspend fun pause() {
        withContext(Dispatchers.Main.immediate) {
            MPVLib.setPropertyBoolean("pause", true)
        }
    }

    override suspend fun setVolume(volume0to100: Int) {
        withContext(Dispatchers.Main.immediate) {
            MPVLib.setPropertyInt("volume", volume0to100.coerceIn(0, 100))
        }
    }

    override fun attachSurface(surface: Surface) {
        MPVLib.attachSurface(surface)
        surfaceAttached = true
    }

    override fun detachSurface() {
        if (surfaceAttached) {
            MPVLib.detachSurface()
            surfaceAttached = false
        }
    }

    override fun release() {
        runCatching { detachSurface() }
        runCatching { MPVLib.removeObserver(observer) }
        runCatching { MPVLib.destroy() }
        MpvGlobal.release()
    }
}

