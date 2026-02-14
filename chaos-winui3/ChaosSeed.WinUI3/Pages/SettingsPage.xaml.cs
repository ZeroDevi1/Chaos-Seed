using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class SettingsPage : Page
{
    private bool _init;
    private bool _updateBusy;
    private UpdateAvailable? _available;

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

        InitUpdateCardFromSettings(s);
    }

    private void InitUpdateCardFromSettings(AppSettings s)
    {
        UpdateCurrentVersionText.Text = $"当前版本：{UpdateService.GetCurrentVersion()}";
        AutoUpdateToggle.IsOn = s.AutoUpdateEnabled;
        AutoUpdateIntervalBox.Value = Math.Clamp(s.AutoUpdateIntervalHours, 1, 336);

        var packaged = AppIdentityService.IsPackaged;
        UpdatePackagedHint.IsOpen = packaged;
        AutoUpdateToggle.IsEnabled = !packaged;
        AutoUpdateIntervalBox.IsEnabled = !packaged && AutoUpdateToggle.IsOn;
        CheckUpdateButton.IsEnabled = !packaged;

        RefreshUpdateUi(UpdateService.Instance.LastResult);
    }

    private void RefreshUpdateUi(UpdateCheckResult? res)
    {
        UpdateInfoBar.IsOpen = false;
        UpdateNowButton.IsEnabled = false;
        UpdateNowButton.Visibility = Visibility.Collapsed;
        _available = null;

        if (res is null)
        {
            UpdateStatusText.Text = "";
            return;
        }

        switch (res)
        {
            case UpdateUpToDate u:
                UpdateInfoBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Success;
                UpdateInfoBar.Title = "更新";
                UpdateInfoBar.Message = $"已是最新版本（{u.CurrentVersion}）";
                UpdateInfoBar.IsOpen = true;
                UpdateStatusText.Text = "";
                break;
            case UpdateAvailable a:
                _available = a;
                UpdateInfoBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
                UpdateInfoBar.Title = "发现新版本";
                UpdateInfoBar.Message = $"{a.CurrentVersion} → {a.LatestVersion}";
                UpdateNowButton.IsEnabled = true;
                UpdateNowButton.Visibility = Visibility.Visible;
                UpdateInfoBar.IsOpen = true;
                UpdateStatusText.Text = "点击“立即更新”将下载并重启应用完成替换。";
                break;
            case UpdateError e:
                UpdateInfoBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Warning;
                UpdateInfoBar.Title = "更新检查失败";
                UpdateInfoBar.Message = e.Message;
                UpdateInfoBar.IsOpen = true;
                UpdateStatusText.Text = "";
                break;
        }
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

    private void OnAutoUpdateToggled(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init)
        {
            return;
        }

        SettingsService.Instance.Update(s => s.AutoUpdateEnabled = AutoUpdateToggle.IsOn);
        AutoUpdateIntervalBox.IsEnabled = !AppIdentityService.IsPackaged && AutoUpdateToggle.IsOn;
    }

    private void OnAutoUpdateIntervalChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
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

        var hours = (int)Math.Round(v);
        hours = Math.Clamp(hours, 1, 336);
        if (Math.Abs(sender.Value - hours) > 0.0001)
        {
            sender.Value = hours;
        }

        SettingsService.Instance.Update(s => s.AutoUpdateIntervalHours = hours);
    }

    private async void OnCheckUpdateClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init || _updateBusy)
        {
            return;
        }

        _updateBusy = true;
        try
        {
            UpdateProgressBar.Visibility = Visibility.Collapsed;
            UpdateProgressBar.IsIndeterminate = true;
            UpdateStatusText.Text = "正在检查更新…";
            CheckUpdateButton.IsEnabled = false;
            UpdateNowButton.IsEnabled = false;

            var res = await UpdateService.Instance.CheckAsync(force: true);
            RefreshUpdateUi(res);
        }
        catch (Exception ex)
        {
            RefreshUpdateUi(new UpdateError(ex.Message));
        }
        finally
        {
            CheckUpdateButton.IsEnabled = !AppIdentityService.IsPackaged;
            _updateBusy = false;
        }
    }

    private async void OnUpdateNowClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_init || _updateBusy)
        {
            return;
        }
        if (_available is null)
        {
            return;
        }

        _updateBusy = true;
        try
        {
            CheckUpdateButton.IsEnabled = false;
            UpdateNowButton.IsEnabled = false;

            UpdateProgressBar.Visibility = Visibility.Visible;
            UpdateProgressBar.IsIndeterminate = true;
            UpdateProgressBar.Value = 0;
            UpdateStatusText.Text = "正在下载更新包…";

            var prog = new Progress<UpdateProgress>(p =>
            {
                var total = p.TotalBytes;
                if (total is null || total <= 0)
                {
                    UpdateProgressBar.IsIndeterminate = true;
                    UpdateStatusText.Text = $"正在下载更新包…（{FormatBytes(p.BytesDownloaded)}）";
                    return;
                }

                UpdateProgressBar.IsIndeterminate = false;
                var pct = Math.Clamp((double)p.BytesDownloaded / total.Value * 100.0, 0, 100);
                UpdateProgressBar.Value = pct;
                UpdateStatusText.Text =
                    $"正在下载更新包… {pct:0.0}%（{FormatBytes(p.BytesDownloaded)} / {FormatBytes(total.Value)}）";
            });

            var pending = await UpdateService.Instance.DownloadAsync(_available, prog);

            UpdateStatusText.Text = "下载完成，准备重启并替换…";
            await Task.Delay(300);

            UpdateService.Instance.ApplyAndRestart(pending);
        }
        catch (Exception ex)
        {
            UpdateProgressBar.Visibility = Visibility.Collapsed;
            RefreshUpdateUi(new UpdateError(ex.Message));
        }
        finally
        {
            CheckUpdateButton.IsEnabled = !AppIdentityService.IsPackaged;
            _updateBusy = false;
        }
    }

    private static string FormatBytes(long b)
    {
        var x = (double)b;
        string[] units = { "B", "KB", "MB", "GB" };
        var i = 0;
        while (x >= 1024 && i < units.Length - 1)
        {
            x /= 1024;
            i++;
        }
        return $"{x:0.##} {units[i]}";
    }
}
