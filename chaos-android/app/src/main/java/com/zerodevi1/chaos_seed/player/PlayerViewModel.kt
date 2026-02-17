package com.zerodevi1.chaos_seed.player

import android.app.Application
import android.view.Surface
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.zerodevi1.chaos_seed.core.backend.BackendHolder
import com.zerodevi1.chaos_seed.core.backend.ChaosBackend
import com.zerodevi1.chaos_seed.core.model.DanmakuMessage
import com.zerodevi1.chaos_seed.core.model.LivestreamDecodeManifestResult
import com.zerodevi1.chaos_seed.core.model.LivestreamInfo
import com.zerodevi1.chaos_seed.core.model.LivestreamVariant
import com.zerodevi1.chaos_seed.player.engine.PlayerEngine
import com.zerodevi1.chaos_seed.player.engine.PlayerEngineFactory
import com.zerodevi1.chaos_seed.player.engine.PlayerState
import com.zerodevi1.chaos_seed.settings.AppSettings
import com.zerodevi1.chaos_seed.settings.SettingsRepository
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay
import kotlinx.coroutines.Job
import kotlinx.coroutines.runBlocking

class PlayerViewModel(app: Application) : AndroidViewModel(app) {
    private val backend: ChaosBackend = BackendHolder.get(app.applicationContext)
    private val settingsRepo = SettingsRepository(app.applicationContext)
    private val engineFactory = PlayerEngineFactory(app.applicationContext)

    private var engine: PlayerEngine? = null
    private var surface: Surface? = null

    private val _engineType = MutableStateFlow(PlayerEngineType.Exo)
    val engineType: StateFlow<PlayerEngineType> = _engineType

    private val _state = MutableStateFlow(PlayerState())
    val state: StateFlow<PlayerState> = _state

    private val _muted = MutableStateFlow(false)
    val muted: StateFlow<Boolean> = _muted

    private val _snackbar = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val snackbar = _snackbar.asSharedFlow()

    private var currentUrl: String? = null
    private var currentHeaders: Map<String, String> = emptyMap()

    private var liveInput: String? = null
    private var liveSessionId: String? = null
    private var liveManifest: LivestreamDecodeManifestResult? = null

    private val _danmakuEnabled = MutableStateFlow(true)
    val danmakuEnabled: StateFlow<Boolean> = _danmakuEnabled

    private val _danmakuTail = MutableStateFlow<List<DanmakuMessage>>(emptyList())
    val danmakuTail: StateFlow<List<DanmakuMessage>> = _danmakuTail

    private val _danmuList = MutableStateFlow<List<DanmakuMessage>>(emptyList())
    val danmuList: StateFlow<List<DanmakuMessage>> = _danmuList

    private var danmakuJob: Job? = null
    private var pipTemporarilyHidDanmaku = false

    private val _liveTitle = MutableStateFlow<String?>(null)
    val liveTitle: StateFlow<String?> = _liveTitle

    private val _liveInfo = MutableStateFlow<LivestreamInfo?>(null)
    val liveInfo: StateFlow<LivestreamInfo?> = _liveInfo

    private val _variants = MutableStateFlow<List<LivestreamVariant>>(emptyList())
    val variants: StateFlow<List<LivestreamVariant>> = _variants

    private val _variantId = MutableStateFlow<String?>(null)
    val variantId: StateFlow<String?> = _variantId

    private val _lines = MutableStateFlow<List<String>>(emptyList())
    val lines: StateFlow<List<String>> = _lines

    private val _lineIndex = MutableStateFlow(0)
    val lineIndex: StateFlow<Int> = _lineIndex

    // Record lines that have already triggered an auto-fallback attempt.
    private val autoFallbackTriedFromLine = mutableSetOf<Int>()
    private var bufferingWatchdogJob: Job? = null
    private var mpvEngineFallbackTriggered = false

    private var engineStateJob: Job? = null

    private val settingsState =
        settingsRepo.settingsFlow.stateIn(viewModelScope, SharingStarted.Eagerly, AppSettings())
    val settings: StateFlow<AppSettings> = settingsState

    @Volatile
    private var danmuBlockWordsEnabled: Boolean = false

    @Volatile
    private var danmuBlockWords: List<String> = emptyList()

    init {
        // Keep danmaku + filters in sync with persisted settings.
        viewModelScope.launch {
            settingsRepo.settingsFlow.collect { st ->
                _danmakuEnabled.value = st.danmakuEnabled
                danmuBlockWordsEnabled = st.danmuBlockWordsEnabled
                danmuBlockWords = parseBlockWords(st.danmuBlockWordsRaw)
            }
        }
    }

    fun startLive(input: String, initialVariantId: String?) {
        val inTrim = input.trim()
        if (inTrim.isEmpty()) return
        liveInput = inTrim
        viewModelScope.launch {
            runCatching {
                val man = backend.decodeManifest(inTrim)
                liveManifest = man
                _liveTitle.value = man.info.title
                _liveInfo.value = man.info
                _variants.value = man.variants.sortedByDescending { it.quality }

                val preferredVariant = initialVariantId?.trim().takeIf { !it.isNullOrEmpty() }
                openLiveInternal(inTrim, preferredVariant)
            }.onFailure { e ->
                _state.value = _state.value.copy(error = e.message)
                _snackbar.tryEmit("解析失败：${sanitizeForSnackbar(e.message ?: e::class.java.simpleName)}")
            }
        }
    }

    fun switchVariant(nextVariantId: String) {
        val inTrim = liveInput?.trim().orEmpty()
        if (inTrim.isEmpty()) return
        viewModelScope.launch {
            runCatching {
                openLiveInternal(inTrim, nextVariantId.trim())
            }.onFailure { e ->
                _snackbar.tryEmit("切换清晰度失败：${sanitizeForSnackbar(e.message ?: e::class.java.simpleName)}")
            }
        }
    }

    fun switchLine(idx: Int) {
        val urls = _lines.value
        if (idx !in urls.indices) return
        val u = urls[idx].trim()
        if (u.isEmpty()) return
        _lineIndex.value = idx
        currentUrl = u
        viewModelScope.launch {
            runCatching {
                engine?.open(u, currentHeaders)
                applyMuteToEngine()
                restartDanmaku()
            }.onFailure { e ->
                _snackbar.tryEmit("切换线路失败：${sanitizeForSnackbar(e.message ?: e::class.java.simpleName)}")
            }
        }
    }

    private suspend fun openLiveInternal(inTrim: String, requestedVariantId: String?) {
        // Close previous danmaku session first to avoid native leaks.
        liveSessionId?.let { sid ->
            runCatching { backend.closeLive(sid) }
        }
        liveSessionId = null

        val open = backend.openLive(inTrim, requestedVariantId)
        liveSessionId = open.sessionId
        _variantId.value = open.variantId

        val urls = buildList {
            add(open.url)
            addAll(open.backupUrls)
        }.map { it.trim() }.filter { it.isNotEmpty() }.distinct()
        _lines.value = urls
        _lineIndex.value = 0
        autoFallbackTriedFromLine.clear()
        mpvEngineFallbackTriggered = false

        val headers = buildMap {
            val ua = open.userAgent?.trim().orEmpty()
            val ref = open.referer?.trim().orEmpty()
            if (ua.isNotEmpty()) put("User-Agent", ua)
            if (ref.isNotEmpty()) put("Referer", ref)
        }

        val url = urls.firstOrNull().orEmpty()
        currentUrl = url
        currentHeaders = headers

        val preferred = settingsState.value.playerEngine
        ensureEngine(preferred)
        engine!!.open(url, headers)
        applyMuteToEngine()

        restartDanmaku()
    }

    fun setDanmakuEnabled(v: Boolean) {
        _danmakuEnabled.value = v
        viewModelScope.launch { settingsRepo.setDanmakuEnabled(v) }
        viewModelScope.launch { restartDanmaku() }
    }

    fun toggleDanmakuEnabled() {
        setDanmakuEnabled(!_danmakuEnabled.value)
    }

    private suspend fun restartDanmaku() {
        danmakuJob?.cancel()
        danmakuJob = null
        _danmakuTail.value = emptyList()
        _danmuList.value = emptyList()

        val sid = liveSessionId ?: return
        if (!_danmakuEnabled.value) return

        danmakuJob = viewModelScope.launch {
            backend.danmakuStream(sid).collect { m ->
                val text = m.text.trim()
                if (danmuBlockWordsEnabled && danmuBlockWords.isNotEmpty()) {
                    if (danmuBlockWords.any { kw -> text.contains(kw) }) return@collect
                }
                _danmakuTail.update { old ->
                    val next = (old + m).takeLast(24)
                    next
                }
                _danmuList.update { old ->
                    val next = (old + m).takeLast(200)
                    next
                }
            }
        }
    }

    fun open(url: String, headers: Map<String, String>) {
        liveSessionId?.let { sid ->
            viewModelScope.launch { runCatching { backend.closeLive(sid) } }
        }
        liveSessionId = null
        currentUrl = url
        currentHeaders = headers
        liveInput = null
        liveManifest = null
        _variants.value = emptyList()
        _variantId.value = null
        _lines.value = emptyList()
        _lineIndex.value = 0
        _liveTitle.value = null
        _liveInfo.value = null
        danmakuJob?.cancel()
        danmakuJob = null
        _danmakuTail.value = emptyList()
        _danmuList.value = emptyList()
        viewModelScope.launch {
            val preferred = settingsState.value.playerEngine
            ensureEngine(preferred)
            runCatching {
                engine!!.open(url, headers)
                applyMuteToEngine()
            }.onFailure { e ->
                _state.value = _state.value.copy(error = e.message)
                _snackbar.tryEmit("播放失败：${sanitizeForSnackbar(e.message ?: e::class.java.simpleName)}")
            }
        }
    }

    fun attachSurface(surface: Surface) {
        this.surface = surface
        engine?.attachSurface(surface)
    }

    fun detachSurface() {
        engine?.detachSurface()
        surface = null
    }

    fun togglePlayPause() {
        val e = engine ?: return
        viewModelScope.launch {
            runCatching {
                if (state.value.playing) e.pause() else e.play()
            }.onFailure { ex ->
                _snackbar.tryEmit("操作失败：${sanitizeForSnackbar(ex.message ?: ex::class.java.simpleName)}")
            }
        }
    }

    fun toggleMute() {
        _muted.value = !_muted.value
        viewModelScope.launch { applyMuteToEngine() }
    }

    fun reconnect() {
        val url = currentUrl?.trim().orEmpty()
        if (url.isEmpty()) return
        viewModelScope.launch {
            runCatching {
                engine?.open(url, currentHeaders)
                applyMuteToEngine()
                restartDanmaku()
            }.onFailure { e ->
                _snackbar.tryEmit("重连失败：${sanitizeForSnackbar(e.message ?: e::class.java.simpleName)}")
            }
        }
    }

    fun setDanmuFontSizeSp(v: Float) {
        viewModelScope.launch { settingsRepo.setDanmuFontSizeSp(v) }
    }

    fun setDanmuOpacity(v: Float) {
        viewModelScope.launch { settingsRepo.setDanmuOpacity(v) }
    }

    fun setDanmuArea(v: Float) {
        viewModelScope.launch { settingsRepo.setDanmuArea(v) }
    }

    fun setDanmuSpeedSeconds(v: Int) {
        viewModelScope.launch { settingsRepo.setDanmuSpeedSeconds(v) }
    }

    fun setDanmuStrokeWidthDp(v: Float) {
        viewModelScope.launch { settingsRepo.setDanmuStrokeWidthDp(v) }
    }

    fun setDanmuBlockWordsEnabled(v: Boolean) {
        viewModelScope.launch { settingsRepo.setDanmuBlockWordsEnabled(v) }
    }

    fun setDanmuBlockWordsRaw(v: String) {
        viewModelScope.launch { settingsRepo.setDanmuBlockWordsRaw(v) }
    }

    fun resetDanmuSettingsToDefaults() {
        viewModelScope.launch {
            settingsRepo.setDanmuFontSizeSp(18f)
            settingsRepo.setDanmuOpacity(0.85f)
            settingsRepo.setDanmuArea(0.6f)
            settingsRepo.setDanmuSpeedSeconds(8)
            settingsRepo.setDanmuStrokeWidthDp(1.0f)
            settingsRepo.setDanmuBlockWordsEnabled(false)
            settingsRepo.setDanmuBlockWordsRaw("")
        }
    }

    fun switchEngine(type: PlayerEngineType) {
        viewModelScope.launch {
            // Persist immediately (even if we fallback later).
            settingsRepo.setPlayerEngine(type)

            val url = currentUrl
            val headers = currentHeaders
            val old = engine
            val oldType = _engineType.value
            if (type == oldType && old != null) return@launch

            // Best-effort cleanup; never block switching due to cleanup failure.
            runCatching { old?.detachSurface() }
            runCatching { old?.release() }
            engine = null

            runCatching {
                val next = engineFactory.create(type)
                engine = next
                _engineType.value = type
                if (type == PlayerEngineType.Mpv) {
                    mpvEngineFallbackTriggered = false
                }

                // Forward engine state into VM state.
                engineStateJob?.cancel()
                engineStateJob = viewModelScope.launch { next.state.collect { onEngineState(it) } }

                surface?.let { next.attachSurface(it) }
                if (!url.isNullOrBlank()) {
                    next.open(url, headers)
                    applyMuteToEngine()
                }
            }.onFailure { e ->
                _snackbar.tryEmit(
                    "切换到 ${type.label} 失败，回退 EXO：${sanitizeForSnackbar(e.message ?: e::class.java.simpleName)}",
                )
                fallbackToExo()
            }
        }
    }

    private suspend fun ensureEngine(type: PlayerEngineType) {
        if (engine != null && _engineType.value == type) return

        runCatching {
            engine?.detachSurface()
            engine?.release()
        }
        engine = null

        val next = engineFactory.create(type)
        engine = next
        _engineType.value = type
        if (type == PlayerEngineType.Mpv) {
            mpvEngineFallbackTriggered = false
        }

        // Bridge engine -> VM state.
        engineStateJob?.cancel()
        engineStateJob = viewModelScope.launch { next.state.collect { onEngineState(it) } }
        surface?.let { next.attachSurface(it) }
    }

    private suspend fun fallbackToExo() {
        runCatching { settingsRepo.setPlayerEngine(PlayerEngineType.Exo) }
        ensureEngine(PlayerEngineType.Exo)
        currentUrl?.let { u ->
            runCatching { engine?.open(u, currentHeaders) }
        }
        applyMuteToEngine()
    }

    private suspend fun applyMuteToEngine() {
        val e = engine ?: return
        val vol = if (_muted.value) 0 else 100
        runCatching { e.setVolume(vol) }
    }

    private fun onEngineState(next: PlayerState) {
        val prevError = _state.value.error
        _state.value = next
        handleBufferingWatchdog(next)

        val currentError = next.error?.trim().orEmpty()
        val shouldHandleError = currentError.isNotEmpty() && currentError != prevError?.trim().orEmpty()
        if (!shouldHandleError) return
        if (maybeFallbackEngineFromMpvError(currentError)) return
        maybeAutoFallbackLine(currentError)
    }

    private fun maybeAutoFallbackLine(errorMsg: String) {
        val urls = _lines.value
        if (urls.size <= 1) return

        val fromIdx = _lineIndex.value
        if (fromIdx !in urls.indices) return
        if (!autoFallbackTriedFromLine.add(fromIdx)) return

        val nextIdx = fromIdx + 1
        if (nextIdx !in urls.indices) return

        val detail = sanitizeForSnackbar(errorMsg)
        val msg =
            if (detail.isBlank()) {
                "线路 ${fromIdx + 1} 播放失败，自动切换到线路 ${nextIdx + 1}/${urls.size}"
            } else {
                "线路 ${fromIdx + 1} 播放失败（$detail），自动切换到线路 ${nextIdx + 1}/${urls.size}"
            }
        _snackbar.tryEmit(msg)
        switchLine(nextIdx)
    }

    private fun handleBufferingWatchdog(state: PlayerState) {
        if (state.playing || !state.buffering || !state.error.isNullOrBlank()) {
            bufferingWatchdogJob?.cancel()
            bufferingWatchdogJob = null
            return
        }

        if (bufferingWatchdogJob?.isActive == true) return
        val watchedLine = _lineIndex.value
        bufferingWatchdogJob = viewModelScope.launch {
            delay(BUFFERING_FAILOVER_TIMEOUT_MS)
            val latest = _state.value
            val stillStuck =
                _lineIndex.value == watchedLine &&
                    latest.buffering &&
                    !latest.playing &&
                    latest.error.isNullOrBlank()
            if (!stillStuck) return@launch
            maybeAutoFallbackLine("缓冲超时")
        }
    }

    private fun maybeFallbackEngineFromMpvError(errorMsg: String): Boolean {
        if (_engineType.value != PlayerEngineType.Mpv) return false
        if (mpvEngineFallbackTriggered) return true

        val low = errorMsg.lowercase()
        val isVideoPipelineFailure =
            low.contains("视频链初始化失败") ||
                low.contains("视频格式转换失败") ||
                low.contains("渲染器不支持") ||
                low.contains("could not initialize video chain") ||
                low.contains("cannot convert decoder/filter output")
        if (!isVideoPipelineFailure) return false

        mpvEngineFallbackTriggered = true
        viewModelScope.launch {
            _snackbar.emit("MPV 视频渲染失败，自动回退 EXO 引擎")
            fallbackToExo()
        }
        return true
    }

    fun onPipModeChanged(inPip: Boolean) {
        val pipHide = settingsState.value.pipHideDanmaku
        if (inPip && pipHide) {
            if (_danmakuEnabled.value) {
                pipTemporarilyHidDanmaku = true
                _danmakuEnabled.value = false
                danmakuJob?.cancel()
                danmakuJob = null
                _danmakuTail.value = emptyList()
                _danmuList.value = emptyList()
            }
            return
        }
        if (!inPip && pipTemporarilyHidDanmaku) {
            pipTemporarilyHidDanmaku = false
            _danmakuEnabled.value = true
            viewModelScope.launch { restartDanmaku() }
        }
    }

    override fun onCleared() {
        detachSurface()
        engineStateJob?.cancel()
        bufferingWatchdogJob?.cancel()
        danmakuJob?.cancel()
        runCatching { engine?.release() }
        engine = null

        liveSessionId?.let { sid -> runCatching { runBlocking { backend.closeLive(sid) } } }
        super.onCleared()
    }

    companion object {
        private const val BUFFERING_FAILOVER_TIMEOUT_MS = 12_000L
    }
}

private fun parseBlockWords(raw: String): List<String> {
    return raw
        .lineSequence()
        .map { it.trim() }
        .filter { it.isNotEmpty() }
        .distinct()
        .toList()
}
