using System.Collections.ObjectModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using ChaosSeed.WinUI3.Services.LiveDirectoryBackends;
using ChaosSeed.WinUI3.Models;
namespace ChaosSeed.WinUI3.Pages;

public sealed partial class HomePage : Page
{
    public ObservableCollection<HomeRoomCardVm> RoomCards { get; } = new();

    private ILiveDirectoryBackend _backend;
    private string _site = "bili_live";
    private bool _isSearching;
    private string _keyword = "";
    private int _page = 1;
    private bool _hasMore = true;
    private bool _isLoading;
    private CancellationTokenSource? _cts;
    private ScrollViewer? _roomScrollViewer;
    private ItemsWrapGrid? _roomWrapGrid;

    public HomePage()
    {
        InitializeComponent();
        _backend = LiveDirectoryBackendFactory.Create();
        PlatformNav.SelectedItem = PlatformNav.MenuItems[0];
        if (!string.IsNullOrWhiteSpace(_backend.InitNotice))
        {
            ShowInfo(_backend.InitNotice!);
        }
        _ = ReloadAsync();
    }

    protected override void OnNavigatedFrom(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        try { _cts?.Cancel(); } catch { }
        try
        {
            if (_roomScrollViewer is not null)
            {
                _roomScrollViewer.ViewChanged -= OnRoomScrollViewChanged;
            }
        }
        catch
        {
            // ignore
        }
    }

    private void ShowInfo(string msg)
    {
        StatusBar.Severity = InfoBarSeverity.Informational;
        StatusBar.Title = "提示";
        StatusBar.Message = msg;
        StatusBar.IsOpen = true;
    }

    private void ShowError(string msg)
    {
        StatusBar.Severity = InfoBarSeverity.Error;
        StatusBar.Title = "失败";
        StatusBar.Message = msg;
        StatusBar.IsOpen = true;
    }

    private async Task ReloadAsync()
    {
        RoomCards.Clear();
        _page = 1;
        _hasMore = true;
        SetCenterState(loading: true, text: "加载中...", showRetry: false);
        await LoadNextPageAsync();
    }

    private async Task LoadNextPageAsync()
    {
        if (_isLoading || !_hasMore)
        {
            return;
        }

        _isLoading = true;
        LoadingRing.IsActive = true;
        LoadMoreBtn.IsEnabled = false;
        if (RoomCards.Count == 0)
        {
            SetCenterState(loading: true, text: "加载中...", showRetry: false);
        }

        _cts?.Cancel();
        _cts?.Dispose();
        _cts = new CancellationTokenSource(TimeSpan.FromSeconds(15));
        var ct = _cts.Token;

        try
        {
            LiveDirRoomListResult res = _isSearching
                ? await _backend.SearchRoomsAsync(_site, _keyword, _page, ct)
                : await _backend.GetRecommendRoomsAsync(_site, _page, ct);

            var items = res.Items ?? Array.Empty<LiveDirRoomCard>();
            foreach (var x in items)
            {
                RoomCards.Add(HomeRoomCardVm.From(x));
            }

            _hasMore = res.HasMore && items.Length > 0;
            if (_hasMore)
            {
                _page += 1;
            }
            StatusBar.IsOpen = false;
        }
        catch (OperationCanceledException)
        {
            ShowError("加载超时/已取消，请重试。");
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
        finally
        {
            _isLoading = false;
            LoadingRing.IsActive = false;
            LoadMoreBtn.IsEnabled = _hasMore;

            if (RoomCards.Count == 0)
            {
                // "Empty but not an error" still needs user-visible feedback.
                SetCenterState(loading: false, text: "暂无数据（可尝试重试或切换平台）", showRetry: true);
            }
            else
            {
                SetCenterState(loading: false, text: "", showRetry: false);
            }
        }
    }

    private void SetCenterState(bool loading, string text, bool showRetry)
    {
        try
        {
            CenterStatePanel.Visibility = (loading || showRetry || !string.IsNullOrWhiteSpace(text))
                ? Visibility.Visible
                : Visibility.Collapsed;
            CenterLoadingRing.IsActive = loading;
            CenterStateText.Text = text;
            RetryBtn.Visibility = showRetry ? Visibility.Visible : Visibility.Collapsed;
        }
        catch
        {
            // ignore (XAML not ready)
        }
    }

    private void OnRoomGridLoaded(object sender, RoutedEventArgs e)
    {
        _ = e;
        if (_roomScrollViewer is not null)
        {
            return;
        }

        try
        {
            _roomScrollViewer = FindDescendant<ScrollViewer>(RoomGrid);
            if (_roomScrollViewer is not null)
            {
                _roomScrollViewer.ViewChanged += OnRoomScrollViewChanged;
            }
        }
        catch
        {
            // ignore
        }

        // Ensure we capture the panel after it gets materialized.
        TryUpdateRoomWrapGrid();
        UpdateRoomItemWidth(RoomGrid.ActualWidth);
    }

    private void OnRoomGridSizeChanged(object sender, SizeChangedEventArgs e)
    {
        _ = sender;
        TryUpdateRoomWrapGrid();
        UpdateRoomItemWidth(e.NewSize.Width);
    }

    private void TryUpdateRoomWrapGrid()
    {
        if (_roomWrapGrid is not null)
        {
            return;
        }
        try
        {
            _roomWrapGrid = RoomGrid.ItemsPanelRoot as ItemsWrapGrid;
        }
        catch
        {
            // ignore
        }
    }

    private void UpdateRoomItemWidth(double width)
    {
        var panel = _roomWrapGrid;
        if (panel is null || width <= 0)
        {
            return;
        }

        const double gap = 12; // matches ChaosCardGridViewItemStyle margin
        const double min = 240;
        const double max = 360;
        var avail = Math.Max(0, width - 24); // keep a bit of breathing room for scroll bar/padding
        var cols = (int)Math.Floor((avail + gap) / (min + gap));
        if (cols < 1)
        {
            cols = 1;
        }
        var item = Math.Floor((avail + gap) / cols) - gap;
        item = Math.Clamp(item, min, max);

        // Avoid re-layout churn.
        if (Math.Abs(panel.ItemWidth - item) > 0.5)
        {
            panel.ItemWidth = item;
        }
    }

    private async void OnRoomScrollViewChanged(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        _ = sender;
        // Only trigger on user scroll end to avoid excessive calls.
        if (e.IsIntermediate)
        {
            return;
        }

        var sv = _roomScrollViewer;
        if (sv is null)
        {
            return;
        }

        // Near bottom => load more.
        if (sv.ScrollableHeight > 0 && (sv.ScrollableHeight - sv.VerticalOffset) < 480)
        {
            await LoadNextPageAsync();
        }
    }

    private static T? FindDescendant<T>(DependencyObject root) where T : DependencyObject
    {
        var n = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < n; i++)
        {
            var child = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(root, i);
            if (child is T t)
            {
                return t;
            }
            var found = FindDescendant<T>(child);
            if (found is not null)
            {
                return found;
            }
        }
        return null;
    }

    private async void OnPlatformNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        _ = sender;
        if (args.SelectedItem is not NavigationViewItem item || item.Tag is not string tag)
        {
            return;
        }

        _site = tag.Trim();
        _isSearching = false;
        _keyword = "";
        SearchBox.Text = "";
        await ReloadAsync();
    }

    private async void OnSearchClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        var kw = (SearchBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(kw))
        {
            return;
        }
        _isSearching = true;
        _keyword = kw;
        await ReloadAsync();
    }

    private async void OnClearSearchClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_isSearching && string.IsNullOrWhiteSpace(SearchBox.Text))
        {
            return;
        }
        _isSearching = false;
        _keyword = "";
        SearchBox.Text = "";
        await ReloadAsync();
    }

    private void OnSearchKeyDown(object sender, KeyRoutedEventArgs e)
    {
        _ = sender;
        if (e.Key == global::Windows.System.VirtualKey.Enter)
        {
            OnSearchClicked(this, new RoutedEventArgs());
        }
    }

    private async void OnRetryClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        await ReloadAsync();
    }

    private async void OnLoadMoreClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        await LoadNextPageAsync();
    }

    private void OnRoomItemClick(object sender, ItemClickEventArgs e)
    {
        _ = sender;
        if (e.ClickedItem is not HomeRoomCardVm vm)
        {
            return;
        }
        if (string.IsNullOrWhiteSpace(vm.Input))
        {
            return;
        }
        App.MainWindowInstance?.NavigateToLive(vm.Input);
    }
}

public sealed class HomeRoomCardVm
{
    public string Input { get; set; } = "";
    public string Title { get; set; } = "";
    public string Streamer { get; set; } = "";
    public string OnlineText { get; set; } = "";
    public Visibility OnlineVisibility { get; set; } = Visibility.Collapsed;
    public BitmapImage? Cover { get; set; }

    public static HomeRoomCardVm From(LiveDirRoomCard x)
    {
        var online = x.Online is null ? "" : FormatOnline(x.Online.Value);
        var onlineVis = string.IsNullOrWhiteSpace(online) ? Visibility.Collapsed : Visibility.Visible;
        return new HomeRoomCardVm
        {
            Input = (x.Input ?? "").Trim(),
            Title = (x.Title ?? "").Trim(),
            Streamer = (x.UserName ?? "").Trim(),
            OnlineText = online,
            OnlineVisibility = onlineVis,
            Cover = TryCreateBitmap(x.Cover),
        };
    }

    private static string FormatOnline(long v)
    {
        if (v >= 10_000)
        {
            return $"{(v / 1000) / 10.0:0.0}万";
        }
        return v.ToString();
    }

    private static BitmapImage? TryCreateBitmap(string? url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            return null;
        }
        try
        {
            var s = url.Trim();
            if (s.StartsWith("//"))
            {
                s = "https:" + s;
            }
            if (!Uri.TryCreate(s, UriKind.Absolute, out var u))
            {
                return null;
            }
            return new BitmapImage(u);
        }
        catch
        {
            return null;
        }
    }
}
