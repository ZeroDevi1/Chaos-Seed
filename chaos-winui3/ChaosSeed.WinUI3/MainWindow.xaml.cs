using ChaosSeed.WinUI3.Pages;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media.Animation;
using WinRT.Interop;

namespace ChaosSeed.WinUI3;

public sealed partial class MainWindow : Window
{
    private bool _suppressSelectionChanged;
    private AppWindow? _appWindow;
    private readonly Thickness _baseTitleBarPadding = new(12, 0, 12, 0);
    private bool _isSystemFullscreen;
    private Models.ThemeMode? _appliedThemeMode;
    private Models.BackdropMode? _appliedBackdropMode;

    public MainWindow()
    {
        InitializeComponent();

        InitTitleBar();
        ApplyWindowStyleFromSettings();
        SettingsService.Instance.SettingsChanged += (_, _) => ApplyWindowStyleFromSettings();

        Nav.SelectedItem = Nav.MenuItems[0];
        ContentFrame.Navigate(typeof(LivePage), null, new DrillInNavigationTransitionInfo());
    }

    private void InitTitleBar()
    {
        try
        {
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);
        }
        catch
        {
            // ignore - if title bar APIs are unavailable, fall back to default.
        }

        try
        {
            var hwnd = WindowNative.GetWindowHandle(this);
            var id = Win32Interop.GetWindowIdFromWindow(hwnd);
            _appWindow = AppWindow.GetFromWindowId(id);
        }
        catch
        {
            _appWindow = null;
        }

        if (_appWindow is null)
        {
            return;
        }

        try
        {
            var tb = _appWindow.TitleBar;
            tb.ExtendsContentIntoTitleBar = true;
            tb.ButtonBackgroundColor = Colors.Transparent;
            tb.ButtonInactiveBackgroundColor = Colors.Transparent;
        }
        catch
        {
            // ignore
        }

        AppTitleBar.Loaded += (_, _) => UpdateTitleBarPadding();
        AppTitleBar.SizeChanged += (_, _) => UpdateTitleBarPadding();
    }

    private void UpdateTitleBarPadding()
    {
        if (_appWindow is null)
        {
            AppTitleBar.Padding = _baseTitleBarPadding;
            return;
        }

        try
        {
            var tb = _appWindow.TitleBar;
            AppTitleBar.Padding = new Thickness(
                _baseTitleBarPadding.Left + tb.LeftInset,
                _baseTitleBarPadding.Top,
                _baseTitleBarPadding.Right + tb.RightInset,
                _baseTitleBarPadding.Bottom
            );
        }
        catch
        {
            AppTitleBar.Padding = _baseTitleBarPadding;
        }
    }

    private void ApplyWindowStyleFromSettings()
    {
        var s = SettingsService.Instance.Current;
        if (_appliedThemeMode != s.ThemeMode)
        {
            WindowStyleService.ApplyTheme(this, s.ThemeMode);
            _appliedThemeMode = s.ThemeMode;
        }

        if (_appliedBackdropMode != s.BackdropMode)
        {
            WindowStyleService.ApplyBackdrop(this, s.BackdropMode);
            _appliedBackdropMode = s.BackdropMode;
        }
    }

    private void OnSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (_suppressSelectionChanged)
        {
            return;
        }

        var wasPaneOpen = sender.IsPaneOpen;
        if (args.SelectedItem is not NavigationViewItem item)
        {
            return;
        }

        switch (item.Tag as string)
        {
            case "live":
                ContentFrame.Navigate(typeof(LivePage), null, new DrillInNavigationTransitionInfo());
                break;
            case "lyrics":
                ContentFrame.Navigate(typeof(LyricsPage), null, new DrillInNavigationTransitionInfo());
                break;
            case "settings":
                ContentFrame.Navigate(typeof(SettingsPage), null, new DrillInNavigationTransitionInfo());
                break;
        }

        if (wasPaneOpen)
        {
            // In some display modes NavigationView auto-collapses after selection; keep it open until user closes it.
            DispatcherQueue.GetForCurrentThread().TryEnqueue(() => sender.IsPaneOpen = true);
        }
    }

    public void NavigateToLive(string input)
    {
        _suppressSelectionChanged = true;
        try
        {
            Nav.SelectedItem = FindNavItemByTag("live") ?? Nav.MenuItems[0];
            ContentFrame.Navigate(typeof(LivePage), input, new DrillInNavigationTransitionInfo());
        }
        finally
        {
            _suppressSelectionChanged = false;
        }
    }

    private NavigationViewItem? FindNavItemByTag(string tag)
    {
        foreach (var x in Nav.MenuItems)
        {
            if (x is NavigationViewItem nvi && string.Equals(nvi.Tag as string, tag, StringComparison.Ordinal))
            {
                return nvi;
            }
        }
        foreach (var x in Nav.FooterMenuItems)
        {
            if (x is NavigationViewItem nvi && string.Equals(nvi.Tag as string, tag, StringComparison.Ordinal))
            {
                return nvi;
            }
        }
        return null;
    }

    public bool TrySetSystemFullscreen(bool fullscreen)
    {
        if (_appWindow is null)
        {
            return false;
        }

        try
        {
            if (fullscreen)
            {
                _appWindow.SetPresenter(AppWindowPresenterKind.FullScreen);
            }
            else
            {
                _appWindow.SetPresenter(AppWindowPresenterKind.Overlapped);
            }

            _isSystemFullscreen = fullscreen;
            return true;
        }
        catch
        {
            return false;
        }
    }

    public bool IsSystemFullscreen => _isSystemFullscreen;

    public FrameworkElement TitleBarElement => AppTitleBar;
    public NavigationView NavigationElement => Nav;

    public Popup FullScreenPopupElement => FullScreenPopup;
    public Grid FullScreenPopupRootElement => FullScreenPopupRoot;
    public Grid FullScreenBackdropElement => FullScreenBackdrop;
    public ContentControl FullScreenPlayerHostElement => FullScreenPlayerHost;
}
