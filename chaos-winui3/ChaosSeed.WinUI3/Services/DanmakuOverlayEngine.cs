using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Foundation;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Services;

public sealed class DanmakuOverlayEngine : IDisposable
{
    private sealed class Sprite
    {
        public FrameworkElement Element { get; }
        public int Lane { get; }
        public double X { get; set; }
        public double Y { get; }
        public double Width { get; }
        public double SpeedPxPerSec { get; }

        public Sprite(FrameworkElement element, int lane, double x, double y, double width, double speedPxPerSec)
        {
            Element = element;
            Lane = lane;
            X = x;
            Y = y;
            Width = width;
            SpeedPxPerSec = speedPxPerSec;
        }
    }

    private readonly DispatcherQueue _dq;
    private readonly Canvas _stage;
    private readonly Func<string, string, CancellationToken, Task<DanmakuFetchImageResult>> _fetchImageAsync;

    private readonly Queue<DanmakuMessage> _queue = new();
    private readonly Dictionary<string, long> _recent = new();
    private readonly List<Sprite> _sprites = new();
    private readonly Random _rand = new();
    private readonly SemaphoreSlim _imgSem = new(4, 4);

    private DispatcherQueueTimer? _timer;
    private long _lastTickTs;

    private CancellationTokenSource? _imgCts = new();

    private bool _active;

    private bool _enabled = true;
    private double _opacity = 1.0;
    private double _fontScale = 1.0;
    private double _density = 1.0;
    private DanmakuOverlayAreaMode _area = DanmakuOverlayAreaMode.Full;

    private int _laneCursor;

    private const double TopPad = 10;
    private const double BottomPad = 10;
    private const int DedupeWindowMs = 80;

    public DanmakuOverlayEngine(
        DispatcherQueue dq,
        Canvas stage,
        Func<string, string, CancellationToken, Task<DanmakuFetchImageResult>> fetchImageAsync
    )
    {
        _dq = dq;
        _stage = stage;
        _fetchImageAsync = fetchImageAsync;

        // Overlay must not block player interactions.
        try { _stage.IsHitTestVisible = false; } catch { }

        _timer = _dq.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(16);
        _timer.IsRepeating = true;
        _timer.Tick += (_, _) => Tick();
        _lastTickTs = Stopwatch.GetTimestamp();
        _timer.Start();

        SyncStageVisibility();
    }

    public void Dispose()
    {
        try { _timer?.Stop(); } catch { }
        _timer = null;

        try
        {
            _imgCts?.Cancel();
            _imgCts?.Dispose();
        }
        catch
        {
            // ignore
        }
        _imgCts = null;

        try { _imgSem.Dispose(); } catch { }

        try { ClearCore(); } catch { }
    }

    public void SetActive(bool active)
    {
        if (!_dq.HasThreadAccess)
        {
            _dq.TryEnqueue(() => SetActive(active));
            return;
        }

        if (_active == active)
        {
            return;
        }

        _active = active;
        if (!_active)
        {
            ClearCore();
        }
        SyncStageVisibility();
    }

    public void ApplySettings(AppSettings s)
    {
        if (s is null)
        {
            return;
        }

        if (!_dq.HasThreadAccess)
        {
            _dq.TryEnqueue(() => ApplySettings(s));
            return;
        }

        var nextEnabled = s.DanmakuOverlayEnabled;
        var nextOpacity = Clamp01(s.DanmakuOverlayOpacity);
        var nextFontScale = Math.Clamp(s.DanmakuOverlayFontScale, 0.5, 2.0);
        var nextDensity = Clamp01(s.DanmakuOverlayDensity);
        var nextArea = s.DanmakuOverlayArea;

        var needClear = nextArea != _area || Math.Abs(nextFontScale - _fontScale) > 0.001;

        _enabled = nextEnabled;
        _opacity = nextOpacity;
        _fontScale = nextFontScale;
        _density = nextDensity;
        _area = nextArea;

        try { _stage.Opacity = _opacity; } catch { }

        if (!_enabled)
        {
            // When disabled, clear immediately so we don't keep stale sprites on screen.
            ClearCore();
        }
        else if (needClear)
        {
            // Keep behavior deterministic: area/font changes should enforce the new bounds immediately.
            ClearCore();
        }

        SyncStageVisibility();
    }

    public void Clear()
    {
        if (!_dq.HasThreadAccess)
        {
            _dq.TryEnqueue(ClearCore);
            return;
        }
        ClearCore();
    }

    public void Enqueue(DanmakuMessage msg)
    {
        if (msg is null)
        {
            return;
        }

        if (!_dq.HasThreadAccess)
        {
            _dq.TryEnqueue(() => Enqueue(msg));
            return;
        }

        if (!IsRunning())
        {
            return;
        }

        var text = (msg.Text ?? "").Trim();
        var imageUrl = (msg.ImageUrl ?? "").Trim();
        if (text.Length == 0 && imageUrl.Length == 0)
        {
            return;
        }

        var user = (msg.User ?? "").Trim();
        var key = $"{user}\n{text}\n{imageUrl}";
        var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

        if (_recent.TryGetValue(key, out var last) && now - last < DedupeWindowMs)
        {
            return;
        }
        _recent[key] = now;

        _queue.Enqueue(msg);

        // Overload protection: keep UI responsive.
        if (_queue.Count > 1200)
        {
            while (_queue.Count > 200)
            {
                _queue.Dequeue();
            }
        }

        // Prevent `_recent` from growing unbounded.
        if (_recent.Count > 2000)
        {
            _recent.Clear();
        }
    }

    private void ClearCore()
    {
        if (!_dq.HasThreadAccess)
        {
            _dq.TryEnqueue(ClearCore);
            return;
        }

        try
        {
            _imgCts?.Cancel();
            _imgCts?.Dispose();
        }
        catch
        {
            // ignore
        }
        _imgCts = new CancellationTokenSource();

        _queue.Clear();
        _recent.Clear();
        _sprites.Clear();

        try { _stage.Children.Clear(); } catch { }
    }

    private void Tick()
    {
        if (!IsRunning())
        {
            return;
        }

        if (_stage.ActualWidth <= 1 || _stage.ActualHeight <= 1)
        {
            return;
        }

        var now = Stopwatch.GetTimestamp();
        var dt = (now - _lastTickTs) / (double)Stopwatch.Frequency;
        if (dt < 0 || dt > 0.2)
        {
            dt = 0.016;
        }
        _lastTickTs = now;

        MoveSprites(dt);
        SpawnSprites();
    }

    private void MoveSprites(double dt)
    {
        for (var i = _sprites.Count - 1; i >= 0; i--)
        {
            var s = _sprites[i];
            s.X -= s.SpeedPxPerSec * dt;
            Canvas.SetLeft(s.Element, s.X);

            if (s.X + s.Width < -10)
            {
                try { _stage.Children.Remove(s.Element); } catch { }
                _sprites.RemoveAt(i);
            }
        }
    }

    private void SpawnSprites()
    {
        if (_queue.Count == 0)
        {
            return;
        }

        var maxSpawn = (int)Math.Round(_density * 6.0);
        maxSpawn = Math.Clamp(maxSpawn, 0, 6);
        if (maxSpawn <= 0)
        {
            return;
        }

        var stageW = _stage.ActualWidth;
        var stageH = _stage.ActualHeight;

        var areaRatio = _area switch
        {
            DanmakuOverlayAreaMode.Quarter => 0.25,
            DanmakuOverlayAreaMode.Half => 0.5,
            DanmakuOverlayAreaMode.ThreeQuarter => 0.75,
            _ => 1.0,
        };

        var fontSize = 20.0 * _fontScale;
        var laneHeight = 32.0 * _fontScale;
        var gapPx = 40.0 * _fontScale;

        var availableHeight = stageH * areaRatio - TopPad - BottomPad;
        var laneCount = Math.Max(1, (int)Math.Floor(availableHeight / laneHeight));

        // Compute current lane tails (max right edge per lane) from existing sprites.
        var laneTail = new double[laneCount];
        for (var i = 0; i < laneCount; i++)
        {
            laneTail[i] = double.NegativeInfinity;
        }

        for (var i = 0; i < _sprites.Count; i++)
        {
            var sp = _sprites[i];
            var lane = sp.Lane;
            if ((uint)lane >= (uint)laneCount)
            {
                continue;
            }
            var tail = sp.X + sp.Width;
            if (tail > laneTail[lane])
            {
                laneTail[lane] = tail;
            }
        }

        var spawnRightEdge = stageW + 10;
        var canSpawnInAnyLane = false;
        for (var i = 0; i < laneCount; i++)
        {
            if (laneTail[i] < spawnRightEdge - gapPx)
            {
                canSpawnInAnyLane = true;
                break;
            }
        }
        if (!canSpawnInAnyLane)
        {
            return;
        }

        for (var n = 0; n < maxSpawn; n++)
        {
            if (_queue.Count == 0)
            {
                return;
            }

            var lane = FindAvailableLane(laneTail, spawnRightEdge - gapPx);
            if (lane < 0)
            {
                return;
            }

            var msg = _queue.Dequeue();
            SpawnOne(msg, lane, TopPad + lane * laneHeight, fontSize, stageW, out var spriteWidth, out var spriteSpeed);

            // Update lane tail for subsequent spawns in this tick.
            laneTail[lane] = spawnRightEdge + spriteWidth;
        }
    }

    private int FindAvailableLane(double[] laneTail, double maxTail)
    {
        if (laneTail.Length == 0)
        {
            return -1;
        }

        var laneCount = laneTail.Length;
        for (var i = 0; i < laneCount; i++)
        {
            var lane = (_laneCursor + i) % laneCount;
            if (laneTail[lane] < maxTail)
            {
                _laneCursor = (lane + 1) % laneCount;
                return lane;
            }
        }

        return -1;
    }

    private void SpawnOne(
        DanmakuMessage msg,
        int lane,
        double y,
        double fontSize,
        double stageWidth,
        out double spriteWidth,
        out double spriteSpeed
    )
    {
        var text = (msg.Text ?? "").Trim();
        var imageUrl = (msg.ImageUrl ?? "").Trim();

        var hasImage = !string.IsNullOrWhiteSpace(imageUrl);
        // If this is an image/emote message, suppress placeholder text like "[图片]" and only show the image.
        if (hasImage && DanmakuRowVm.IsImagePlaceholderText(text))
        {
            text = "";
        }

        var sp = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
        };

        TextBlock? tb = null;
        if (!string.IsNullOrWhiteSpace(text))
        {
            tb = new TextBlock
            {
                Text = text,
                Foreground = new SolidColorBrush(Colors.White),
                FontSize = fontSize,
                TextWrapping = TextWrapping.NoWrap,
            };
            // Mild shadow-like effect via duplicate outline is overkill; keep it lightweight.
            sp.Children.Add(tb);
        }

        Image? img = null;
        if (hasImage)
        {
            img = new Image
            {
                Width = 28 * _fontScale,
                Height = 28 * _fontScale,
                Stretch = Stretch.Uniform,
                Visibility = Visibility.Visible,
                Opacity = 0,
            };
            sp.Children.Add(img);
        }

        // Measure before adding to canvas so we can remove it when off-screen.
        sp.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        var width = Math.Max(60, sp.DesiredSize.Width);
        spriteWidth = width;

        var x = stageWidth + 10;
        Canvas.SetLeft(sp, x);
        Canvas.SetTop(sp, y);
        _stage.Children.Add(sp);

        // Use a near-constant on-screen duration so longer messages move faster and won't catch up easily.
        var durationSec = 8.0 + _rand.NextDouble() * 2.0; // 8~10s
        var speed = (stageWidth + width + 60) / Math.Max(1.0, durationSec);
        spriteSpeed = speed;

        _sprites.Add(new Sprite(sp, lane, x, y, width, speed));

        if (img is not null && _imgCts is not null)
        {
            _ = TryLoadImageAsync(msg.SessionId, imageUrl, img, _imgCts.Token);
        }
    }

    private async Task TryLoadImageAsync(string sessionId, string url, Image img, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        var u = (url ?? "").Trim();
        if (sid.Length == 0 || u.Length == 0)
        {
            return;
        }

        await _imgSem.WaitAsync(ct);
        try
        {
            var res = await _fetchImageAsync(sid, u, ct);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            var bmp = new BitmapImage();
            using var ms = new InMemoryRandomAccessStream();
            await ms.WriteAsync(bytes.AsBuffer());
            ms.Seek(0);
            await bmp.SetSourceAsync(ms);

            if (ct.IsCancellationRequested)
            {
                return;
            }

            // Timer tick runs on UI thread, but image fetch may complete later; ensure UI-thread update.
            if (_dq.HasThreadAccess)
            {
                img.Source = bmp;
                img.Opacity = 1;
            }
            else
            {
                _dq.TryEnqueue(() =>
                {
                    try
                    {
                        img.Source = bmp;
                        img.Opacity = 1;
                    }
                    catch
                    {
                        // ignore
                    }
                });
            }
        }
        catch (OperationCanceledException)
        {
            // ignore
        }
        catch
        {
            // ignore image failures
        }
        finally
        {
            try { _imgSem.Release(); } catch { }
        }
    }

    private bool IsRunning() => _active && _enabled;

    private void SyncStageVisibility()
    {
        try
        {
            _stage.Visibility = IsRunning() ? Visibility.Visible : Visibility.Collapsed;
        }
        catch
        {
            // ignore
        }
    }

    private static double Clamp01(double v)
    {
        if (double.IsNaN(v) || double.IsInfinity(v))
        {
            return 1.0;
        }
        return Math.Clamp(v, 0.0, 1.0);
    }
}
