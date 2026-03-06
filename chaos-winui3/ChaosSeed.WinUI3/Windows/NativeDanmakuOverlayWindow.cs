using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.NativeOverlay;
using Microsoft.UI.Dispatching;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Windows;

public sealed class NativeDanmakuOverlayWindow : IDisposable
{
    private static readonly NativeOverlayHitTest.Config _hitCfg = new(
        TopBarHeightPx: TopBarHeightPx,
        CloseBtnSizePx: CloseBtnSizePx,
        CloseBtnPadPx: CloseBtnPadPx,
        ResizeGripPx: ResizeGripPx
    );

    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private readonly object _queueGate = new();
    private readonly Queue<DanmakuMessage> _queue = new();
    private readonly List<Sprite> _sprites = new();
    private readonly Random _rand = new();
    private readonly Dictionary<string, Image> _imageCache = new(StringComparer.Ordinal);
    private readonly HashSet<string> _imageLoading = new(StringComparer.Ordinal);
    private readonly NativeOverlayTextLayout _textLayout = new();
    private readonly NativeOverlayColorTextRenderer _colorTextRenderer = new();
    private readonly Win32Native.WndProc _wndProc;

    private DispatcherQueueTimer? _timer;
    private long _lastTickTs;
    private IntPtr _hwnd;
    private bool _locked = true;
    private bool _closed;
    private NativeLayeredSurface? _surface;
    private int _wPx;
    private int _hPx;
    private Font? _uiFont;
    private int _laneCursor;

    private bool _enabled = true;
    private float _opacity = 1f;
    private float _fontScale = 1f;
    private double _density = 1.0;
    private DanmakuOverlayAreaMode _area = DanmakuOverlayAreaMode.Full;

    private const int ResizeGripPx = 32;
    private const int TopBarHeightPx = 32;
    private const int CloseBtnSizePx = 18;
    private const int CloseBtnPadPx = 8;
    private const int BorderThicknessPx = 6;
    private const int CornerRadiusPx = 12;
    private const float TopPadPx = 10f;
    private const float BottomPadPx = 10f;
    private const float ContentSpacingPx = 6f;
    private const int MaxQueuedMessages = 1000;
    private const int QueuedTrimTo = 200;

    public NativeDanmakuOverlayWindow()
    {
        _wndProc = WndProcImpl;
        DanmakuService.Instance.Message += OnMsg;
        SettingsService.Instance.SettingsChanged += OnSettingsChanged;
        ApplySettings(SettingsService.Instance.Current);
    }

    public event EventHandler? Closed;

    public void Show()
    {
        if (_closed)
        {
            return;
        }

        if (_hwnd != IntPtr.Zero)
        {
            try
            {
                Win32Native.ShowWindow(_hwnd, Win32Native.SW_SHOWNOACTIVATE);
                Win32OverlayInterop.SetTopmost(_hwnd, true);
                RenderFrame();
            }
            catch
            {
                // ignore
            }
            return;
        }

        CreateWindow();
        StartLoop();
    }

    public void Close()
    {
        if (_closed)
        {
            return;
        }

        _closed = true;
        Dispose();
    }

    public void Dispose()
    {
        SettingsService.Instance.SettingsChanged -= OnSettingsChanged;
        DanmakuService.Instance.Message -= OnMsg;

        try { _timer?.Stop(); } catch { }
        _timer = null;

        try { SaveBoundsBestEffort(); } catch { }

        try
        {
            if (_hwnd != IntPtr.Zero)
            {
                Win32Native.DestroyWindow(_hwnd);
            }
        }
        catch
        {
            // ignore
        }
        _hwnd = IntPtr.Zero;

        try { _surface?.Dispose(); } catch { }
        _surface = null;
        _wPx = 0;
        _hPx = 0;

        try { _uiFont?.Dispose(); } catch { }
        _uiFont = null;
        try { _textLayout.Dispose(); } catch { }
        try { _colorTextRenderer.Dispose(); } catch { }

        ClearDanmakuState();

        lock (_imageCache)
        {
            foreach (var kv in _imageCache)
            {
                try { kv.Value.Dispose(); } catch { }
            }
            _imageCache.Clear();
            _imageLoading.Clear();
        }

        try
        {
            Closed?.Invoke(this, EventArgs.Empty);
        }
        catch
        {
            // ignore
        }
    }

    private void OnSettingsChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;

        var settings = SettingsService.Instance.Current;
        if (_dq.HasThreadAccess)
        {
            ApplySettings(settings);
            return;
        }

        try
        {
            _dq.TryEnqueue(() => ApplySettings(settings));
        }
        catch
        {
            // ignore
        }
    }

    private void ApplySettings(AppSettings settings)
    {
        var nextEnabled = settings?.DanmakuOverlayEnabled ?? true;
        var nextOpacity = (float)Clamp01(settings?.DanmakuOverlayOpacity ?? 1.0);
        var nextFontScale = Math.Clamp(
            (float)(settings?.DanmakuOverlayFontScale ?? 1.0),
            NativeOverlayMetricsCalculator.MinFontScale,
            NativeOverlayMetricsCalculator.MaxFontScale
        );
        var nextDensity = Clamp01(settings?.DanmakuOverlayDensity ?? 1.0);
        var nextArea = settings?.DanmakuOverlayArea ?? DanmakuOverlayAreaMode.Full;

        var needClear = !nextEnabled
            || Math.Abs(nextFontScale - _fontScale) > 0.001f
            || Math.Abs(nextDensity - _density) > 0.001
            || nextArea != _area;

        _enabled = nextEnabled;
        _opacity = nextOpacity;
        _fontScale = nextFontScale;
        _density = nextDensity;
        _area = nextArea;

        if (needClear)
        {
            ClearDanmakuState();
        }

        RenderFrame();
    }

    private void CreateWindow()
    {
        var hInstance = Win32Native.GetModuleHandleW(IntPtr.Zero);

        var wc = new Win32Native.WNDCLASSEXW
        {
            cbSize = (uint)Marshal.SizeOf<Win32Native.WNDCLASSEXW>(),
            style = Win32Native.CS_DBLCLKS,
            lpfnWndProc = Marshal.GetFunctionPointerForDelegate(_wndProc),
            cbClsExtra = 0,
            cbWndExtra = 0,
            hInstance = hInstance,
            hIcon = IntPtr.Zero,
            hCursor = Win32Native.LoadCursorW(IntPtr.Zero, (IntPtr)Win32Native.IDC_ARROW),
            hbrBackground = IntPtr.Zero,
            lpszMenuName = IntPtr.Zero,
            lpszClassName = "ChaosSeed.NativeDanmakuOverlay",
            hIconSm = IntPtr.Zero,
        };

        _ = Win32Native.RegisterClassExW(ref wc);

        LoadBoundsOrDefault(out var x, out var y, out var w, out var h);

        var exStyle = Win32Native.WS_EX_LAYERED | Win32Native.WS_EX_APPWINDOW;
        var style = Win32Native.WS_POPUP;

        _hwnd = Win32Native.CreateWindowExW(
            exStyle,
            wc.lpszClassName,
            "Overlay",
            style,
            x,
            y,
            w,
            h,
            IntPtr.Zero,
            IntPtr.Zero,
            hInstance,
            IntPtr.Zero
        );

        if (_hwnd == IntPtr.Zero)
        {
            throw new InvalidOperationException("CreateWindowExW failed.");
        }

        Win32OverlayInterop.TryEnableWin11WindowChrome(_hwnd);
        Win32OverlayInterop.SetTopmost(_hwnd, true);

        Win32Native.ShowWindow(_hwnd, Win32Native.SW_SHOWNOACTIVATE);
        Win32Native.UpdateWindow(_hwnd);

        _surface = new NativeLayeredSurface(_hwnd);
        _surface.Resize(w, h);
        _wPx = w;
        _hPx = h;
        RenderFrame();
    }

    private void StartLoop()
    {
        if (_timer is not null)
        {
            return;
        }

        _timer = _dq.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(16);
        _timer.IsRepeating = true;
        _timer.Tick += (_, _) => Tick();
        _lastTickTs = Stopwatch.GetTimestamp();
        _timer.Start();
    }

    private void Tick()
    {
        if (_closed || _hwnd == IntPtr.Zero)
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
        if (_enabled)
        {
            SpawnSprites();
        }
        RenderFrame();
    }

    private void OnMsg(object? sender, DanmakuMessage msg)
    {
        _ = sender;
        if (msg is null || !_enabled)
        {
            return;
        }

        var text = (msg.Text ?? string.Empty).Trim();
        var url = (msg.ImageUrl ?? string.Empty).Trim();
        if (text.Length == 0 && url.Length == 0)
        {
            return;
        }

        lock (_queueGate)
        {
            _queue.Enqueue(msg);
            if (_queue.Count > MaxQueuedMessages)
            {
                while (_queue.Count > QueuedTrimTo)
                {
                    _queue.Dequeue();
                }
            }
        }
    }

    private void ClearDanmakuState()
    {
        lock (_queueGate)
        {
            _queue.Clear();
        }

        foreach (var sprite in _sprites)
        {
            DisposeSprite(sprite);
        }

        _sprites.Clear();
        _laneCursor = 0;
    }

    private void MoveSprites(double dt)
    {
        if (_wPx <= 1 || _hPx <= 1)
        {
            return;
        }

        for (var i = _sprites.Count - 1; i >= 0; i--)
        {
            var sprite = _sprites[i];
            sprite.X -= sprite.SpeedPxPerSec * dt;
            if (sprite.X + sprite.Width < -10)
            {
                DisposeSprite(sprite);
                _sprites.RemoveAt(i);
            }
        }
    }

    private void SpawnSprites()
    {
        if (_wPx <= 1 || _hPx <= 1)
        {
            return;
        }

        var metrics = GetMetrics();
        if (metrics.MaxSpawn <= 0)
        {
            return;
        }

        lock (_queueGate)
        {
            if (_queue.Count == 0)
            {
                return;
            }
        }

        var laneCount = NativeOverlayLaneScheduler.ComputeLaneCount(
            _hPx,
            metrics.AreaRatio,
            metrics.LaneHeightPx,
            TopPadPx,
            BottomPadPx
        );
        var laneTail = new double[laneCount];
        for (var i = 0; i < laneTail.Length; i++)
        {
            laneTail[i] = double.NegativeInfinity;
        }

        for (var i = 0; i < _sprites.Count; i++)
        {
            var sprite = _sprites[i];
            if ((uint)sprite.Lane >= (uint)laneTail.Length)
            {
                continue;
            }

            var tail = sprite.X + sprite.Width;
            if (tail > laneTail[sprite.Lane])
            {
                laneTail[sprite.Lane] = tail;
            }
        }

        var spawnRightEdge = _wPx + 10.0;
        var canSpawnInAnyLane = false;
        for (var i = 0; i < laneTail.Length; i++)
        {
            if (laneTail[i] < spawnRightEdge - metrics.GapPx)
            {
                canSpawnInAnyLane = true;
                break;
            }
        }

        if (!canSpawnInAnyLane)
        {
            return;
        }

        for (var n = 0; n < metrics.MaxSpawn; n++)
        {
            DanmakuMessage? msg;
            lock (_queueGate)
            {
                if (_queue.Count == 0)
                {
                    return;
                }

                msg = _queue.Peek();
            }

            var lane = NativeOverlayLaneScheduler.FindAvailableLane(
                laneTail,
                spawnRightEdge - metrics.GapPx,
                _laneCursor,
                out var nextLaneCursor
            );
            if (lane < 0)
            {
                return;
            }

            if (!TryCreateSprite(msg!, lane, metrics, out var sprite))
            {
                lock (_queueGate)
                {
                    if (_queue.Count > 0)
                    {
                        _queue.Dequeue();
                    }
                }
                continue;
            }

            lock (_queueGate)
            {
                if (_queue.Count > 0)
                {
                    _queue.Dequeue();
                }
            }

            _laneCursor = nextLaneCursor;
            _sprites.Add(sprite!);
            laneTail[lane] = spawnRightEdge + sprite!.Width;
        }
    }

    private bool TryCreateSprite(DanmakuMessage msg, int lane, NativeOverlayMetrics metrics, out Sprite? sprite)
    {
        sprite = null;

        var text = (msg.Text ?? string.Empty).Trim();
        var imageUrl = (msg.ImageUrl ?? string.Empty).Trim();
        var hasImage = !string.IsNullOrWhiteSpace(imageUrl);
        if (hasImage && DanmakuRowVm.IsImagePlaceholderText(text))
        {
            text = string.Empty;
        }

        if (text.Length == 0 && imageUrl.Length == 0)
        {
            return false;
        }

        NativeOverlayTextLayout.Layout? layout = null;
        Bitmap? textBitmap = null;
        if (text.Length > 0)
        {
            textBitmap = _colorTextRenderer.TryRender(text, metrics.FontSizePx);
            if (textBitmap is null)
            {
                layout = _textLayout.CreateLayout(text, metrics.FontSizePx);
            }
        }

        var textWidth = textBitmap?.Width ?? layout?.Size.Width ?? 0f;
        var textHeight = textBitmap?.Height ?? layout?.Size.Height ?? 0f;
        var imageSize = hasImage ? metrics.ImageSizePx : 0f;
        var spacing = textWidth > 0f && imageSize > 0f ? ContentSpacingPx : 0f;
        var contentHeight = Math.Max(textHeight, imageSize);
        var width = Math.Max(60f, textWidth + imageSize + spacing);
        var laneTop = TopPadPx + (float)(lane * metrics.LaneHeightPx);
        var textTopOffset = textWidth > 0f ? (contentHeight - textHeight) / 2f : 0f;
        var imageTopOffset = imageSize > 0f ? (contentHeight - imageSize) / 2f : 0f;
        var speed = (_wPx + width + 60f) / Math.Max(1.0, 8.0 + _rand.NextDouble() * 2.0);
        var sessionId = (msg.SessionId ?? string.Empty).Trim();

        TryEnsureImageLoadingBestEffort(sessionId, imageUrl);

        sprite = new Sprite(
            sessionId,
            imageUrl,
            layout,
            textBitmap,
            lane,
            _wPx + 10.0,
            laneTop,
            width,
            speed,
            metrics.LaneHeightPx,
            contentHeight,
            imageSize,
            textTopOffset,
            imageTopOffset,
            spacing,
            metrics.FontSizePx
        );
        return true;
    }

    private void TryEnsureImageLoadingBestEffort(string sessionId, string url)
    {
        if (string.IsNullOrWhiteSpace(sessionId) || string.IsNullOrWhiteSpace(url))
        {
            return;
        }

        lock (_imageCache)
        {
            if (_imageCache.ContainsKey(url) || _imageLoading.Contains(url))
            {
                return;
            }

            _imageLoading.Add(url);
        }

        _ = TryLoadImageAsync(sessionId, url);
    }

    private async Task TryLoadImageAsync(string sessionId, string url)
    {
        try
        {
            var sid = (sessionId ?? string.Empty).Trim();
            var targetUrl = (url ?? string.Empty).Trim();
            if (sid.Length == 0 || targetUrl.Length == 0)
            {
                return;
            }

            var res = await DanmakuService.Instance.FetchImageAsync(sid, targetUrl, CancellationToken.None);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            var img = await DecodeImageBestEffortAsync(bytes);
            if (img is null)
            {
                return;
            }

            lock (_imageCache)
            {
                if (_imageCache.ContainsKey(targetUrl))
                {
                    img.Dispose();
                    return;
                }

                _imageCache[targetUrl] = img;
                if (_imageCache.Count > 512)
                {
                    foreach (var kv in _imageCache)
                    {
                        try { kv.Value.Dispose(); } catch { }
                    }
                    _imageCache.Clear();
                    _imageLoading.Clear();
                }
            }
        }
        catch
        {
            // ignore
        }
        finally
        {
            lock (_imageCache)
            {
                _imageLoading.Remove(url);
            }
        }
    }

    private static async Task<Bitmap?> DecodeImageBestEffortAsync(byte[] bytes)
    {
        if (bytes.Length == 0)
        {
            return null;
        }

        try
        {
            using var ms = new InMemoryRandomAccessStream();
            await ms.WriteAsync(bytes.AsBuffer());
            ms.Seek(0);

            var decoder = await BitmapDecoder.CreateAsync(ms);
            if (decoder.PixelWidth == 0 || decoder.PixelHeight == 0)
            {
                return null;
            }

            const uint targetMax = 192;
            var width = decoder.PixelWidth;
            var height = decoder.PixelHeight;
            var max = Math.Max(width, height);
            var scale = max > targetMax ? (double)targetMax / max : 1.0;
            var scaledWidth = (uint)Math.Max(1, Math.Round(width * scale));
            var scaledHeight = (uint)Math.Max(1, Math.Round(height * scale));

            var transform = new BitmapTransform { ScaledWidth = scaledWidth, ScaledHeight = scaledHeight };
            var pixelData = await decoder.GetPixelDataAsync(
                BitmapPixelFormat.Bgra8,
                BitmapAlphaMode.Premultiplied,
                transform,
                ExifOrientationMode.IgnoreExifOrientation,
                ColorManagementMode.DoNotColorManage
            );

            var pixels = pixelData.DetachPixelData();
            if (pixels is null || pixels.Length == 0)
            {
                return null;
            }

            var bmp = new Bitmap((int)scaledWidth, (int)scaledHeight, PixelFormat.Format32bppPArgb);
            var rect = new Rectangle(0, 0, bmp.Width, bmp.Height);
            var data = bmp.LockBits(rect, ImageLockMode.WriteOnly, PixelFormat.Format32bppPArgb);
            try
            {
                Marshal.Copy(pixels, 0, data.Scan0, Math.Min(pixels.Length, Math.Abs(data.Stride) * data.Height));
            }
            finally
            {
                bmp.UnlockBits(data);
            }

            return bmp;
        }
        catch
        {
            // ignore and fall back to System.Drawing
        }

        try
        {
            using var ms = new MemoryStream(bytes);
            using var tmp = Image.FromStream(ms);
            return new Bitmap(tmp);
        }
        catch
        {
            return null;
        }
    }

    private void RenderFrame()
    {
        var bmp = _surface?.Bitmap;
        if (_hwnd == IntPtr.Zero || bmp is null || _wPx <= 1 || _hPx <= 1)
        {
            return;
        }

        try
        {
            using var g = Graphics.FromImage(bmp);
            g.CompositingMode = CompositingMode.SourceOver;
            g.CompositingQuality = CompositingQuality.HighSpeed;
            g.SmoothingMode = SmoothingMode.HighSpeed;
            g.InterpolationMode = InterpolationMode.HighQualityBilinear;
            g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
            g.Clear(Color.Transparent);

            using var frameBrush = new SolidBrush(Color.White);
            using var topBarBg = new SolidBrush(Color.FromArgb(0x88, 0, 0, 0));
            using var borderPen = new Pen(
                Color.FromArgb(_locked ? 0xC0 : 0xE0, 255, 255, 255),
                BorderThicknessPx
            );
            borderPen.Alignment = PenAlignment.Inset;
            using var closePen = new Pen(Color.FromArgb(0xD0, 255, 255, 255), 2);
            closePen.Alignment = PenAlignment.Inset;
            using var stringFormat = new StringFormat(StringFormat.GenericTypographic);
            stringFormat.FormatFlags |= StringFormatFlags.MeasureTrailingSpaces;

            var oldSmooth = g.SmoothingMode;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            using (var outer = CreateRoundRectPath(0, 0, _wPx - 1, _hPx - 1, CornerRadiusPx))
            {
                g.DrawPath(borderPen, outer);
            }
            g.SmoothingMode = oldSmooth;
            g.FillRectangle(topBarBg, 0, 0, _wPx, TopBarHeightPx);

            var uiFont = GetUiFont();
            var mode = _locked ? "LOCK" : "EDIT";
            var hint = $"Overlay ({mode})  F2 Toggle  Esc Close";
            g.DrawString(hint, uiFont, frameBrush, 10, 8, stringFormat);

            var closeX = _wPx - CloseBtnPadPx - CloseBtnSizePx;
            var closeY = CloseBtnPadPx;
            g.DrawRectangle(borderPen, closeX, closeY, CloseBtnSizePx, CloseBtnSizePx);
            g.DrawLine(closePen, closeX + 4, closeY + 4, closeX + CloseBtnSizePx - 4, closeY + CloseBtnSizePx - 4);
            g.DrawLine(closePen, closeX + CloseBtnSizePx - 4, closeY + 4, closeX + 4, closeY + CloseBtnSizePx - 4);

            var alpha = Math.Clamp((int)Math.Round(_opacity * 255f), 0, 255);
            if (alpha <= 0)
            {
                _surface?.Present();
                return;
            }

            using var textBrush = new SolidBrush(Color.FromArgb(alpha, 255, 255, 255));
            using var shadowBrush = new SolidBrush(Color.FromArgb(Math.Min(255, (int)Math.Round(alpha * 0.55f)), 0, 0, 0));
            using var imageAttributes = CreateContentImageAttributes(_opacity);

            foreach (var sprite in _sprites)
            {
                var drawX = (float)sprite.X;
                var contentTop = (float)(sprite.Y + Math.Max(0f, (sprite.LaneHeight - sprite.ContentHeight) / 2f));

                if (sprite.ImageSize > 0f && !string.IsNullOrWhiteSpace(sprite.ImageUrl))
                {
                    if (!TryGetCachedImage(sprite.ImageUrl, out var img) && sprite.SessionId.Length > 0)
                    {
                        TryEnsureImageLoadingBestEffort(sprite.SessionId, sprite.ImageUrl);
                        _ = TryGetCachedImage(sprite.ImageUrl, out img);
                    }

                    if (img is not null)
                    {
                        try
                        {
                            var dest = new RectangleF(
                                drawX,
                                contentTop + sprite.ImageTopOffset,
                                sprite.ImageSize,
                                sprite.ImageSize
                            );
                            g.DrawImage(
                                img,
                                Rectangle.Round(dest),
                                0,
                                0,
                                img.Width,
                                img.Height,
                                GraphicsUnit.Pixel,
                                imageAttributes
                            );
                            drawX += sprite.ImageSize + sprite.SpacingAfterImage;
                        }
                        catch
                        {
                            // ignore
                        }
                    }
                }

                if (sprite.TextBitmap is not null)
                {
                    try
                    {
                        var dest = new RectangleF(
                            drawX,
                            contentTop + sprite.TextTopOffset,
                            sprite.TextBitmap.Width,
                            sprite.TextBitmap.Height
                        );
                        g.DrawImage(
                            sprite.TextBitmap,
                            Rectangle.Round(dest),
                            0,
                            0,
                            sprite.TextBitmap.Width,
                            sprite.TextBitmap.Height,
                            GraphicsUnit.Pixel,
                            imageAttributes
                        );
                    }
                    catch
                    {
                        // ignore
                    }
                }
                else if (sprite.TextLayout is not null)
                {
                    _textLayout.DrawLayout(
                        g,
                        sprite.TextLayout,
                        shadowBrush,
                        textBrush,
                        stringFormat,
                        drawX,
                        contentTop + sprite.TextTopOffset,
                        sprite.FontSizePx
                    );
                }
            }

            _surface?.Present();
        }
        catch
        {
            // ignore
        }
    }

    private NativeOverlayMetrics GetMetrics()
    {
        return NativeOverlayMetricsCalculator.FromValues(_fontScale, _density, _area);
    }

    private bool TryGetCachedImage(string imageUrl, out Image? image)
    {
        lock (_imageCache)
        {
            return _imageCache.TryGetValue(imageUrl, out image);
        }
    }

    private static ImageAttributes CreateContentImageAttributes(float opacity)
    {
        var imageAttributes = new ImageAttributes();
        var matrix = new ColorMatrix
        {
            Matrix00 = 1f,
            Matrix11 = 1f,
            Matrix22 = 1f,
            Matrix33 = Math.Clamp(opacity, 0f, 1f),
            Matrix44 = 1f,
        };
        imageAttributes.SetColorMatrix(matrix, ColorMatrixFlag.Default, ColorAdjustType.Bitmap);
        return imageAttributes;
    }

    private static void DisposeSprite(Sprite sprite)
    {
        try { sprite.TextBitmap?.Dispose(); } catch { }
    }

    private Font GetUiFont()
    {
        return _uiFont ??= new Font("Segoe UI", 12, FontStyle.Regular, GraphicsUnit.Pixel);
    }

    private static GraphicsPath CreateRoundRectPath(int x, int y, int w, int h, int r)
    {
        var radius = Math.Max(0, Math.Min(r, Math.Min(w, h) / 2));
        var diameter = radius * 2;
        var path = new GraphicsPath();
        if (radius == 0)
        {
            path.AddRectangle(new Rectangle(x, y, w, h));
            path.CloseFigure();
            return path;
        }

        path.AddArc(x, y, diameter, diameter, 180, 90);
        path.AddArc(x + w - diameter, y, diameter, diameter, 270, 90);
        path.AddArc(x + w - diameter, y + h - diameter, diameter, diameter, 0, 90);
        path.AddArc(x, y + h - diameter, diameter, diameter, 90, 90);
        path.CloseFigure();
        return path;
    }

    private void LoadBoundsOrDefault(out int x, out int y, out int w, out int h)
    {
        var settings = SettingsService.Instance.Current;
        w = settings.DanmakuOverlayWidth is > 100 and < 10_000 ? settings.DanmakuOverlayWidth.Value : 960;
        h = settings.DanmakuOverlayHeight is > 100 and < 10_000 ? settings.DanmakuOverlayHeight.Value : 540;
        x = settings.DanmakuOverlayX is > -50_000 and < 50_000 ? settings.DanmakuOverlayX.Value : 100;
        y = settings.DanmakuOverlayY is > -50_000 and < 50_000 ? settings.DanmakuOverlayY.Value : 100;
    }

    private void SaveBoundsBestEffort()
    {
        if (_hwnd == IntPtr.Zero)
        {
            return;
        }

        if (!Win32Native.GetWindowRect(_hwnd, out var rc))
        {
            return;
        }

        var width = Math.Max(100, rc.Right - rc.Left);
        var height = Math.Max(100, rc.Bottom - rc.Top);
        SettingsService.Instance.Update(s =>
        {
            s.DanmakuOverlayX = rc.Left;
            s.DanmakuOverlayY = rc.Top;
            s.DanmakuOverlayWidth = width;
            s.DanmakuOverlayHeight = height;
        });
    }

    private sealed class Sprite
    {
        public Sprite(
            string sessionId,
            string imageUrl,
            NativeOverlayTextLayout.Layout? textLayout,
            Bitmap? textBitmap,
            int lane,
            double x,
            float y,
            float width,
            double speedPxPerSec,
            float laneHeight,
            float contentHeight,
            float imageSize,
            float textTopOffset,
            float imageTopOffset,
            float spacingAfterImage,
            float fontSizePx
        )
        {
            SessionId = sessionId;
            ImageUrl = imageUrl;
            TextLayout = textLayout;
            TextBitmap = textBitmap;
            Lane = lane;
            X = x;
            Y = y;
            Width = width;
            SpeedPxPerSec = speedPxPerSec;
            LaneHeight = laneHeight;
            ContentHeight = contentHeight;
            ImageSize = imageSize;
            TextTopOffset = textTopOffset;
            ImageTopOffset = imageTopOffset;
            SpacingAfterImage = spacingAfterImage;
            FontSizePx = fontSizePx;
        }

        public string SessionId { get; }
        public string ImageUrl { get; }
        public NativeOverlayTextLayout.Layout? TextLayout { get; }
        public Bitmap? TextBitmap { get; }
        public int Lane { get; }
        public double X { get; set; }
        public float Y { get; }
        public float Width { get; }
        public double SpeedPxPerSec { get; }
        public float LaneHeight { get; }
        public float ContentHeight { get; }
        public float ImageSize { get; }
        public float TextTopOffset { get; }
        public float ImageTopOffset { get; }
        public float SpacingAfterImage { get; }
        public float FontSizePx { get; }
    }

    private IntPtr WndProcImpl(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam)
    {
        switch (msg)
        {
            case Win32Native.WM_NCDESTROY:
                _hwnd = IntPtr.Zero;
                return Win32Native.DefWindowProcW(hwnd, msg, wParam, lParam);

            case Win32Native.WM_SIZE:
                {
                    var w = (int)(lParam.ToInt64() & 0xFFFF);
                    var h = (int)((lParam.ToInt64() >> 16) & 0xFFFF);
                    if (w > 0 && h > 0)
                    {
                        try { _surface?.Resize(w, h); } catch { }
                        _wPx = w;
                        _hPx = h;
                        ClearDanmakuState();
                        RenderFrame();
                    }
                    return IntPtr.Zero;
                }

            case Win32Native.WM_KEYDOWN:
                {
                    var vk = (int)wParam;
                    if (vk == Win32Native.VK_ESCAPE)
                    {
                        Close();
                        return IntPtr.Zero;
                    }
                    if (vk == Win32Native.VK_F2)
                    {
                        _locked = !_locked;
                        RenderFrame();
                        return IntPtr.Zero;
                    }
                    break;
                }

            case Win32Native.WM_NCHITTEST:
                {
                    var x = Win32Native.GetXParam(lParam);
                    var y = Win32Native.GetYParam(lParam);
                    if (Win32Native.GetWindowRect(hwnd, out var rc))
                    {
                        var dx = x - rc.Left;
                        var dy = y - rc.Top;
                        var wPx = Math.Max(1, rc.Right - rc.Left);
                        var hPx = Math.Max(1, rc.Bottom - rc.Top);
                        var region = NativeOverlayHitTest.HitTest(wPx, hPx, dx, dy, _locked, _hitCfg);

                        return region switch
                        {
                            NativeOverlayHitTest.Region.PassThrough => (IntPtr)Win32Native.HTTRANSPARENT,
                            NativeOverlayHitTest.Region.ResizeLeft => (IntPtr)Win32Native.HTLEFT,
                            NativeOverlayHitTest.Region.ResizeRight => (IntPtr)Win32Native.HTRIGHT,
                            NativeOverlayHitTest.Region.ResizeTop => (IntPtr)Win32Native.HTTOP,
                            NativeOverlayHitTest.Region.ResizeBottom => (IntPtr)Win32Native.HTBOTTOM,
                            NativeOverlayHitTest.Region.ResizeTopLeft => (IntPtr)Win32Native.HTTOPLEFT,
                            NativeOverlayHitTest.Region.ResizeTopRight => (IntPtr)Win32Native.HTTOPRIGHT,
                            NativeOverlayHitTest.Region.ResizeBottomLeft => (IntPtr)Win32Native.HTBOTTOMLEFT,
                            NativeOverlayHitTest.Region.ResizeBottomRight => (IntPtr)Win32Native.HTBOTTOMRIGHT,
                            _ => (IntPtr)Win32Native.HTCLIENT,
                        };
                    }

                    return (IntPtr)Win32Native.HTCLIENT;
                }

            case Win32Native.WM_LBUTTONDOWN:
                {
                    var dx = Win32Native.GetXParam(lParam);
                    var dy = Win32Native.GetYParam(lParam);
                    var wPx = _wPx;
                    var hPx = _hPx;
                    if (wPx <= 1 || hPx <= 1)
                    {
                        if (Win32Native.GetWindowRect(hwnd, out var rc))
                        {
                            wPx = Math.Max(1, rc.Right - rc.Left);
                            hPx = Math.Max(1, rc.Bottom - rc.Top);
                        }
                    }

                    if (dy >= 0 && dy < TopBarHeightPx)
                    {
                        var close = NativeOverlayHitTest.CloseButtonRect(wPx, _hitCfg);
                        if (close.Contains(dx, dy))
                        {
                            Close();
                            return IntPtr.Zero;
                        }

                        if (!_locked)
                        {
                            try
                            {
                                _ = Win32Native.ReleaseCapture();
                                _ = Win32Native.SendMessageW(
                                    hwnd,
                                    Win32Native.WM_NCLBUTTONDOWN,
                                    (IntPtr)Win32Native.HTCAPTION,
                                    IntPtr.Zero
                                );
                            }
                            catch
                            {
                                // ignore
                            }
                        }
                        return IntPtr.Zero;
                    }

                    break;
                }

            case Win32Native.WM_LBUTTONDBLCLK:
                {
                    var dy = Win32Native.GetYParam(lParam);
                    if (dy >= 0 && dy < TopBarHeightPx)
                    {
                        _locked = !_locked;
                        RenderFrame();
                        return IntPtr.Zero;
                    }
                    break;
                }

            case Win32Native.WM_CLOSE:
                Close();
                return IntPtr.Zero;
        }

        return Win32Native.DefWindowProcW(hwnd, msg, wParam, lParam);
    }

    private static double Clamp01(double value)
    {
        if (double.IsNaN(value) || double.IsInfinity(value))
        {
            return 1.0;
        }

        return Math.Clamp(value, 0.0, 1.0);
    }
}
