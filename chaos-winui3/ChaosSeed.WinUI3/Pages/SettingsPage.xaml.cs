using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class SettingsPage : Page
{
    private bool _init;

    public SettingsPage()
    {
        InitializeComponent();
        Loaded += (_, _) => InitFromSettings();
    }

    private void InitFromSettings()
    {
        if (_init)
        {
            return;
        }
        _init = true;

        var s = SettingsService.Instance.Current;

        ThemeCombo.SelectedIndex = s.ThemeMode switch
        {
            ThemeMode.Dark => 1,
            ThemeMode.Light => 2,
            _ => 0,
        };

        BackdropCombo.SelectedIndex = s.BackdropMode switch
        {
            BackdropMode.MicaAlt => 1,
            BackdropMode.None => 2,
            _ => 0, // Mica
        };

        LiveBackendCombo.SelectedIndex = s.LiveBackendMode switch
        {
            LiveBackendMode.Ffi => 1,
            LiveBackendMode.Daemon => 2,
            _ => 0, // Auto
        };

        LyricsBackendCombo.SelectedIndex = s.LyricsBackendMode switch
        {
            LiveBackendMode.Ffi => 1,
            LiveBackendMode.Daemon => 2,
            _ => 0, // Auto
        };

        DanmakuBackendCombo.SelectedIndex = s.DanmakuBackendMode switch
        {
            LiveBackendMode.Ffi => 1,
            LiveBackendMode.Daemon => 2,
            _ => 0, // Auto
        };

        LyricsAutoDetectToggle.IsOn = s.LyricsAutoDetect;

        LiveDefaultFullscreenToggle.IsOn = s.LiveDefaultFullscreen;
        LiveFullscreenAnimRateBox.Value = Math.Clamp(s.LiveFullscreenAnimRate, 0.25, 2.5);
        DebugPlayerToggle.IsOn = s.DebugPlayerOverlay;

        var win11 = OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000);
        BackdropCombo.IsEnabled = win11;
        BackdropHint.IsOpen = !win11;

        MusicKugouBaseUrlBox.Text = s.KugouBaseUrl ?? "";
        MusicNeteaseBaseUrlsBox.Text = s.NeteaseBaseUrls ?? "";
        MusicNeteaseAnonUrlBox.Text = s.NeteaseAnonymousCookieUrl ?? "/register/anonimous";
        MusicAskOutDirToggle.IsOn = s.MusicAskOutDirEachTime;
        MusicPathTemplateBox.Text = string.IsNullOrWhiteSpace(s.MusicPathTemplate)
            ? new AppSettings().MusicPathTemplate
            : s.MusicPathTemplate;
    }

    private void OnThemeChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_init)
        {
            return;
        }
        if (ThemeCombo.SelectedItem is not ComboBoxItem item || item.Tag is not string tag)
        {
            return;
        }

        var mode = tag switch
        {
            "Dark" => ThemeMode.Dark,
            "Light" => ThemeMode.Light,
            _ => ThemeMode.FollowSystem,
        };
        SettingsService.Instance.Update(s => s.ThemeMode = mode);
    }

    private void OnBackdropChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_init)
        {
            return;
        }
        if (BackdropCombo.SelectedItem is not ComboBoxItem item || item.Tag is not string tag)
        {
            return;
        }

        var mode = tag switch
        {
            "None" => BackdropMode.None,
            "MicaAlt" => BackdropMode.MicaAlt,
            _ => BackdropMode.Mica,
        };
        SettingsService.Instance.Update(s => s.BackdropMode = mode);
    }

    private void OnLiveBackendChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_init)
        {
            return;
        }
        if (LiveBackendCombo.SelectedItem is not ComboBoxItem item || item.Tag is not string tag)
        {
            return;
        }

        var mode = tag switch
        {
            "Ffi" => LiveBackendMode.Ffi,
            "Daemon" => LiveBackendMode.Daemon,
            _ => LiveBackendMode.Auto,
        };
        SettingsService.Instance.Update(s => s.LiveBackendMode = mode);
    }

    private void OnLyricsBackendChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_init)
        {
            return;
        }
        if (LyricsBackendCombo.SelectedItem is not ComboBoxItem item || item.Tag is not string tag)
        {
            return;
        }

        var mode = tag switch
        {
            "Ffi" => LiveBackendMode.Ffi,
            "Daemon" => LiveBackendMode.Daemon,
            _ => LiveBackendMode.Auto,
        };
        SettingsService.Instance.Update(s => s.LyricsBackendMode = mode);
    }

    private void OnDanmakuBackendChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_init)
        {
            return;
        }
        if (DanmakuBackendCombo.SelectedItem is not ComboBoxItem item || item.Tag is not string tag)
        {
            return;
        }

        var mode = tag switch
        {
            "Ffi" => LiveBackendMode.Ffi,
            "Daemon" => LiveBackendMode.Daemon,
            _ => LiveBackendMode.Auto,
        };
        SettingsService.Instance.Update(s => s.DanmakuBackendMode = mode);
    }

    private void OnLyricsAutoDetectToggled(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init)
        {
            return;
        }

        SettingsService.Instance.Update(s => s.LyricsAutoDetect = LyricsAutoDetectToggle.IsOn);
    }

    private void OnLiveDefaultFullscreenToggled(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (!_init)
        {
            return;
        }

        SettingsService.Instance.Update(s => s.LiveDefaultFullscreen = LiveDefaultFullscreenToggle.IsOn);
    }

    private void OnLiveFullscreenAnimRateChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        _ = args;
        if (!_init)
        {
            return;
        }

        var v = sender.Value;
        if (double.IsNaN(v) || double.IsInfinity(v))
        {
            return;
        }

        v = Math.Round(v, 2);
        v = Math.Clamp(v, 0.25, 2.5);
        SettingsService.Instance.Update(s => s.LiveFullscreenAnimRate = v);
    }

    private void OnDebugPlayerToggled(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (!_init)
        {
            return;
        }

        SettingsService.Instance.Update(s => s.DebugPlayerOverlay = DebugPlayerToggle.IsOn);
    }

    private void OnMusicKugouBaseUrlLostFocus(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init)
        {
            return;
        }

        var v = (MusicKugouBaseUrlBox.Text ?? "").Trim().TrimEnd('/');
        SettingsService.Instance.Update(s => s.KugouBaseUrl = string.IsNullOrWhiteSpace(v) ? null : v);
    }

    private void OnMusicNeteaseBaseUrlsLostFocus(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init)
        {
            return;
        }

        var v = (MusicNeteaseBaseUrlsBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(v))
        {
            v = new AppSettings().NeteaseBaseUrls ?? "";
            MusicNeteaseBaseUrlsBox.Text = v;
        }
        SettingsService.Instance.Update(s => s.NeteaseBaseUrls = v);
    }

    private void OnMusicNeteaseAnonUrlLostFocus(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init)
        {
            return;
        }

        var v = (MusicNeteaseAnonUrlBox.Text ?? "").Trim();
        SettingsService.Instance.Update(s => s.NeteaseAnonymousCookieUrl = string.IsNullOrWhiteSpace(v) ? "/register/anonimous" : v);
    }

    private void OnMusicAskOutDirToggled(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init)
        {
            return;
        }

        SettingsService.Instance.Update(s => s.MusicAskOutDirEachTime = MusicAskOutDirToggle.IsOn);
    }

    private void OnMusicPathTemplateLostFocus(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init)
        {
            return;
        }

        var v = (MusicPathTemplateBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(v))
        {
            v = new AppSettings().MusicPathTemplate;
            MusicPathTemplateBox.Text = v;
        }
        SettingsService.Instance.Update(s => s.MusicPathTemplate = v);
    }
}
