using System.IO;
using System.Diagnostics;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Threading;
using FlyleafLib;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;

namespace ChaosSeed.WinUI3;

public sealed partial class App : Application
{
    public static MainWindow? MainWindowInstance { get; private set; }
    public static string? FlyleafInitError { get; private set; }
#if DEBUG
    private static int _comFirstChanceLogged;
#endif

    public App()
    {
        InitializeComponent();
#if DEBUG
        AppDomain.CurrentDomain.FirstChanceException += OnFirstChanceException;
#endif
        TryInitFlyleaf();
    }

#if DEBUG
    private static void OnFirstChanceException(object? sender, FirstChanceExceptionEventArgs e)
    {
        if (e.Exception is not COMException com)
        {
            return;
        }

        var idx = Interlocked.Increment(ref _comFirstChanceLogged);
        if (idx > 10)
        {
            return;
        }

        try
        {
            Debug.WriteLine(
                $"[COM#{idx}] HRESULT=0x{com.HResult:X8} {com.Message}\n{com.StackTrace}\n"
            );
        }
        catch
        {
            // ignore
        }
    }
#endif

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
                LogLevel = LogLevel.Debug,
                LogOutput = ":debug",
#else
                LogLevel = LogLevel.Quiet,
#endif
            };

            Engine.Start(cfg);
        }
        catch (Exception ex)
        {
            FlyleafInitError = ex.ToString();
        }
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        MainWindowInstance = new MainWindow();
        MainWindowInstance.Activate();

        // Best-effort: background update check for zip (unpackaged) builds.
        _ = UpdateService.Instance.TryAutoCheckAsync();
    }
}
