using System.Collections.ObjectModel;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using VirtualKey = Windows.System.VirtualKey;

namespace ChaosSeed.WinUI3.Windows;

public sealed partial class DanmakuChatWindow : Window
{
    public ObservableCollection<DanmakuRowVm> Rows { get; } = new();

    private readonly Microsoft.UI.Dispatching.DispatcherQueue _dq =
        Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();

    private DanmakuListStore? _store;
    private DanmakuImageLoader? _images;
    private CancellationTokenSource? _cts;

    public DanmakuChatWindow()
    {
        InitializeComponent();

        _cts = new CancellationTokenSource();
        _images = new DanmakuImageLoader(_dq, maxConcurrency: 4);
        _store = new DanmakuListStore(
            _dq,
            Rows,
            (msg, row) =>
            {
                if (!string.IsNullOrWhiteSpace(msg.ImageUrl))
                {
                    _ = _images!.TryLoadEmoteAsync(msg.SessionId, row, msg.ImageUrl!, _cts.Token);
                }
            },
            maxRows: 400,
            maxFlushPerTick: 30
        );

        DanmakuService.Instance.Message += OnMsg;
        DanmakuService.Instance.StatusChanged += OnStatusChanged;
        Closed += (_, _) => Cleanup();
        Activated += (_, _) =>
        {
            try
            {
                Root.Focus(FocusState.Programmatic);
            }
            catch
            {
                // ignore
            }
        };

        UpdateHeader();
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
        UpdateHeader();
    }

    private void UpdateHeader()
    {
        _dq.TryEnqueue(() =>
        {
            StatusText.Text = DanmakuService.Instance.StatusText;
            BackendText.Text = $"后端：{DanmakuService.Instance.BackendName}";
        });
    }

    private void Cleanup()
    {
        DanmakuService.Instance.Message -= OnMsg;
        DanmakuService.Instance.StatusChanged -= OnStatusChanged;

        try
        {
            _cts?.Cancel();
        }
        catch
        {
            // ignore
        }

        try
        {
            _cts?.Dispose();
        }
        catch
        {
            // ignore
        }
        _cts = null;

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

    private void OnKeyDown(object sender, KeyRoutedEventArgs e)
    {
        _ = sender;
        if (e.Key == VirtualKey.Escape)
        {
            e.Handled = true;
            try
            {
                Close();
            }
            catch
            {
                // ignore
            }
        }
    }
}

