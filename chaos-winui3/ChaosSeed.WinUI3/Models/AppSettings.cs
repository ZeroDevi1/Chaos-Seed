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
    public bool LiveDefaultFullscreen { get; set; } = false;
    public int LiveFullscreenDelayMs { get; set; } = 350;
    public bool DebugPlayerOverlay { get; set; } = false;
}
