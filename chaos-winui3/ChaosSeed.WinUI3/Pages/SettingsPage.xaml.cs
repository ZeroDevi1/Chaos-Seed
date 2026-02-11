using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
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

        var win11 = OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000);
        BackdropCombo.IsEnabled = win11;
        BackdropHint.IsOpen = !win11;
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
}
