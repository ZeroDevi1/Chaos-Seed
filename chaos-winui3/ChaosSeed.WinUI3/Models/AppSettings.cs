namespace ChaosSeed.WinUI3.Models;

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

    public bool LyricsAutoDetect { get; set; } = false;
    public string[] LyricsProviders { get; set; } = new[] { "qq", "netease", "lrclib" };
    public int LyricsThreshold { get; set; } = 40;
    public int LyricsLimit { get; set; } = 10;
    public int LyricsTimeoutMs { get; set; } = 8000;

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
}
