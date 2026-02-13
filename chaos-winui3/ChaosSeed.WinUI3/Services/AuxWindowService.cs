using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Windows;

namespace ChaosSeed.WinUI3.Services;

public sealed class AuxWindowService
{
    public static AuxWindowService Instance { get; } = new();

    private DanmakuChatWindow? _chat;
    private NativeDanmakuOverlayWindow? _overlay;

    private AuxWindowService()
    {
        SettingsService.Instance.SettingsChanged += (_, _) => ApplyStyleToOpenWindows();
    }

    public void OpenOrShowChat()
    {
        if (_chat is not null)
        {
            try
            {
                _chat.Activate();
                return;
            }
            catch
            {
                _chat = null;
            }
        }

        var w = new DanmakuChatWindow();
        _chat = w;
        w.Closed += (_, _) => { if (ReferenceEquals(_chat, w)) _chat = null; };
        ApplyStyle(w, allowBackdrop: true);
        w.Activate();
    }

    public void OpenOrShowOverlay()
    {
        if (_overlay is not null)
        {
            try
            {
                _overlay.Show();
                return;
            }
            catch
            {
                _overlay = null;
            }
        }

        // Use a Win32 layered window for "true" transparency on Windows 11.
        // The old WinUI3 overlay window is kept for reference/debugging but isn't the default.
        var w = new NativeDanmakuOverlayWindow();
        _overlay = w;
        w.Closed += (_, _) => { if (ReferenceEquals(_overlay, w)) _overlay = null; };
        w.Show();
    }

    private void ApplyStyleToOpenWindows()
    {
        if (_chat is not null)
        {
            ApplyStyle(_chat, allowBackdrop: true);
        }

        if (_overlay is not null)
        {
            // Native overlay doesn't support theme/backdrop.
        }
    }

    private static void ApplyStyle(Microsoft.UI.Xaml.Window w, bool allowBackdrop)
    {
        var s = SettingsService.Instance.Current;
        WindowStyleService.ApplyTheme(w, s.ThemeMode);
        WindowStyleService.ApplyBackdrop(w, allowBackdrop ? s.BackdropMode : BackdropMode.None);
    }
}
