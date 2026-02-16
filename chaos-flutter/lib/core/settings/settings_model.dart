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

  // Player (Android).
  final bool pipHideDanmu;
  final double danmuFontSize;
  final double danmuOpacity;
  final double danmuArea;
  final int danmuSpeedSeconds;

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
    required this.pipHideDanmu,
    required this.danmuFontSize,
    required this.danmuOpacity,
    required this.danmuArea,
    required this.danmuSpeedSeconds,
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
      // 注意：WinUI3 的 XAML 里常用前缀 "{}" 来转义花括号（避免被当成 MarkupExtension）。
      // Flutter/Rust 模板不需要该转义，因此这里不要带 "{}"。
      musicPathTemplate: '{{artist}}/{{album}}/{{title}} - {{artist}}.{{ext}}',
      qqMusicCookieJson: null,
      pipHideDanmu: true,
      danmuFontSize: 18,
      danmuOpacity: 0.85,
      danmuArea: 0.6,
      danmuSpeedSeconds: 8,
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
    bool? pipHideDanmu,
    double? danmuFontSize,
    double? danmuOpacity,
    double? danmuArea,
    int? danmuSpeedSeconds,
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
      pipHideDanmu: pipHideDanmu ?? this.pipHideDanmu,
      danmuFontSize: danmuFontSize ?? this.danmuFontSize,
      danmuOpacity: danmuOpacity ?? this.danmuOpacity,
      danmuArea: danmuArea ?? this.danmuArea,
      danmuSpeedSeconds: danmuSpeedSeconds ?? this.danmuSpeedSeconds,
    );
  }
}
