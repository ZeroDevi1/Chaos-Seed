using System.Collections.ObjectModel;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using WinRT.Interop;
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
    private AppWindow? _appWindow;

    public DanmakuChatWindow()
    {
        InitializeComponent();

        TryApplyDefaultWindowSize();

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
    }

    private void OnMsg(object? sender, DanmakuMessage msg)
    {
        _ = sender;
        _store?.Enqueue(msg);
    }

    private void Cleanup()
    {
        DanmakuService.Instance.Message -= OnMsg;

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

    private void TryApplyDefaultWindowSize()
    {
        try
        {
            var hwnd = WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            var id = global::WinRT.Interop.Win32Interop.GetWindowIdFromWindow(hwnd);
            _appWindow = AppWindow.GetFromWindowId(id);

            if (_appWindow.Presenter is OverlappedPresenter p)
            {
                try
                {
                    p.IsMaximizable = false;
                }
                catch
                {
                    // ignore
                }
            }

            // Default to a narrow chat window.
            _appWindow.Resize(new global::Windows.Graphics.SizeInt32(380, 760));
        }
        catch
        {
            // ignore
        }
    }
}
