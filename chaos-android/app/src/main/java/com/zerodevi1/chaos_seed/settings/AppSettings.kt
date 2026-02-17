package com.zerodevi1.chaos_seed.settings

import com.zerodevi1.chaos_seed.player.PlayerEngineType

enum class AppThemeMode(val persisted: String) {
    System("system"),
    Dark("dark"),
    Light("light"),
    ;

    companion object {
        fun fromPersisted(raw: String?): AppThemeMode? =
            entries.firstOrNull { it.persisted == (raw ?: "").trim() }
    }
}

data class AppSettings(
    val themeMode: AppThemeMode = AppThemeMode.System,
    val playerEngine: PlayerEngineType = PlayerEngineType.Exo,
    val pipHideDanmaku: Boolean = true,
    val danmakuEnabled: Boolean = true,
    // Danmu (Android player)
    val danmuFontSizeSp: Float = 18f,
    val danmuOpacity: Float = 0.85f,
    val danmuArea: Float = 0.6f,
    val danmuSpeedSeconds: Int = 8,
    val danmuStrokeWidthDp: Float = 1.0f,
    val danmuBlockWordsEnabled: Boolean = false,
    val danmuBlockWordsRaw: String = "",
    // Music
    val qqMusicCookieJson: String? = null,
    val kugouBaseUrl: String? = null,
    val neteaseBaseUrls: String = "",
    val neteaseAnonymousCookieUrl: String = "",
    val musicDownloadConcurrency: Int = 3,
    val musicDownloadRetries: Int = 2,
    val musicPathTemplate: String = "",
)
