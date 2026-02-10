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
}

public enum PlayerEngine
{
    Vlc = 0,
    System = 1,
}

public sealed class AppSettings
{
    public ThemeMode ThemeMode { get; set; } = ThemeMode.FollowSystem;
    public BackdropMode BackdropMode { get; set; } = BackdropMode.Mica;
    public PlayerEngine PlayerEngine { get; set; } = PlayerEngine.Vlc;
}

