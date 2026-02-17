package com.zerodevi1.chaos_seed.settings

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.zerodevi1.chaos_seed.player.PlayerEngineType
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class SettingsViewModel(app: Application) : AndroidViewModel(app) {
    private val repo = SettingsRepository(app.applicationContext)

    val state: StateFlow<AppSettings> =
        repo.settingsFlow.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), AppSettings())

    fun setPlayerEngine(engine: PlayerEngineType) {
        viewModelScope.launch { repo.setPlayerEngine(engine) }
    }

    fun setThemeMode(mode: AppThemeMode) {
        viewModelScope.launch { repo.setThemeMode(mode) }
    }

    fun setPipHideDanmaku(v: Boolean) {
        viewModelScope.launch { repo.setPipHideDanmaku(v) }
    }

    fun setDanmakuEnabled(v: Boolean) {
        viewModelScope.launch { repo.setDanmakuEnabled(v) }
    }

    fun setDanmuFontSizeSp(v: Float) {
        viewModelScope.launch { repo.setDanmuFontSizeSp(v) }
    }

    fun setDanmuOpacity(v: Float) {
        viewModelScope.launch { repo.setDanmuOpacity(v) }
    }

    fun setDanmuArea(v: Float) {
        viewModelScope.launch { repo.setDanmuArea(v) }
    }

    fun setDanmuSpeedSeconds(v: Int) {
        viewModelScope.launch { repo.setDanmuSpeedSeconds(v) }
    }

    fun setDanmuStrokeWidthDp(v: Float) {
        viewModelScope.launch { repo.setDanmuStrokeWidthDp(v) }
    }

    fun setDanmuBlockWordsEnabled(v: Boolean) {
        viewModelScope.launch { repo.setDanmuBlockWordsEnabled(v) }
    }

    fun setDanmuBlockWordsRaw(v: String) {
        viewModelScope.launch { repo.setDanmuBlockWordsRaw(v) }
    }

    fun setQqMusicCookieJson(rawJson: String?) {
        viewModelScope.launch { repo.setQqMusicCookieJson(rawJson) }
    }

    fun setKugouBaseUrl(v: String?) {
        viewModelScope.launch { repo.setKugouBaseUrl(v) }
    }

    fun setNeteaseBaseUrls(v: String) {
        viewModelScope.launch { repo.setNeteaseBaseUrls(v) }
    }

    fun setNeteaseAnonymousCookieUrl(v: String) {
        viewModelScope.launch { repo.setNeteaseAnonymousCookieUrl(v) }
    }

    fun setMusicDownloadConcurrency(v: Int) {
        viewModelScope.launch { repo.setMusicDownloadConcurrency(v) }
    }

    fun setMusicDownloadRetries(v: Int) {
        viewModelScope.launch { repo.setMusicDownloadRetries(v) }
    }

    fun setMusicPathTemplate(v: String) {
        viewModelScope.launch { repo.setMusicPathTemplate(v) }
    }
}
