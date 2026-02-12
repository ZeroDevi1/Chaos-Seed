using System.Diagnostics;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using WinRT.Interop;
using VirtualKey = Windows.System.VirtualKey;

namespace ChaosSeed.WinUI3.Windows;

public sealed partial class DanmakuOverlayWindow : Window
{
    private readonly Microsoft.UI.Dispatching.DispatcherQueue _dq =
        Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();

    private readonly object _queueGate = new();
    private readonly Queue<DanmakuMessage> _queue = new();
    private readonly List<Sprite> _sprites = new();
    private readonly Random _rand = new();
    private readonly SemaphoreSlim _imgSem = new(4, 4);

    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _timer;
    private long _lastTickTs;
    private bool _clickThrough;
    private IntPtr _hwnd;
    private AppWindow? _appWindow;
    private CancellationTokenSource? _cts;

    private const double LaneHeight = 32;
    private const double TopPad = 44; // leave room for hint box
    private const double BottomPad = 10;
    private int _laneCursor;

    public DanmakuOverlayWindow()
    {
        InitializeComponent();

        _cts = new CancellationTokenSource();

        InitWindowStyle();

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

        _timer = _dq.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(16);
        _timer.IsRepeating = true;
        _timer.Tick += (_, _) => Tick();
        _lastTickTs = Stopwatch.GetTimestamp();
        _timer.Start();

        UpdateHint();
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

            Win32OverlayInterop.EnsureLayered(_hwnd);
            Win32OverlayInterop.SetTopmost(_hwnd, true);

            var id = Win32Interop.GetWindowIdFromWindow(_hwnd);
            _appWindow = AppWindow.GetFromWindowId(id);

            if (_appWindow.Presenter is OverlappedPresenter p)
            {
                try
                {
                    p.SetBorderAndTitleBar(false, false);
                }
                catch
                {
                    // ignore
                }

                try
                {
                    p.IsAlwaysOnTop = true;
                }
                catch
                {
                    // ignore
                }

                try
                {
                    p.IsMinimizable = false;
                    p.IsMaximizable = false;
                }
                catch
                {
                    // ignore
                }
            }

            try
            {
                _appWindow.Title = "Chaos Seed - Overlay";
            }
            catch
            {
                // ignore
            }

            // A reasonable default size; user can later adjust in OS window manager if needed.
            try
            {
                _appWindow.Resize(new Windows.Graphics.SizeInt32(960, 540));
            }
            catch
            {
                // ignore
            }
        }
        catch
        {
            // ignore
        }
    }

    private void Cleanup()
    {
        DanmakuService.Instance.Message -= OnMsg;

        try
        {
            _timer?.Stop();
        }
        catch
        {
            // ignore
        }
        _timer = null;

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
            _imgSem.Dispose();
        }
        catch
        {
            // ignore
        }
    }

    private void OnMsg(object? sender, DanmakuMessage msg)
    {
        _ = sender;
        if (msg is null)
        {
            return;
        }

        lock (_queueGate)
        {
            _queue.Enqueue(msg);
            if (_queue.Count > 1000)
            {
                while (_queue.Count > 200)
                {
                    _queue.Dequeue();
                }
            }
        }
    }

    private void Tick()
    {
        var now = Stopwatch.GetTimestamp();
        var dt = (now - _lastTickTs) / (double)Stopwatch.Frequency;
        if (dt < 0 || dt > 0.2)
        {
            dt = 0.016;
        }
        _lastTickTs = now;

        MoveSprites(dt);
        SpawnSprites(maxSpawn: 6);
    }

    private void MoveSprites(double dt)
    {
        if (Stage.ActualWidth <= 1 || Stage.ActualHeight <= 1)
        {
            return;
        }

        for (var i = _sprites.Count - 1; i >= 0; i--)
        {
            var s = _sprites[i];
            s.X -= s.SpeedPxPerSec * dt;
            Canvas.SetLeft(s.Element, s.X);

            if (s.X + s.Width < -10)
            {
                try
                {
                    Stage.Children.Remove(s.Element);
                }
                catch
                {
                    // ignore
                }
                _sprites.RemoveAt(i);
            }
        }
    }

    private void SpawnSprites(int maxSpawn)
    {
        if (Stage.ActualWidth <= 1 || Stage.ActualHeight <= 1)
        {
            return;
        }

        var availableHeight = Stage.ActualHeight - TopPad - BottomPad;
        var laneCount = Math.Max(1, (int)Math.Floor(availableHeight / LaneHeight));

        List<DanmakuMessage>? batch = null;
        lock (_queueGate)
        {
            if (_queue.Count == 0)
            {
                return;
            }

            var n = Math.Min(maxSpawn, _queue.Count);
            batch = new List<DanmakuMessage>(n);
            for (var i = 0; i < n; i++)
            {
                batch.Add(_queue.Dequeue());
            }
        }

        if (batch is null || batch.Count == 0)
        {
            return;
        }

        foreach (var msg in batch)
        {
            var lane = _laneCursor++ % laneCount;
            var y = TopPad + lane * LaneHeight;
            SpawnOne(msg, y);
        }
    }

    private void SpawnOne(DanmakuMessage msg, double y)
    {
        var user = (msg.User ?? "").Trim();
        if (user.Length == 0)
        {
            user = "??";
        }

        var text = (msg.Text ?? "").Trim();
        if (text.Length == 0 && !string.IsNullOrWhiteSpace(msg.ImageUrl))
        {
            text = "[表情]";
        }

        var display = DanmakuRowVm.IsImagePlaceholderText(text) ? $"{user}: [表情]" : $"{user}: {text}";

        var sp = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
        };

        var tb = new TextBlock
        {
            Text = display,
            Foreground = new SolidColorBrush(Colors.White),
            FontSize = 20,
        };
        sp.Children.Add(tb);

        Image? img = null;
        if (!string.IsNullOrWhiteSpace(msg.ImageUrl))
        {
            img = new Image
            {
                Width = 28,
                Height = 28,
                Stretch = Stretch.Uniform,
                Visibility = Visibility.Collapsed,
            };
            sp.Children.Add(img);
        }

        // Measure before adding to canvas so we can remove it when off-screen.
        sp.Measure(new Windows.Foundation.Size(double.PositiveInfinity, double.PositiveInfinity));
        var width = Math.Max(60, sp.DesiredSize.Width);

        var x = Stage.ActualWidth + 10;
        Canvas.SetLeft(sp, x);
        Canvas.SetTop(sp, y);
        Stage.Children.Add(sp);

        var speed = 160 + _rand.NextDouble() * 80;
        _sprites.Add(new Sprite(sp, x, width, speed));

        if (img is not null && _cts is not null)
        {
            _ = TryLoadOverlayImageAsync(msg.SessionId, msg.ImageUrl!, img, _cts.Token);
        }
    }

    private async Task TryLoadOverlayImageAsync(string sessionId, string url, Image img, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            return;
        }

        await _imgSem.WaitAsync(ct);
        try
        {
            var res = await DanmakuService.Instance.FetchImageAsync(sid, url, ct);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            var bmp = new BitmapImage();
            using var ms = new Windows.Storage.Streams.InMemoryRandomAccessStream();
            await ms.WriteAsync(bytes.AsBuffer());
            ms.Seek(0);
            await bmp.SetSourceAsync(ms);

            img.Source = bmp;
            img.Visibility = Visibility.Visible;
        }
        catch
        {
            // ignore
        }
        finally
        {
            _imgSem.Release();
        }
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
            return;
        }

        if (e.Key == VirtualKey.F2)
        {
            e.Handled = true;
            ToggleClickThrough();
        }
    }

    private void ToggleClickThrough()
    {
        _clickThrough = !_clickThrough;
        try
        {
            Win32OverlayInterop.SetClickThrough(_hwnd, _clickThrough);
        }
        catch
        {
            // ignore
        }
        UpdateHint();
    }

    private void UpdateHint()
    {
        var t = _clickThrough ? "ON" : "OFF";
        HintText.Text = $"Overlay: Esc 关闭 / F2 点击穿透（{t}）";
    }

    private sealed class Sprite
    {
        public Sprite(UIElement element, double x, double width, double speedPxPerSec)
        {
            Element = element;
            X = x;
            Width = width;
            SpeedPxPerSec = speedPxPerSec;
        }

        public UIElement Element { get; }
        public double X { get; set; }
        public double Width { get; }
        public double SpeedPxPerSec { get; }
    }
}
