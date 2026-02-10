using ChaosSeed.WinUI3.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace ChaosSeed.WinUI3.Services;

public static class WindowStyleService
{
    public static void ApplyTheme(Window window, ThemeMode mode)
    {
        if (window.Content is not FrameworkElement root)
        {
            return;
        }

        root.RequestedTheme = mode switch
        {
            ThemeMode.Dark => ElementTheme.Dark,
            ThemeMode.Light => ElementTheme.Light,
            _ => ElementTheme.Default,
        };
    }

    public static void ApplyBackdrop(Window window, BackdropMode mode)
    {
        if (mode == BackdropMode.None)
        {
            window.SystemBackdrop = null;
            return;
        }

        if (!OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000))
        {
            window.SystemBackdrop = null;
            return;
        }

        try
        {
            window.SystemBackdrop = new MicaBackdrop();
        }
        catch
        {
            window.SystemBackdrop = null;
        }
    }
}

