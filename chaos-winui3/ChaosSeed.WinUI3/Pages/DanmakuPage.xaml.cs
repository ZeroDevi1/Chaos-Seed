using System.Collections.ObjectModel;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using VirtualKey = Windows.System.VirtualKey;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class DanmakuPage : Page
{
    public ObservableCollection<DanmakuRowVm> Rows { get; } = new();

    private readonly Microsoft.UI.Dispatching.DispatcherQueue _dq =
        Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();

    private DanmakuListStore? _store;
    private DanmakuImageLoader? _images;
    private CancellationTokenSource? _pageCts;

    public DanmakuPage()
    {
        InitializeComponent();
        Loaded += (_, _) => Start();
        Unloaded += (_, _) => Stop();
    }

    private void Start()
    {
        if (_pageCts is not null)
        {
            return;
        }

        _pageCts = new CancellationTokenSource();
        _images = new DanmakuImageLoader(_dq, maxConcurrency: 4);
        _store = new DanmakuListStore(
            _dq,
            Rows,
            (msg, row) =>
            {
                if (!string.IsNullOrWhiteSpace(msg.ImageUrl))
                {
                    _ = _images!.TryLoadEmoteAsync(msg.SessionId, row, msg.ImageUrl!, _pageCts.Token);
                }
            },
            maxRows: 400,
            maxFlushPerTick: 30
        );

        DanmakuService.Instance.Message += OnMsg;
        DanmakuService.Instance.StatusChanged += OnStatusChanged;

        UpdateUiFromService();
    }

    private void Stop()
    {
        if (_pageCts is null)
        {
            return;
        }

        DanmakuService.Instance.Message -= OnMsg;
        DanmakuService.Instance.StatusChanged -= OnStatusChanged;

        try
        {
            _pageCts.Cancel();
        }
        catch
        {
            // ignore
        }
        finally
        {
            _pageCts.Dispose();
            _pageCts = null;
        }

        try
        {
            _images?.Dispose();
        }
        catch
        {
            // ignore
        }
        _images = null;

        try
        {
            _store?.Dispose();
        }
        catch
        {
            // ignore
        }
        _store = null;
    }

    private void OnMsg(object? sender, DanmakuMessage msg)
    {
        _ = sender;
        _store?.Enqueue(msg);
    }

    private void OnStatusChanged(object? sender, string status)
    {
        _ = sender;
        _ = status;
        UpdateUiFromService();
    }

    private void UpdateUiFromService()
    {
        _dq.TryEnqueue(() =>
        {
            BackendLabel.Text = $"后端：{DanmakuService.Instance.BackendName}"
                + (string.IsNullOrWhiteSpace(DanmakuService.Instance.BackendInitNotice)
                    ? ""
                    : $"（{DanmakuService.Instance.BackendInitNotice}）");

            var connected = !string.IsNullOrWhiteSpace(DanmakuService.Instance.CurrentSessionId);
            ConnectBtn.IsEnabled = !connected;
            DisconnectBtn.IsEnabled = connected;

            StatusBar.Message = DanmakuService.Instance.StatusText;
        });
    }

    private async void OnConnectClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        await ConnectAsync();
    }

    private async Task ConnectAsync()
    {
        var input = (InputBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            StatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Warning;
            StatusBar.Message = "请输入直播间地址";
            return;
        }

        var ct = _pageCts?.Token ?? CancellationToken.None;
        try
        {
            StatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
            StatusBar.Message = "正在连接...";
            await DanmakuService.Instance.ConnectAsync(input, ct);
            StatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Success;
            StatusBar.Message = "已连接";
        }
        catch (OperationCanceledException)
        {
            StatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
            StatusBar.Message = "已取消";
        }
        catch (Exception ex)
        {
            StatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
            StatusBar.Message = ex.Message;
        }
        finally
        {
            UpdateUiFromService();
        }
    }

    private async void OnDisconnectClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        var ct = _pageCts?.Token ?? CancellationToken.None;
        try
        {
            await DanmakuService.Instance.DisconnectAsync(ct);
        }
        catch
        {
            // ignore
        }
        finally
        {
            UpdateUiFromService();
        }
    }

    private void OnOpenChatClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        AuxWindowService.Instance.OpenOrShowChat();
    }

    private void OnOpenOverlayClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        AuxWindowService.Instance.OpenOrShowOverlay();
    }

    private async void OnInputKeyDown(object sender, KeyRoutedEventArgs e)
    {
        _ = sender;
        if (e.Key != VirtualKey.Enter)
        {
            return;
        }

        if (!ConnectBtn.IsEnabled)
        {
            return;
        }

        e.Handled = true;
        await ConnectAsync();
    }
}

