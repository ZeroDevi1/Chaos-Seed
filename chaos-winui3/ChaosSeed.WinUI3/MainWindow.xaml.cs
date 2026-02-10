using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Pages;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3;

public sealed partial class MainWindow : Window
{
    private bool _suppressSelectionChanged;

    public MainWindow()
    {
        InitializeComponent();

        ApplyWindowStyleFromSettings();
        SettingsService.Instance.SettingsChanged += (_, _) => ApplyWindowStyleFromSettings();

        Nav.SelectedItem = Nav.MenuItems[0];
        ContentFrame.Navigate(typeof(HomePage));
    }

    private void ApplyWindowStyleFromSettings()
    {
        var s = SettingsService.Instance.Current;
        WindowStyleService.ApplyTheme(this, s.ThemeMode);
        WindowStyleService.ApplyBackdrop(this, s.BackdropMode);
    }

    private void OnSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (_suppressSelectionChanged)
        {
            return;
        }

        if (args.SelectedItem is not NavigationViewItem item)
        {
            return;
        }

        switch (item.Tag as string)
        {
            case "home":
                ContentFrame.Navigate(typeof(HomePage));
                break;
            case "live":
                ContentFrame.Navigate(typeof(LivePage));
                break;
            case "settings":
                ContentFrame.Navigate(typeof(SettingsPage));
                break;
        }
    }

    public void NavigateToLive(LiveOpenResult result)
    {
        _suppressSelectionChanged = true;
        try
        {
            Nav.SelectedItem = Nav.MenuItems[1];
            ContentFrame.Navigate(typeof(LivePage), result);
        }
        finally
        {
            _suppressSelectionChanged = false;
        }
    }
}
