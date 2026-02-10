using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class LivePage : Page
{
    private const int DanmakuDefaultWidthPx = 280;
    private const int SplitterWidthPx = 6;
    private const int DanmakuMinWidthPx = 220;
    private const int DanmakuMaxWidthPx = 480;
    public ObservableCollection<DanmakuRowVm> Rows { get; } = new();

    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private readonly SemaphoreSlim _imageSem = new(4, 4);
    private readonly PlayerEngineService _player;

    private string? _sessionId;
    private bool _danmakuExpanded = true;
    private LiveOpenResult? _lastOpen;
    private int _danmakuWidthPx = DanmakuDefaultWidthPx;
    private bool _resizingDanmaku;
    private double _resizeStartX;
    private int _resizeStartWidth;

    public LivePage()
    {
        InitializeComponent();
        DaemonClient.Instance.DanmakuMessageReceived += OnDanmakuMessage;
        _player = new PlayerEngineService(_dq, SystemPlayer, VlcImage);
        _player.Error += (_, msg) => _dq.TryEnqueue(() => ShowPlayerError(msg));
        _player.Info += (_, msg) => _dq.TryEnqueue(() => ShowPlayerInfo(msg));
    }

    protected override void OnNavigatedTo(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (e.Parameter is LiveOpenResult res)
        {
            _lastOpen = res;
            BeginLive(res);
        }
        else
        {
            ShowParsePanel();
        }
        UpdateDanmakuPane();
    }

    protected override async void OnNavigatedFrom(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        DaemonClient.Instance.DanmakuMessageReceived -= OnDanmakuMessage;

        if (_sessionId is not null)
        {
            try { await DaemonClient.Instance.CloseLiveAsync(_sessionId); } catch { }
        }

        _player.Dispose();
    }

    private void BeginLive(LiveOpenResult res)
    {
        _sessionId = res.SessionId;
        _lastOpen = res;
        Rows.Clear();
        HideParsePanel();

        var engine = SettingsService.Instance.Current.PlayerEngine;
        _player.Play(engine, res.Url, res.Referer, res.UserAgent);
    }

    private void ShowParsePanel()
    {
        ParsePanel.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
        ParseStatusBar.IsOpen = false;
        PlayerStatusBar.IsOpen = false;
    }

    private void HideParsePanel()
    {
        ParsePanel.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    private async void OnParseClicked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var input = (InputBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            ShowParseError("请输入直播间地址。");
            return;
        }

        ParseBtn.IsEnabled = false;
        InputBox.IsEnabled = false;
        ShowParseInfo("解析中…");

        try
        {
            if (_sessionId is not null)
            {
                try { await DaemonClient.Instance.CloseLiveAsync(_sessionId); } catch { }
                _sessionId = null;
            }

            var res = await DaemonClient.Instance.OpenLiveAsync(input);
            ParseStatusBar.IsOpen = false;
            BeginLive(res);
        }
        catch (Exception ex)
        {
            ShowParseError(ex.Message);
        }
        finally
        {
            ParseBtn.IsEnabled = true;
            InputBox.IsEnabled = true;
        }
    }

    private void ShowParseError(string msg)
    {
        ParseStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
        ParseStatusBar.Title = "失败";
        ParseStatusBar.Message = msg;
        ParseStatusBar.IsOpen = true;
    }

    private void ShowParseInfo(string msg)
    {
        ParseStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
        ParseStatusBar.Title = "提示";
        ParseStatusBar.Message = msg;
        ParseStatusBar.IsOpen = true;
    }

    private void OnToggleDanmaku(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        _danmakuExpanded = !_danmakuExpanded;
        UpdateDanmakuPane();
    }

    private void UpdateDanmakuPane()
    {
        if (_danmakuExpanded)
        {
            DanmakuCol.Width = new GridLength(_danmakuWidthPx);
            SplitterCol.Width = new Microsoft.UI.Xaml.GridLength(SplitterWidthPx);
            DanmakuPane.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
            DanmakuResizeHandle.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
            DanmakuToggleIcon.Symbol = Symbol.Back;
        }
        else
        {
            DanmakuCol.Width = new Microsoft.UI.Xaml.GridLength(0);
            SplitterCol.Width = new Microsoft.UI.Xaml.GridLength(0);
            DanmakuPane.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
            DanmakuResizeHandle.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
            DanmakuToggleIcon.Symbol = Symbol.Forward;
        }
    }

    private void OnDanmakuResizePressed(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (!_danmakuExpanded)
        {
            return;
        }

        _resizingDanmaku = true;
        _resizeStartX = e.GetCurrentPoint(this).Position.X;
        _resizeStartWidth = _danmakuWidthPx;
        DanmakuResizeHandle.CapturePointer(e.Pointer);
        e.Handled = true;
    }

    private void OnDanmakuResizeMoved(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (!_resizingDanmaku || !_danmakuExpanded)
        {
            return;
        }

        var x = e.GetCurrentPoint(this).Position.X;
        var dx = _resizeStartX - x; // drag left => wider danmaku
        var newWidth = (int)Math.Round(_resizeStartWidth + dx);
        newWidth = Math.Clamp(newWidth, DanmakuMinWidthPx, DanmakuMaxWidthPx);
        if (newWidth == _danmakuWidthPx)
        {
            return;
        }

        _danmakuWidthPx = newWidth;
        DanmakuCol.Width = new GridLength(_danmakuWidthPx);
        e.Handled = true;
    }

    private void OnDanmakuResizeReleased(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (!_resizingDanmaku)
        {
            return;
        }

        _resizingDanmaku = false;
        try
        {
            DanmakuResizeHandle.ReleasePointerCapture(e.Pointer);
        }
        catch
        {
            // ignore
        }
        e.Handled = true;
    }

    private void ShowPlayerError(string msg)
    {
        PlayerStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
        PlayerStatusBar.Title = "播放失败";
        PlayerStatusBar.Message = msg;
        PlayerStatusBar.IsOpen = true;
    }

    private void ShowPlayerInfo(string msg)
    {
        PlayerStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
        PlayerStatusBar.Title = "播放器";
        PlayerStatusBar.Message = msg;
        PlayerStatusBar.IsOpen = true;
    }

    private void OnDanmakuMessage(object? sender, DanmakuMessage msg)
    {
        if (_sessionId is null || msg.SessionId != _sessionId)
        {
            return;
        }

        _dq.TryEnqueue(async () =>
        {
            var row = new DanmakuRowVm(msg.User, msg.Text);
            Rows.Add(row);
            if (Rows.Count > 5000)
            {
                Rows.RemoveAt(0);
            }
            if (_danmakuExpanded)
            {
                DanmakuList.ScrollIntoView(row);
            }

            if (!string.IsNullOrWhiteSpace(msg.ImageUrl))
            {
                await TryLoadEmoteAsync(row, msg.ImageUrl!);
            }
        });
    }

    private async Task TryLoadEmoteAsync(DanmakuRowVm row, string url)
    {
        if (_sessionId is null)
        {
            return;
        }

        await _imageSem.WaitAsync();
        try
        {
            var res = await DaemonClient.Instance.FetchDanmakuImageAsync(_sessionId, url);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            using var ms = new InMemoryRandomAccessStream();
            await ms.WriteAsync(bytes.AsBuffer());
            ms.Seek(0);

            var bmp = new BitmapImage();
            await bmp.SetSourceAsync(ms);
            row.Emote = bmp;
        }
        catch
        {
            // ignore
        }
        finally
        {
            _imageSem.Release();
        }
    }
}

public sealed class DanmakuRowVm : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    public DanmakuRowVm(string user, string text)
    {
        User = user;
        Text = text;
    }

    public string User { get; }
    public string Text { get; }

    public string DisplayText => $"{User}: {Text}";

    private BitmapImage? _emote;
    public BitmapImage? Emote
    {
        get => _emote;
        set
        {
            if (ReferenceEquals(_emote, value))
            {
                return;
            }
            _emote = value;
            OnPropertyChanged();
        }
    }

    private void OnPropertyChanged([CallerMemberName] string? name = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
    }
}
