package com.zerodevi1.chaos_seed.player.engine

import android.content.Context
import android.view.Surface
import dev.jdtech.mpv.MPVLib
import android.os.SystemClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MpvEngine(appContext: Context) : PlayerEngine {
    private val appCtx = appContext.applicationContext
    private val _state = MutableStateFlow(PlayerState())
    override val state: StateFlow<PlayerState> = _state

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var postAttachJob: Job? = null

    private var surfaceAttached = false
    private var currentSurface: Surface? = null
    private var pendingUrl: String? = null
    private var pendingHeaders: Map<String, String> = emptyMap()
    private var loadedUrl: String? = null
    private var loadedHeaders: Map<String, String> = emptyMap()
    @Volatile
    private var lastFailureReason: String? = null
    @Volatile
    private var videoPipelineFailed = false
    @Volatile
    private var suppressEndFileUntilElapsedMs: Long = 0L

    @Volatile
    private var lastVideoReconfigAtMs: Long = 0L
    @Volatile
    private var lastPlaybackRestartAtMs: Long = 0L
    @Volatile
    private var lastFirstFrameAtMs: Long = 0L

    @Volatile
    private var baselineVideoReconfigAtMs: Long = 0L
    @Volatile
    private var baselinePlaybackRestartAtMs: Long = 0L
    @Volatile
    private var baselineFirstFrameAtMs: Long = 0L

    @Volatile
    private var awaitingHealthyAfterAttach: Boolean = false

    private fun markHealthyAfterAttach() {
        if (!awaitingHealthyAfterAttach) return
        awaitingHealthyAfterAttach = false
        postAttachJob?.cancel()
        postAttachJob = null
    }

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
            when (eventId) {
                MPVLib.MPV_EVENT_VIDEO_RECONFIG -> {
                    lastVideoReconfigAtMs = SystemClock.elapsedRealtime()
                    markHealthyAfterAttach()
                    // Best-effort refresh.
                    val w = MPVLib.getPropertyInt("video-params/w") ?: 0
                    val h = MPVLib.getPropertyInt("video-params/h") ?: 0
                    _state.value = _state.value.copy(videoWidth = w, videoHeight = h)
                }
                MPVLib.MPV_EVENT_START_FILE -> {
                    lastFailureReason = null
                    videoPipelineFailed = false
                    _state.value = _state.value.copy(buffering = true, playing = false, error = null)
                }
                MPVLib.MPV_EVENT_FILE_LOADED -> {
                    _state.value = _state.value.copy(buffering = false, error = null)
                }
                MPVLib.MPV_EVENT_PLAYBACK_RESTART -> {
                    lastPlaybackRestartAtMs = SystemClock.elapsedRealtime()
                    markHealthyAfterAttach()
                    _state.value = _state.value.copy(buffering = false, error = null)
                }
                MPVLib.MPV_EVENT_END_FILE -> {
                    val now = SystemClock.elapsedRealtime()
                    if (now <= suppressEndFileUntilElapsedMs) {
                        _state.value = _state.value.copy(
                            playing = false,
                            buffering = true,
                            error = null,
                        )
                        return
                    }
                    val normalizedError = lastFailureReason?.trim().takeUnless { it.isNullOrEmpty() }
                        ?: "播放结束或打开失败"
                    _state.value = _state.value.copy(
                        playing = false,
                        buffering = false,
                        error = normalizedError,
                    )
                }
            }
        }
    }

    private val logObserver = MPVLib.LogObserver { prefix, level, text ->
        val msg = text.trim()
        if (msg.isEmpty()) return@LogObserver

        fun markVideoPipelineFailure(reason: String) {
            lastFailureReason = reason
            if (videoPipelineFailed) return
            videoPipelineFailed = true
            _state.value = _state.value.copy(
                playing = false,
                buffering = false,
                error = reason,
            )
        }

        when {
            msg.contains("first video frame", ignoreCase = true) -> {
                lastFirstFrameAtMs = SystemClock.elapsedRealtime()
                markHealthyAfterAttach()
            }
            msg.contains("HTTP error", ignoreCase = true) ->
                lastFailureReason = msg
            msg.contains("Could not initialize video chain", ignoreCase = true) ->
                markVideoPipelineFailure("视频链初始化失败")
            msg.contains("Cannot convert decoder/filter output", ignoreCase = true) ->
                markVideoPipelineFailure("视频格式转换失败")
            msg.contains("hardware format not supported", ignoreCase = true) ->
                markVideoPipelineFailure("渲染器不支持当前视频格式")
            level <= MPVLib.MPV_LOG_LEVEL_ERROR ->
                lastFailureReason = msg
        }
    }

    init {
        MpvGlobal.acquireOrThrow()
        // MPV init must happen exactly once for this engine instance.
        MPVLib.create(appCtx)
        // Android app 内不提供 yt-dlp/youtube-dl，关闭 ytdl hook 避免无意义子进程尝试。
        MPVLib.setOptionString("ytdl", "no")
        // Use a general GPU video output on Android.
        // Default auto-probe may pick mediacodec_embed which can fail with software decode fallback.
        MPVLib.setOptionString("vo", "gpu")
        MPVLib.setOptionString("gpu-context", "android")
        // Prefer software decode path for compatibility (especially emulator/x86 OpenGL stacks).
        MPVLib.setOptionString("hwdec", "no")
        MPVLib.init()
        MPVLib.addObserver(observer)
        MPVLib.addLogObserver(logObserver)
        MPVLib.observeProperty("pause", MPVLib.MPV_FORMAT_FLAG)
        MPVLib.observeProperty("paused-for-cache", MPVLib.MPV_FORMAT_FLAG)
        MPVLib.observeProperty("video-params/w", MPVLib.MPV_FORMAT_INT64)
        MPVLib.observeProperty("video-params/h", MPVLib.MPV_FORMAT_INT64)
    }

    override suspend fun open(url: String, headers: Map<String, String>) {
        pendingUrl = url
        pendingHeaders = headers
        if (!surfaceAttached) {
            // mpv may start with no video output if we load before a Surface exists.
            // Defer actual "loadfile" until attachSurface().
            return
        }
        withContext(Dispatchers.Main.immediate) {
            loadFile(url = url, headers = headers)
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
        if (!surface.isValid) {
            detachSurface()
            return
        }
        // SurfaceView can keep the same Surface *object* while swapping the underlying BufferQueue
        // (common during rotation / relayout). Always re-attach when we're told the surface is ready.
        if (surfaceAttached) detachSurface()
        MPVLib.attachSurface(surface)
        surfaceAttached = true
        currentSurface = surface

        // Surface callbacks are on the main thread; (re)load only when we never loaded, or when
        // URL/headers changed (e.g. line/variant switch).
        val url = pendingUrl
        if (!url.isNullOrBlank() && (loadedUrl != url || loadedHeaders != pendingHeaders)) {
            runCatching { loadFile(url = url, headers = pendingHeaders) }
        }

        postAttachJob?.cancel()
        postAttachJob = null
        if (awaitingHealthyAfterAttach) {
            val baseVr = baselineVideoReconfigAtMs
            val basePr = baselinePlaybackRestartAtMs
            val baseFf = baselineFirstFrameAtMs

            postAttachJob =
                scope.launch {
                    delay(800L)
                    val shouldReload =
                        awaitingHealthyAfterAttach &&
                            lastVideoReconfigAtMs == baseVr &&
                            lastPlaybackRestartAtMs == basePr &&
                            lastFirstFrameAtMs == baseFf
                    if (!shouldReload) return@launch

                    val pending = pendingUrl
                    if (!pending.isNullOrBlank()) {
                        runCatching { loadFile(url = pending, headers = pendingHeaders) }
                    }
                    awaitingHealthyAfterAttach = false
                }
        }
    }

    override fun detachSurface() {
        if (surfaceAttached) {
            postAttachJob?.cancel()
            postAttachJob = null
            awaitingHealthyAfterAttach = true
            baselineVideoReconfigAtMs = lastVideoReconfigAtMs
            baselinePlaybackRestartAtMs = lastPlaybackRestartAtMs
            baselineFirstFrameAtMs = lastFirstFrameAtMs

            // Rotation / surface relayout may produce an "end-file" for the old surface.
            // Ignore end-file briefly to avoid false failure + unnecessary line switch.
            suppressEndFileUntilElapsedMs = SystemClock.elapsedRealtime() + 2_000L
            MPVLib.detachSurface()
            surfaceAttached = false
            currentSurface = null
        }
    }

    override fun release() {
        runCatching { postAttachJob?.cancel() }
        postAttachJob = null
        runCatching { scope.cancel() }
        runCatching { detachSurface() }
        runCatching { MPVLib.removeObserver(observer) }
        runCatching { MPVLib.removeLogObserver(logObserver) }
        runCatching { MPVLib.destroy() }
        MpvGlobal.release()
    }

    private fun loadFile(url: String, headers: Map<String, String>) {
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
        loadedUrl = url
        loadedHeaders = headers
    }
}
