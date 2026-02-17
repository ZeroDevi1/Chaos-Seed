package com.zerodevi1.chaos_seed.settings

import android.content.Context
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.zerodevi1.chaos_seed.player.PlayerEngineType
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.dataStore by preferencesDataStore(name = "settings")

class SettingsRepository(private val appContext: Context) {
    private object Keys {
        val themeMode: Preferences.Key<String> = stringPreferencesKey("settings.theme.mode")
        val playerEngine: Preferences.Key<String> = stringPreferencesKey("settings.player.engine")
        val pipHideDanmaku: Preferences.Key<Boolean> = booleanPreferencesKey("settings.player.pipHideDanmaku")
        val danmakuEnabled: Preferences.Key<Boolean> = booleanPreferencesKey("settings.danmaku.enabled")
        val danmuFontSizeSp: Preferences.Key<Float> = floatPreferencesKey("settings.danmu.fontSizeSp")
        val danmuOpacity: Preferences.Key<Float> = floatPreferencesKey("settings.danmu.opacity")
        val danmuArea: Preferences.Key<Float> = floatPreferencesKey("settings.danmu.area")
        val danmuSpeedSeconds: Preferences.Key<Int> = intPreferencesKey("settings.danmu.speedSeconds")
        val danmuStrokeWidthDp: Preferences.Key<Float> = floatPreferencesKey("settings.danmu.strokeWidthDp")
        val danmuBlockWordsEnabled: Preferences.Key<Boolean> = booleanPreferencesKey("settings.danmu.blockWordsEnabled")
        val danmuBlockWordsRaw: Preferences.Key<String> = stringPreferencesKey("settings.danmu.blockWordsRaw")

        val qqMusicCookieJson: Preferences.Key<String> = stringPreferencesKey("settings.music.qqCookieJson")
        val kugouBaseUrl: Preferences.Key<String> = stringPreferencesKey("settings.music.kugouBaseUrl")
        val neteaseBaseUrls: Preferences.Key<String> = stringPreferencesKey("settings.music.neteaseBaseUrls")
        val neteaseAnonymousCookieUrl: Preferences.Key<String> = stringPreferencesKey("settings.music.neteaseAnonymousCookieUrl")
        val musicDownloadConcurrency: Preferences.Key<Int> = intPreferencesKey("settings.music.download.concurrency")
        val musicDownloadRetries: Preferences.Key<Int> = intPreferencesKey("settings.music.download.retries")
        val musicPathTemplate: Preferences.Key<String> = stringPreferencesKey("settings.music.download.pathTemplate")
    }

    val settingsFlow: Flow<AppSettings> =
        appContext.dataStore.data.map { p ->
            val themeRaw = p[Keys.themeMode]
            val theme = AppThemeMode.fromPersisted(themeRaw) ?: AppThemeMode.System
            val engineRaw = p[Keys.playerEngine]?.trim().orEmpty()
            val engine = PlayerEngineType.fromPersisted(engineRaw) ?: PlayerEngineType.Exo
            val pipHide = p[Keys.pipHideDanmaku] ?: true
            val dmEnabled = p[Keys.danmakuEnabled] ?: true
            val danmuFontSizeSp = (p[Keys.danmuFontSizeSp] ?: 18f).coerceIn(12f, 32f)
            val danmuOpacity = (p[Keys.danmuOpacity] ?: 0.85f).coerceIn(0.2f, 1.0f)
            val danmuArea = (p[Keys.danmuArea] ?: 0.6f).coerceIn(0.25f, 1.0f)
            val danmuSpeedSeconds = (p[Keys.danmuSpeedSeconds] ?: 8).coerceIn(4, 16)
            val danmuStrokeWidthDp = (p[Keys.danmuStrokeWidthDp] ?: 1.0f).coerceIn(0f, 4f)
            val danmuBlockWordsEnabled = p[Keys.danmuBlockWordsEnabled] ?: false
            val danmuBlockWordsRaw = p[Keys.danmuBlockWordsRaw]?.trim().orEmpty()
            AppSettings(
                themeMode = theme,
                playerEngine = engine,
                pipHideDanmaku = pipHide,
                danmakuEnabled = dmEnabled,
                danmuFontSizeSp = danmuFontSizeSp,
                danmuOpacity = danmuOpacity,
                danmuArea = danmuArea,
                danmuSpeedSeconds = danmuSpeedSeconds,
                danmuStrokeWidthDp = danmuStrokeWidthDp,
                danmuBlockWordsEnabled = danmuBlockWordsEnabled,
                danmuBlockWordsRaw = danmuBlockWordsRaw,
                qqMusicCookieJson = p[Keys.qqMusicCookieJson]?.trim()?.takeIf { it.isNotEmpty() },
                kugouBaseUrl = p[Keys.kugouBaseUrl]?.trim()?.takeIf { it.isNotEmpty() },
                neteaseBaseUrls = p[Keys.neteaseBaseUrls]?.trim().orEmpty(),
                neteaseAnonymousCookieUrl = p[Keys.neteaseAnonymousCookieUrl]?.trim().orEmpty(),
                musicDownloadConcurrency = (p[Keys.musicDownloadConcurrency] ?: 3).coerceIn(1, 16),
                musicDownloadRetries = (p[Keys.musicDownloadRetries] ?: 2).coerceIn(0, 10),
                musicPathTemplate = p[Keys.musicPathTemplate]?.trim().orEmpty(),
            )
        }

    suspend fun setThemeMode(mode: AppThemeMode) {
        appContext.dataStore.edit { it[Keys.themeMode] = mode.persisted }
    }

    suspend fun setPlayerEngine(engine: PlayerEngineType) {
        appContext.dataStore.edit { it[Keys.playerEngine] = engine.persistedValue }
    }

    suspend fun setPipHideDanmaku(v: Boolean) {
        appContext.dataStore.edit { it[Keys.pipHideDanmaku] = v }
    }

    suspend fun setDanmakuEnabled(v: Boolean) {
        appContext.dataStore.edit { it[Keys.danmakuEnabled] = v }
    }

    suspend fun setDanmuFontSizeSp(v: Float) {
        appContext.dataStore.edit { it[Keys.danmuFontSizeSp] = v.coerceIn(12f, 32f) }
    }

    suspend fun setDanmuOpacity(v: Float) {
        appContext.dataStore.edit { it[Keys.danmuOpacity] = v.coerceIn(0.2f, 1.0f) }
    }

    suspend fun setDanmuArea(v: Float) {
        appContext.dataStore.edit { it[Keys.danmuArea] = v.coerceIn(0.25f, 1.0f) }
    }

    suspend fun setDanmuSpeedSeconds(v: Int) {
        appContext.dataStore.edit { it[Keys.danmuSpeedSeconds] = v.coerceIn(4, 16) }
    }

    suspend fun setDanmuStrokeWidthDp(v: Float) {
        appContext.dataStore.edit { it[Keys.danmuStrokeWidthDp] = v.coerceIn(0f, 4f) }
    }

    suspend fun setDanmuBlockWordsEnabled(v: Boolean) {
        appContext.dataStore.edit { it[Keys.danmuBlockWordsEnabled] = v }
    }

    suspend fun setDanmuBlockWordsRaw(v: String) {
        appContext.dataStore.edit { it[Keys.danmuBlockWordsRaw] = v.trim() }
    }

    suspend fun setQqMusicCookieJson(rawJson: String?) {
        appContext.dataStore.edit { p ->
            val s = (rawJson ?: "").trim()
            if (s.isEmpty()) p.remove(Keys.qqMusicCookieJson) else p[Keys.qqMusicCookieJson] = s
        }
    }

    suspend fun setKugouBaseUrl(v: String?) {
        appContext.dataStore.edit { p ->
            val s = (v ?: "").trim()
            if (s.isEmpty()) p.remove(Keys.kugouBaseUrl) else p[Keys.kugouBaseUrl] = s
        }
    }

    suspend fun setNeteaseBaseUrls(v: String) {
        appContext.dataStore.edit { it[Keys.neteaseBaseUrls] = v }
    }

    suspend fun setNeteaseAnonymousCookieUrl(v: String) {
        appContext.dataStore.edit { it[Keys.neteaseAnonymousCookieUrl] = v }
    }

    suspend fun setMusicDownloadConcurrency(v: Int) {
        appContext.dataStore.edit { it[Keys.musicDownloadConcurrency] = v.coerceIn(1, 16) }
    }

    suspend fun setMusicDownloadRetries(v: Int) {
        appContext.dataStore.edit { it[Keys.musicDownloadRetries] = v.coerceIn(0, 10) }
    }

    suspend fun setMusicPathTemplate(v: String) {
        appContext.dataStore.edit { it[Keys.musicPathTemplate] = v }
    }
}
