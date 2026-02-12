using System.Collections.ObjectModel;
using System.IO;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using WinRT.Interop;

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
    private IntPtr _hwnd;

    public DanmakuChatWindow()
    {
        InitializeComponent();

        InitWindowStyle();

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
            SaveBoundsBestEffort();
        }
        catch
        {
            // ignore
        }

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
        if (e.Key == global::Windows.System.VirtualKey.Escape)
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

    private void InitWindowStyle()
    {
        try
        {
            _hwnd = WindowNative.GetWindowHandle(this);
            if (_hwnd == IntPtr.Zero)
            {
                return;
            }

            var id = global::Microsoft.UI.Win32Interop.GetWindowIdFromWindow(_hwnd);
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

            try
            {
                _appWindow.Title = "Chat";
            }
            catch
            {
                // ignore
            }

            try
            {
                _appWindow.SetIcon("Assets\\icon.ico");
            }
            catch
            {
                try
                {
                    var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "icon.ico");
                    _appWindow.SetIcon(iconPath);
                }
                catch
                {
                    // ignore
                }
            }

            ApplySavedBoundsOrDefault();
        }
        catch
        {
            // ignore
        }
    }

    private void ApplySavedBoundsOrDefault()
    {
        if (_appWindow is null)
        {
            return;
        }

        var s = SettingsService.Instance.Current;
        var x = s.DanmakuChatX;
        var y = s.DanmakuChatY;
        var w = s.DanmakuChatWidth;
        var h = s.DanmakuChatHeight;

        var size = new global::Windows.Graphics.SizeInt32(380, 760);
        if (w is > 100 and < 10_000 && h is > 100 and < 10_000)
        {
            size = new global::Windows.Graphics.SizeInt32(w.Value, h.Value);
        }

        try
        {
            _appWindow.Resize(size);
        }
        catch
        {
            // ignore
        }

        if (x is > -50_000 and < 50_000 && y is > -50_000 and < 50_000)
        {
            try
            {
                _appWindow.Move(new global::Windows.Graphics.PointInt32(x.Value, y.Value));
            }
            catch
            {
                // ignore
            }
        }
    }

    private void SaveBoundsBestEffort()
    {
        if (_appWindow is null)
        {
            return;
        }

        global::Windows.Graphics.PointInt32 pos;
        global::Windows.Graphics.SizeInt32 size;
        try
        {
            pos = _appWindow.Position;
            size = _appWindow.Size;
        }
        catch
        {
            return;
        }

        if (size.Width < 100 || size.Height < 100)
        {
            return;
        }

        SettingsService.Instance.Update(s =>
        {
            s.DanmakuChatX = pos.X;
            s.DanmakuChatY = pos.Y;
            s.DanmakuChatWidth = size.Width;
            s.DanmakuChatHeight = size.Height;
        });
    }
}
