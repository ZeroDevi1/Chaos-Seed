enum BackendMode {
  auto,
  ffi,
  daemon,
}

enum AppThemeMode {
  system,
  dark,
  light,
}

class AppSettings {
  static const Object _unset = Object();

  final AppThemeMode themeMode;

  // For Windows: "mica", "micaAlt", "none"
  final String backdropMode;

  // Backend selection (Windows). Android always uses FFI.
  final BackendMode liveBackendMode;
  final BackendMode musicBackendMode;
  final BackendMode lyricsBackendMode;
  final BackendMode danmakuBackendMode;

  // Update.
  final bool autoUpdateEnabled;
  final int autoUpdateIntervalHours;

  // Music config defaults (mirrors WinUI3).
  final String? kugouBaseUrl;
  final String neteaseBaseUrls; // ';' separated
  final String neteaseAnonymousCookieUrl;

  // Music download preferences (minimal subset).
  final int musicDownloadConcurrency;
  final int musicDownloadRetries;
  final String? musicPathTemplate;

  // Auth cache.
  // Android 端用于 QQ 音乐扫码登录后缓存 cookie（JSON 字符串）。
  final String? qqMusicCookieJson;

  const AppSettings({
    required this.themeMode,
    required this.backdropMode,
    required this.liveBackendMode,
    required this.musicBackendMode,
    required this.lyricsBackendMode,
    required this.danmakuBackendMode,
    required this.autoUpdateEnabled,
    required this.autoUpdateIntervalHours,
    required this.kugouBaseUrl,
    required this.neteaseBaseUrls,
    required this.neteaseAnonymousCookieUrl,
    required this.musicDownloadConcurrency,
    required this.musicDownloadRetries,
    required this.musicPathTemplate,
    required this.qqMusicCookieJson,
  });

  factory AppSettings.defaults() {
    return const AppSettings(
      themeMode: AppThemeMode.system,
      backdropMode: 'mica',
      liveBackendMode: BackendMode.auto,
      musicBackendMode: BackendMode.auto,
      lyricsBackendMode: BackendMode.auto,
      danmakuBackendMode: BackendMode.auto,
      autoUpdateEnabled: true,
      autoUpdateIntervalHours: 24,
      kugouBaseUrl: null,
      neteaseBaseUrls:
          'http://plugin.changsheng.space:3000;https://wyy.xhily.com;http://111.229.38.178:3333;http://dg-t.cn:3000;https://zm.armoe.cn',
      neteaseAnonymousCookieUrl: '/register/anonimous',
      musicDownloadConcurrency: 3,
      musicDownloadRetries: 2,
      musicPathTemplate:
          '{}{{artist}}/{{album}}/{{title}} - {{artist}}.{{ext}}',
      qqMusicCookieJson: null,
    );
  }

  AppSettings copyWith({
    AppThemeMode? themeMode,
    String? backdropMode,
    BackendMode? liveBackendMode,
    BackendMode? musicBackendMode,
    BackendMode? lyricsBackendMode,
    BackendMode? danmakuBackendMode,
    bool? autoUpdateEnabled,
    int? autoUpdateIntervalHours,
    String? kugouBaseUrl,
    String? neteaseBaseUrls,
    String? neteaseAnonymousCookieUrl,
    int? musicDownloadConcurrency,
    int? musicDownloadRetries,
    Object? musicPathTemplate = _unset,
    Object? qqMusicCookieJson = _unset,
  }) {
    return AppSettings(
      themeMode: themeMode ?? this.themeMode,
      backdropMode: backdropMode ?? this.backdropMode,
      liveBackendMode: liveBackendMode ?? this.liveBackendMode,
      musicBackendMode: musicBackendMode ?? this.musicBackendMode,
      lyricsBackendMode: lyricsBackendMode ?? this.lyricsBackendMode,
      danmakuBackendMode: danmakuBackendMode ?? this.danmakuBackendMode,
      autoUpdateEnabled: autoUpdateEnabled ?? this.autoUpdateEnabled,
      autoUpdateIntervalHours:
          autoUpdateIntervalHours ?? this.autoUpdateIntervalHours,
      kugouBaseUrl: kugouBaseUrl ?? this.kugouBaseUrl,
      neteaseBaseUrls: neteaseBaseUrls ?? this.neteaseBaseUrls,
      neteaseAnonymousCookieUrl:
          neteaseAnonymousCookieUrl ?? this.neteaseAnonymousCookieUrl,
      musicDownloadConcurrency:
          musicDownloadConcurrency ?? this.musicDownloadConcurrency,
      musicDownloadRetries: musicDownloadRetries ?? this.musicDownloadRetries,
      musicPathTemplate: musicPathTemplate == _unset
          ? this.musicPathTemplate
          : musicPathTemplate as String?,
      qqMusicCookieJson: qqMusicCookieJson == _unset
          ? this.qqMusicCookieJson
          : qqMusicCookieJson as String?,
    );
  }
}
