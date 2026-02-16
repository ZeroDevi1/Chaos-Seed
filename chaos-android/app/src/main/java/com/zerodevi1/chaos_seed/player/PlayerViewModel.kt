package com.zerodevi1.chaos_seed.player

import android.app.Application
import android.view.Surface
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.zerodevi1.chaos_seed.core.backend.BackendHolder
import com.zerodevi1.chaos_seed.core.backend.ChaosBackend
import com.zerodevi1.chaos_seed.core.model.DanmakuMessage
import com.zerodevi1.chaos_seed.core.model.LivestreamDecodeManifestResult
import com.zerodevi1.chaos_seed.core.model.LivestreamVariant
import com.zerodevi1.chaos_seed.player.engine.PlayerEngine
import com.zerodevi1.chaos_seed.player.engine.PlayerEngineFactory
import com.zerodevi1.chaos_seed.player.engine.PlayerState
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

    private var danmakuJob: Job? = null
    private var pipTemporarilyHidDanmaku = false

    private val _liveTitle = MutableStateFlow<String?>(null)
    val liveTitle: StateFlow<String?> = _liveTitle

    private val _variants = MutableStateFlow<List<LivestreamVariant>>(emptyList())
    val variants: StateFlow<List<LivestreamVariant>> = _variants

    private val _variantId = MutableStateFlow<String?>(null)
    val variantId: StateFlow<String?> = _variantId

    private val _lines = MutableStateFlow<List<String>>(emptyList())
    val lines: StateFlow<List<String>> = _lines

    private val _lineIndex = MutableStateFlow(0)
    val lineIndex: StateFlow<Int> = _lineIndex

    private var engineStateJob: Job? = null

    private val settingsState =
        settingsRepo.settingsFlow.stateIn(viewModelScope, SharingStarted.Eagerly, null)

    init {
        // Keep danmaku default in sync with persisted settings.
        viewModelScope.launch {
            settingsRepo.settingsFlow.collect { st ->
                _danmakuEnabled.value = st.danmakuEnabled
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
                _variants.value = man.variants.sortedByDescending { it.quality }

                val preferredVariant = initialVariantId?.trim().takeIf { !it.isNullOrEmpty() }
                openLiveInternal(inTrim, preferredVariant)
            }.onFailure { e ->
                _state.value = _state.value.copy(error = e.message)
                _snackbar.tryEmit("解析失败：${e.message ?: e::class.java.simpleName}")
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
                _snackbar.tryEmit("切换清晰度失败：${e.message ?: e::class.java.simpleName}")
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
            }.onFailure { e ->
                _snackbar.tryEmit("切换线路失败：${e.message ?: e::class.java.simpleName}")
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

        val headers = buildMap {
            val ua = open.userAgent?.trim().orEmpty()
            val ref = open.referer?.trim().orEmpty()
            if (ua.isNotEmpty()) put("User-Agent", ua)
            if (ref.isNotEmpty()) put("Referer", ref)
        }

        val url = urls.firstOrNull().orEmpty()
        currentUrl = url
        currentHeaders = headers

        val preferred = settingsState.value?.playerEngine ?: PlayerEngineType.Exo
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

        val sid = liveSessionId ?: return
        if (!_danmakuEnabled.value) return

        danmakuJob = viewModelScope.launch {
            backend.danmakuStream(sid).collect { m ->
                _danmakuTail.update { old ->
                    val next = (old + m).takeLast(24)
                    next
                }
            }
        }
    }

    fun open(url: String, headers: Map<String, String>) {
        currentUrl = url
        currentHeaders = headers
        viewModelScope.launch {
            val preferred = settingsState.value?.playerEngine ?: PlayerEngineType.Exo
            ensureEngine(preferred)
            runCatching {
                engine!!.open(url, headers)
                applyMuteToEngine()
            }.onFailure { e ->
                _state.value = _state.value.copy(error = e.message)
                _snackbar.tryEmit("播放失败：${e.message ?: e::class.java.simpleName}")
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
                _snackbar.tryEmit("操作失败：${ex.message ?: ex::class.java.simpleName}")
            }
        }
    }

    fun toggleMute() {
        _muted.value = !_muted.value
        viewModelScope.launch { applyMuteToEngine() }
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

                // Forward engine state into VM state.
                engineStateJob?.cancel()
                engineStateJob = viewModelScope.launch { next.state.collect { _state.value = it } }

                surface?.let { next.attachSurface(it) }
                if (!url.isNullOrBlank()) {
                    next.open(url, headers)
                    applyMuteToEngine()
                }
            }.onFailure { e ->
                _snackbar.tryEmit("切换到 ${type.label} 失败，回退 EXO：${e.message ?: e::class.java.simpleName}")
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

        // Bridge engine -> VM state.
        engineStateJob?.cancel()
        engineStateJob = viewModelScope.launch { next.state.collect { _state.value = it } }
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

    fun onPipModeChanged(inPip: Boolean) {
        val pipHide = settingsState.value?.pipHideDanmaku ?: true
        if (inPip && pipHide) {
            if (_danmakuEnabled.value) {
                pipTemporarilyHidDanmaku = true
                _danmakuEnabled.value = false
                danmakuJob?.cancel()
                danmakuJob = null
                _danmakuTail.value = emptyList()
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
        danmakuJob?.cancel()
        runCatching { engine?.release() }
        engine = null

        liveSessionId?.let { sid -> runCatching { runBlocking { backend.closeLive(sid) } } }
        super.onCleared()
    }
}
