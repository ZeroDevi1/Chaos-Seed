using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Windows;

namespace ChaosSeed.WinUI3.Services;

public sealed class AuxWindowService
{
    public static AuxWindowService Instance { get; } = new();

    private DanmakuChatWindow? _chat;
    private DanmakuOverlayWindow? _overlay;

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
                _overlay.Activate();
                return;
            }
            catch
            {
                _overlay = null;
            }
        }

        var w = new DanmakuOverlayWindow();
        _overlay = w;
        w.Closed += (_, _) => { if (ReferenceEquals(_overlay, w)) _overlay = null; };
        ApplyStyle(w, allowBackdrop: false);
        w.Activate();
    }

    private void ApplyStyleToOpenWindows()
    {
        if (_chat is not null)
        {
            ApplyStyle(_chat, allowBackdrop: true);
        }

        if (_overlay is not null)
        {
            ApplyStyle(_overlay, allowBackdrop: false);
        }
    }

    private static void ApplyStyle(Microsoft.UI.Xaml.Window w, bool allowBackdrop)
    {
        var s = SettingsService.Instance.Current;
        WindowStyleService.ApplyTheme(w, s.ThemeMode);
        WindowStyleService.ApplyBackdrop(w, allowBackdrop ? s.BackdropMode : BackdropMode.None);
    }
}

