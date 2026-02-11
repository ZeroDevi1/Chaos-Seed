using System.IO;
using FlyleafLib;
using Microsoft.UI.Xaml;

namespace ChaosSeed.WinUI3;

public sealed partial class App : Application
{
    public static MainWindow? MainWindowInstance { get; private set; }
    public static string? FlyleafInitError { get; private set; }

    public App()
    {
        InitializeComponent();
        TryInitFlyleaf();
    }

    private static void TryInitFlyleaf()
    {
        try
        {
            if (Engine.IsLoaded)
            {
                return;
            }

            var ffmpegDir = Path.Combine(AppContext.BaseDirectory, "FFmpeg");
            var cfg = new EngineConfig
            {
                FFmpegPath = ffmpegDir,
#if DEBUG
                FFmpegLogLevel = Flyleaf.FFmpeg.LogLevel.Warn,
                LogLevel = LogLevel.Debug,
                LogOutput = ":debug",
#else
                FFmpegLogLevel = Flyleaf.FFmpeg.LogLevel.Quiet,
                LogLevel = LogLevel.Quiet,
#endif
                UIRefresh = false,
                UIRefreshInterval = 250,
            };

            TrySetBool(cfg, "FFmpegDevices", false);
            TrySetBool(cfg, "UICurTimePerSecond", true);
            TrySetEnum(cfg, "FFmpegLoadProfile", "Filters");

            Engine.Start(cfg);
        }
        catch (Exception ex)
        {
            FlyleafInitError = ex.ToString();
        }
    }

    private static void TrySetBool(object target, string propName, bool value)
    {
        try
        {
            var p = target.GetType().GetProperty(propName);
            if (p?.PropertyType == typeof(bool) && p.CanWrite)
            {
                p.SetValue(target, value);
            }
        }
        catch
        {
            // ignore
        }
    }

    private static void TrySetEnum(object target, string propName, string enumName)
    {
        try
        {
            var p = target.GetType().GetProperty(propName);
            if (p?.CanWrite != true)
            {
                return;
            }
            var t = p.PropertyType;
            if (!t.IsEnum)
            {
                return;
            }
            if (Enum.TryParse(t, enumName, ignoreCase: true, out var v))
            {
                p.SetValue(target, v);
            }
        }
        catch
        {
            // ignore
        }
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        MainWindowInstance = new MainWindow();
        MainWindowInstance.Activate();
    }
}
