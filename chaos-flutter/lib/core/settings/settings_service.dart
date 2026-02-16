import 'package:shared_preferences/shared_preferences.dart';

import 'settings_model.dart';

class SettingsService {
  static const _kThemeMode = 'settings.themeMode';
  static const _kBackdropMode = 'settings.backdropMode';
  static const _kLiveBackend = 'settings.backend.live';
  static const _kMusicBackend = 'settings.backend.music';
  static const _kLyricsBackend = 'settings.backend.lyrics';
  static const _kDanmakuBackend = 'settings.backend.danmaku';
  static const _kAutoUpdateEnabled = 'settings.update.enabled';
  static const _kAutoUpdateIntervalHours = 'settings.update.intervalHours';
  static const _kKugouBaseUrl = 'settings.music.kugouBaseUrl';
  static const _kNeteaseBaseUrls = 'settings.music.neteaseBaseUrls';
  static const _kNeteaseAnonUrl = 'settings.music.neteaseAnonymousCookieUrl';
  static const _kMusicConcurrency = 'settings.music.download.concurrency';
  static const _kMusicRetries = 'settings.music.download.retries';
  static const _kMusicPathTemplate = 'settings.music.download.pathTemplate';
  static const _kQqCookieJson = 'settings.music.qq.cookieJson';
  static const _kPipHideDanmu = 'settings.player.pipHideDanmu';
  static const _kDanmuFontSize = 'settings.player.danmu.fontSize';
  static const _kDanmuOpacity = 'settings.player.danmu.opacity';
  static const _kDanmuArea = 'settings.player.danmu.area';
  static const _kDanmuSpeedSeconds = 'settings.player.danmu.speedSeconds';

  Future<AppSettings> load() async {
    final sp = await SharedPreferences.getInstance();
    final d = AppSettings.defaults();
    return AppSettings(
      themeMode: _parseThemeMode(sp.getString(_kThemeMode)) ?? d.themeMode,
      backdropMode: sp.getString(_kBackdropMode) ?? d.backdropMode,
      liveBackendMode:
          _parseBackendMode(sp.getString(_kLiveBackend)) ?? d.liveBackendMode,
      musicBackendMode:
          _parseBackendMode(sp.getString(_kMusicBackend)) ?? d.musicBackendMode,
      lyricsBackendMode: _parseBackendMode(sp.getString(_kLyricsBackend)) ??
          d.lyricsBackendMode,
      danmakuBackendMode: _parseBackendMode(sp.getString(_kDanmakuBackend)) ??
          d.danmakuBackendMode,
      autoUpdateEnabled: sp.getBool(_kAutoUpdateEnabled) ?? d.autoUpdateEnabled,
      autoUpdateIntervalHours:
          sp.getInt(_kAutoUpdateIntervalHours) ?? d.autoUpdateIntervalHours,
      kugouBaseUrl: sp.getString(_kKugouBaseUrl) ?? d.kugouBaseUrl,
      neteaseBaseUrls: sp.getString(_kNeteaseBaseUrls) ?? d.neteaseBaseUrls,
      neteaseAnonymousCookieUrl:
          sp.getString(_kNeteaseAnonUrl) ?? d.neteaseAnonymousCookieUrl,
      musicDownloadConcurrency:
          sp.getInt(_kMusicConcurrency) ?? d.musicDownloadConcurrency,
      musicDownloadRetries: sp.getInt(_kMusicRetries) ?? d.musicDownloadRetries,
      musicPathTemplate:
          sp.getString(_kMusicPathTemplate) ?? d.musicPathTemplate,
      qqMusicCookieJson: sp.getString(_kQqCookieJson) ?? d.qqMusicCookieJson,
      pipHideDanmu: sp.getBool(_kPipHideDanmu) ?? d.pipHideDanmu,
      danmuFontSize: sp.getDouble(_kDanmuFontSize) ?? d.danmuFontSize,
      danmuOpacity: sp.getDouble(_kDanmuOpacity) ?? d.danmuOpacity,
      danmuArea: sp.getDouble(_kDanmuArea) ?? d.danmuArea,
      danmuSpeedSeconds:
          sp.getInt(_kDanmuSpeedSeconds) ?? d.danmuSpeedSeconds,
    );
  }

  Future<void> save(AppSettings s) async {
    final sp = await SharedPreferences.getInstance();
    await sp.setString(_kThemeMode, s.themeMode.name);
    await sp.setString(_kBackdropMode, s.backdropMode);
    await sp.setString(_kLiveBackend, s.liveBackendMode.name);
    await sp.setString(_kMusicBackend, s.musicBackendMode.name);
    await sp.setString(_kLyricsBackend, s.lyricsBackendMode.name);
    await sp.setString(_kDanmakuBackend, s.danmakuBackendMode.name);
    await sp.setBool(_kAutoUpdateEnabled, s.autoUpdateEnabled);
    await sp.setInt(_kAutoUpdateIntervalHours, s.autoUpdateIntervalHours);
    if (s.kugouBaseUrl == null || s.kugouBaseUrl!.trim().isEmpty) {
      await sp.remove(_kKugouBaseUrl);
    } else {
      await sp.setString(_kKugouBaseUrl, s.kugouBaseUrl!.trim());
    }
    await sp.setString(_kNeteaseBaseUrls, s.neteaseBaseUrls);
    await sp.setString(_kNeteaseAnonUrl, s.neteaseAnonymousCookieUrl);
    await sp.setInt(_kMusicConcurrency, s.musicDownloadConcurrency);
    await sp.setInt(_kMusicRetries, s.musicDownloadRetries);
    if (s.musicPathTemplate == null || s.musicPathTemplate!.trim().isEmpty) {
      await sp.remove(_kMusicPathTemplate);
    } else {
      await sp.setString(_kMusicPathTemplate, s.musicPathTemplate!);
    }
    if (s.qqMusicCookieJson == null || s.qqMusicCookieJson!.trim().isEmpty) {
      await sp.remove(_kQqCookieJson);
    } else {
      await sp.setString(_kQqCookieJson, s.qqMusicCookieJson!);
    }

    await sp.setBool(_kPipHideDanmu, s.pipHideDanmu);
    await sp.setDouble(_kDanmuFontSize, s.danmuFontSize);
    await sp.setDouble(_kDanmuOpacity, s.danmuOpacity);
    await sp.setDouble(_kDanmuArea, s.danmuArea);
    await sp.setInt(_kDanmuSpeedSeconds, s.danmuSpeedSeconds);
  }

  static BackendMode? _parseBackendMode(String? raw) {
    if (raw == null) return null;
    for (final v in BackendMode.values) {
      if (v.name == raw) return v;
    }
    return null;
  }

  static AppThemeMode? _parseThemeMode(String? raw) {
    if (raw == null) return null;
    for (final v in AppThemeMode.values) {
      if (v.name == raw) return v;
    }
    return null;
  }
}
