namespace ChaosSeed.WinUI3.Models;

using ChaosSeed.WinUI3.Models.Music;

public enum ThemeMode
{
    FollowSystem = 0,
    Dark = 1,
    Light = 2,
}

public enum BackdropMode
{
    Mica = 0,
    None = 1,
    MicaAlt = 2,
}

public enum LiveBackendMode
{
    Auto = 0,
    Ffi = 1,
    Daemon = 2,
}

public sealed class AppSettings
{
    public ThemeMode ThemeMode { get; set; } = ThemeMode.FollowSystem;
    public BackdropMode BackdropMode { get; set; } = BackdropMode.Mica;
    public LiveBackendMode LiveBackendMode { get; set; } = LiveBackendMode.Auto;
    public LiveBackendMode LyricsBackendMode { get; set; } = LiveBackendMode.Auto;
    public LiveBackendMode DanmakuBackendMode { get; set; } = LiveBackendMode.Auto;
    public LiveBackendMode MusicBackendMode { get; set; } = LiveBackendMode.Auto;

    public bool LyricsAutoDetect { get; set; } = false;
    public string[] LyricsProviders { get; set; } = new[] { "qq", "netease", "lrclib" };
    public int LyricsThreshold { get; set; } = 40;
    public int LyricsLimit { get; set; } = 10;
    public int LyricsTimeoutMs { get; set; } = 8000;
    public string? LyricsPreferredAppId { get; set; }

    public int? DanmakuOverlayX { get; set; }
    public int? DanmakuOverlayY { get; set; }
    public int? DanmakuOverlayWidth { get; set; }
    public int? DanmakuOverlayHeight { get; set; }

    public int? DanmakuChatX { get; set; }
    public int? DanmakuChatY { get; set; }
    public int? DanmakuChatWidth { get; set; }
    public int? DanmakuChatHeight { get; set; }

    // Live player overlay danmaku settings (Bilibili-like overlay).
    public bool DanmakuOverlayEnabled { get; set; } = true;
    public double DanmakuOverlayOpacity { get; set; } = 1.0; // 0..1
    public double DanmakuOverlayFontScale { get; set; } = 1.0; // 0.5..2
    public double DanmakuOverlayDensity { get; set; } = 1.0; // 0..1
    public DanmakuOverlayAreaMode DanmakuOverlayArea { get; set; } = DanmakuOverlayAreaMode.Full;

    public bool LiveDefaultFullscreen { get; set; } = false;
    public double LiveFullscreenAnimRate { get; set; } = 1.0;
    public bool DebugPlayerOverlay { get; set; } = false;

    // ----- music -----

    // ProviderConfig
    public string? KugouBaseUrl { get; set; }
    // Built-in defaults so users can use Netease search/download without manual configuration.
    // Users can still override in Settings if needed.
    public string? NeteaseBaseUrls { get; set; } =
        "http://plugin.changsheng.space:3000;https://wyy.xhily.com;http://111.229.38.178:3333;http://dg-t.cn:3000;https://zm.armoe.cn"; // ';' separated
    public string? NeteaseAnonymousCookieUrl { get; set; } = "/register/anonimous";

    // Auth (persisted in WinUI settings; daemon/ffi does not persist)
    public QqMusicCookie? QqMusicCookie { get; set; }
    public KugouUserInfo? KugouUserInfo { get; set; }

    // Download preferences
    public string? MusicLastOutDir { get; set; }
    public bool MusicAskOutDirEachTime { get; set; } = true;
    public string MusicPathTemplate { get; set; } = "{{artist}}/{{album}}/{{title}} - {{artist}}.{{ext}}";
    public int MusicDownloadConcurrency { get; set; } = 3;
    public int MusicDownloadRetries { get; set; } = 2;
    public bool MusicDownloadOverwrite { get; set; } = false;

    // ----- updates (WinUI3 only; zip self-updater) -----

    public bool AutoUpdateEnabled { get; set; } = true;
    public int AutoUpdateIntervalHours { get; set; } = 24;
    public long? AutoUpdateLastCheckUnixMs { get; set; }
    public string? AutoUpdateIgnoredVersion { get; set; }
}
