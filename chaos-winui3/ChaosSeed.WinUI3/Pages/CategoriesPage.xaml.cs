using System.Collections.ObjectModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services.LiveDirectoryBackends;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class CategoriesPage : Page
{
    public ObservableCollection<CategoryVm> Categories { get; } = new();
    public ObservableCollection<HomeRoomCardVm> Rooms { get; } = new();

    private ILiveDirectoryBackend _backend;
    private string _site = "bili_live";
    private ItemsWrapGrid? _roomsWrapGrid;

    private string? _activeParentId;
    private string? _activeCategoryId;
    private string _activeCategoryName = "";
    private int _page = 1;
    private bool _hasMore = true;
    private bool _isLoading;
    private CancellationTokenSource? _cts;

    public CategoriesPage()
    {
        InitializeComponent();
        _backend = LiveDirectoryBackendFactory.Create();
        if (PlatformNav.MenuItems.Count > 0)
        {
            PlatformNav.SelectedItem = PlatformNav.MenuItems[0];
        }
        if (!string.IsNullOrWhiteSpace(_backend.InitNotice))
        {
            ShowInfo(_backend.InitNotice!);
        }
        SetCenterState(loading: true, text: "加载中...", showRetry: false);
        _ = LoadCategoriesAsync();
    }

    protected override void OnNavigatedFrom(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        try { _cts?.Cancel(); } catch { }
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

    private async Task LoadCategoriesAsync()
    {
        if (_isLoading)
        {
            return;
        }
        _isLoading = true;
        LoadMoreBtn.IsEnabled = false;
        if (Categories.Count == 0)
        {
            SetCenterState(loading: true, text: "加载中...", showRetry: false);
        }

        _cts?.Cancel();
        _cts?.Dispose();
        _cts = new CancellationTokenSource(TimeSpan.FromSeconds(15));
        var ct = _cts.Token;

        try
        {
            Categories.Clear();
            var cats = await _backend.GetCategoriesAsync(_site, ct);
            foreach (var c in cats ?? Array.Empty<LiveDirCategory>())
            {
                Categories.Add(CategoryVm.From(c));
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
            // load more only used in Rooms view
            LoadMoreBtn.IsEnabled = PagerBar.Visibility == Visibility.Visible && _hasMore;

            if (Categories.Count == 0)
            {
                SetCenterState(loading: false, text: "暂无分类数据（可尝试重试或切换平台）", showRetry: true);
            }
            else
            {
                SetCenterState(loading: false, text: "", showRetry: false);
            }
        }
    }

    private async Task ReloadRoomsAsync()
    {
        Rooms.Clear();
        _page = 1;
        _hasMore = true;
        await LoadNextRoomsPageAsync();
    }

    private async Task LoadNextRoomsPageAsync()
    {
        if (_isLoading || !_hasMore)
        {
            return;
        }

        if (string.IsNullOrWhiteSpace(_activeCategoryId))
        {
            return;
        }

        _isLoading = true;
        LoadingRing.IsActive = true;
        LoadMoreBtn.IsEnabled = false;
        if (Rooms.Count == 0)
        {
            SetCenterState(loading: true, text: "加载中...", showRetry: false);
        }

        _cts?.Cancel();
        _cts?.Dispose();
        _cts = new CancellationTokenSource(TimeSpan.FromSeconds(15));
        var ct = _cts.Token;

        try
        {
            var res = await _backend.GetCategoryRoomsAsync(
                _site,
                _activeParentId,
                _activeCategoryId!,
                _page,
                ct
            );

            var items = res.Items ?? Array.Empty<LiveDirRoomCard>();
            foreach (var x in items)
            {
                Rooms.Add(HomeRoomCardVm.From(x));
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

            if (Rooms.Count == 0 && RoomsView.Visibility == Visibility.Visible)
            {
                SetCenterState(loading: false, text: "暂无直播间数据（可尝试重试）", showRetry: true);
            }
            else if (Categories.Count > 0)
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

    private async void OnPlatformNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        _ = sender;
        if (args.SelectedItem is not NavigationViewItem item || item.Tag is not string tag)
        {
            return;
        }

        var site = tag.Trim();
        if (string.IsNullOrWhiteSpace(site))
        {
            return;
        }

        _site = site;

        // Reset view state.
        CategoriesView.Visibility = Visibility.Visible;
        RoomsView.Visibility = Visibility.Collapsed;
        PagerBar.Visibility = Visibility.Collapsed;
        Rooms.Clear();
        _activeParentId = null;
        _activeCategoryId = null;
        _activeCategoryName = "";
        SetCenterState(loading: true, text: "加载中...", showRetry: false);

        await LoadCategoriesAsync();
    }

    private async void OnSubCategoryButtonClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (sender is not Button btn || btn.Tag is not SubCategoryVm vm)
        {
            return;
        }
        if (string.IsNullOrWhiteSpace(vm.Id))
        {
            return;
        }

        _activeParentId = vm.ParentId;
        _activeCategoryId = vm.Id;
        _activeCategoryName = vm.Name ?? "";

        RoomsTitle.Text = _activeCategoryName;
        CategoriesView.Visibility = Visibility.Collapsed;
        RoomsView.Visibility = Visibility.Visible;
        PagerBar.Visibility = Visibility.Visible;
        TryUpdateRoomsWrapGrid();
        UpdateRoomsItemWidth(RoomGrid.ActualWidth);

        await ReloadRoomsAsync();
    }

    private async void OnBackClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        CategoriesView.Visibility = Visibility.Visible;
        RoomsView.Visibility = Visibility.Collapsed;
        PagerBar.Visibility = Visibility.Collapsed;
        Rooms.Clear();
        _activeParentId = null;
        _activeCategoryId = null;
        _activeCategoryName = "";
        LoadMoreBtn.IsEnabled = false;
        if (Categories.Count > 0)
        {
            SetCenterState(loading: false, text: "", showRetry: false);
        }
        await Task.CompletedTask;
    }

    private async void OnLoadMoreClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (RoomsView.Visibility == Visibility.Visible)
        {
            await LoadNextRoomsPageAsync();
        }
    }

    private async void OnRetryClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (RoomsView.Visibility == Visibility.Visible)
        {
            await ReloadRoomsAsync();
        }
        else
        {
            await LoadCategoriesAsync();
        }
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

    private void OnRoomGridLoaded(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        TryUpdateRoomsWrapGrid();
        UpdateRoomsItemWidth(RoomGrid.ActualWidth);
    }

    private void OnRoomGridSizeChanged(object sender, SizeChangedEventArgs e)
    {
        _ = sender;
        TryUpdateRoomsWrapGrid();
        UpdateRoomsItemWidth(e.NewSize.Width);
    }

    private void TryUpdateRoomsWrapGrid()
    {
        if (_roomsWrapGrid is not null)
        {
            return;
        }
        try
        {
            _roomsWrapGrid = RoomGrid.ItemsPanelRoot as ItemsWrapGrid;
        }
        catch
        {
            // ignore
        }
    }

    private void UpdateRoomsItemWidth(double width)
    {
        var panel = _roomsWrapGrid;
        if (panel is null || width <= 0)
        {
            return;
        }

        const double gap = 12; // matches ChaosCardGridViewItemStyle margin
        const double min = 240;
        const double max = 360;
        var avail = Math.Max(0, width - 24);
        var cols = (int)Math.Floor((avail + gap) / (min + gap));
        if (cols < 1)
        {
            cols = 1;
        }
        var item = Math.Floor((avail + gap) / cols) - gap;
        item = Math.Clamp(item, min, max);

        if (Math.Abs(panel.ItemWidth - item) > 0.5)
        {
            panel.ItemWidth = item;
        }
    }
}

public sealed class CategoryVm
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public ObservableCollection<SubCategoryVm> Children { get; set; } = new();

    public static CategoryVm From(LiveDirCategory c)
    {
        var vm = new CategoryVm
        {
            Id = (c.Id ?? "").Trim(),
            Name = (c.Name ?? "").Trim(),
        };
        foreach (var x in c.Children ?? Array.Empty<LiveDirSubCategory>())
        {
            vm.Children.Add(SubCategoryVm.From(x));
        }
        return vm;
    }
}

public sealed class SubCategoryVm
{
    public string Id { get; set; } = "";
    public string ParentId { get; set; } = "";
    public string Name { get; set; } = "";
    public BitmapImage? Pic { get; set; }
    public Visibility PicVisibility { get; set; } = Visibility.Collapsed;
    public Visibility PicPlaceholderVisibility { get; set; } = Visibility.Visible;

    public static SubCategoryVm From(LiveDirSubCategory x)
    {
        var bmp = TryCreateBitmap(x.Pic);
        var hasPic = bmp is not null;
        return new SubCategoryVm
        {
            Id = (x.Id ?? "").Trim(),
            ParentId = (x.ParentId ?? "").Trim(),
            Name = (x.Name ?? "").Trim(),
            Pic = bmp,
            PicVisibility = hasPic ? Visibility.Visible : Visibility.Collapsed,
            PicPlaceholderVisibility = hasPic ? Visibility.Collapsed : Visibility.Visible,
        };
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
