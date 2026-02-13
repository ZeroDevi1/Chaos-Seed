using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.NativeOverlay;
using Microsoft.UI.Dispatching;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Windows;

// A Win32 layered window overlay for "true" transparency on Windows 11.
// This avoids WinUI3's limitations where XAML-level Transparent can still render as black.
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

    private DispatcherQueueTimer? _timer;
    private long _lastTickTs;

    private IntPtr _hwnd;
    private bool _locked = true; // locked == click-through except top grip; F2 toggles
    private bool _closed;

    private NativeLayeredSurface? _surface;
    private int _wPx;
    private int _hPx;

    private readonly Dictionary<string, Image> _imageCache = new(StringComparer.Ordinal);
    private readonly HashSet<string> _imageLoading = new(StringComparer.Ordinal);
    private Font? _font;
    private Font? _uiFont;

    // Increase grip width to make resizing easier to hit with the mouse.
    // User request: enlarge 2~4x so the resize cursor is easier to trigger.
    private const int ResizeGripPx = 32;
    private const int TopBarHeightPx = 32;
    private const int CloseBtnSizePx = 18;
    private const int CloseBtnPadPx = 8;
    private const int BorderThicknessPx = 6;
    private const int CornerRadiusPx = 12;

    // Danmaku font size (px). User request: 2x larger than the previous 20px.
    private const int DanmakuFontSizePx = 40;
    private const double LaneHeight = DanmakuFontSizePx + 24;
    private const double TopPad = 10;
    private const double BottomPad = 10;

    public NativeDanmakuOverlayWindow()
    {
        _wndProc = WndProcImpl;
        DanmakuService.Instance.Message += OnMsg;
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

        try { _font?.Dispose(); } catch { }
        _font = null;
        try { _uiFont?.Dispose(); } catch { }
        _uiFont = null;

        try
        {
            foreach (var kv in _imageCache)
            {
                try { kv.Value.Dispose(); } catch { }
            }
            _imageCache.Clear();
            _imageLoading.Clear();
        }
        catch
        {
            // ignore
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

        // Best-effort: if already registered, RegisterClassEx returns 0; ignore.
        _ = Win32Native.RegisterClassExW(ref wc);

        LoadBoundsOrDefault(out var x, out var y, out var w, out var h);

        // Show in taskbar so users can find/close it even if main window is hidden.
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

        // Initialize surface.
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
        SpawnSprites(maxSpawn: 6);
        RenderFrame();
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

    private void MoveSprites(double dt)
    {
        if (_wPx <= 1 || _hPx <= 1)
        {
            return;
        }

        for (var i = _sprites.Count - 1; i >= 0; i--)
        {
            var s = _sprites[i];
            s.X -= s.SpeedPxPerSec * dt;
            if (s.X + s.Width < -10)
            {
                _sprites.RemoveAt(i);
            }
        }
    }

    private void SpawnSprites(int maxSpawn)
    {
        if (_wPx <= 1 || _hPx <= 1)
        {
            return;
        }

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

        var availableHeight = _hPx - TopPad - BottomPad;
        var laneCount = Math.Max(1, (int)Math.Floor(availableHeight / LaneHeight));

        foreach (var msg in batch)
        {
            var lane = _laneCursor++ % laneCount;
            var y = TopPad + lane * LaneHeight;
            SpawnOne(msg, y);
        }
    }

    private int _laneCursor;

    private void SpawnOne(DanmakuMessage msg, double y)
    {
        var text = (msg.Text ?? "").Trim();
        var url = (msg.ImageUrl ?? "").Trim();
        if (text.Length == 0 && url.Length > 0)
        {
            text = "[图片]";
        }

        if (text.Length == 0 && url.Length == 0)
        {
            return;
        }

        var font = GetFont();
        var sid = (msg.SessionId ?? "").Trim();
        TryEnsureImageLoadingBestEffort(sid, url);

        var spacing = url.Length > 0 ? 6 : 0;
        var imgW = url.Length > 0 ? 28 : 0;
        var textSize = MeasureTextBestEffort(text, font);
        var width = Math.Max(60, imgW + spacing + textSize.Width);

        var x = _wPx + 10;
        var speed = 160 + _rand.NextDouble() * 80;

        _sprites.Add(new Sprite(sid, text, url, x, y, width, speed));
    }

    private Font GetFont()
    {
        // Keep it simple; can be made configurable later.
        return _font ??= new Font("Segoe UI", DanmakuFontSizePx, FontStyle.Regular, GraphicsUnit.Pixel);
    }

    private Font GetUiFont()
    {
        return _uiFont ??= new Font("Segoe UI", 12, FontStyle.Regular, GraphicsUnit.Pixel);
    }

    private static SizeF MeasureTextBestEffort(string text, Font font)
    {
        try
        {
            using var bmp = new Bitmap(1, 1, PixelFormat.Format32bppPArgb);
            using var g = Graphics.FromImage(bmp);
            g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
            using var fmt = new StringFormat(StringFormat.GenericTypographic);
            fmt.FormatFlags |= StringFormatFlags.MeasureTrailingSpaces;
            return g.MeasureString(text, font, int.MaxValue, fmt);
        }
        catch
        {
            return new SizeF(Math.Max(60, text.Length * 10), (float)font.Size);
        }
    }

    private void TryEnsureImageLoadingBestEffort(string sessionId, string url)
    {
        try
        {
            if (url.Length == 0)
            {
                return;
            }
            var sid = (sessionId ?? "").Trim();
            if (string.IsNullOrWhiteSpace(sid))
            {
                return;
            }

            // Cache by URL (good enough; backend returns stable URLs for emotes).
            lock (_imageCache)
            {
                if (_imageCache.TryGetValue(url, out var cached))
                {
                    _ = cached;
                    return;
                }

                if (_imageLoading.Contains(url))
                {
                    return;
                }

                _imageLoading.Add(url);
            }

            // Fire-and-forget async load; meanwhile render placeholder (text only).
            _ = TryLoadImageAsync(sid, url);
            return;
        }
        catch
        {
            return;
        }
    }

    private async Task TryLoadImageAsync(string sessionId, string url)
    {
        try
        {
            var sid = (sessionId ?? "").Trim();
            if (string.IsNullOrWhiteSpace(sid))
            {
                lock (_imageCache)
                {
                    _imageLoading.Remove(url);
                }
                return;
            }

            var res = await DanmakuService.Instance.FetchImageAsync(sid, url, CancellationToken.None);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                lock (_imageCache)
                {
                    _imageLoading.Remove(url);
                }
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            var img = await DecodeImageBestEffortAsync(bytes);
            if (img is null)
            {
                lock (_imageCache)
                {
                    _imageLoading.Remove(url);
                }
                return;
            }

            lock (_imageCache)
            {
                _imageLoading.Remove(url);
                if (_imageCache.ContainsKey(url))
                {
                    img.Dispose();
                    return;
                }
                _imageCache[url] = img;

                // Best-effort bound: emote URLs are effectively unbounded; keep memory usage in check.
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

        // Prefer WIC via WinRT BitmapDecoder so we can decode WebP on Win11 (System.Drawing can't).
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

            // Scale down to reduce memory; we draw emotes as 28x28 anyway.
            const uint targetMax = 64;
            var w = decoder.PixelWidth;
            var h = decoder.PixelHeight;
            var max = Math.Max(w, h);
            var scale = max > targetMax ? (double)targetMax / max : 1.0;
            var sw = (uint)Math.Max(1, Math.Round(w * scale));
            var sh = (uint)Math.Max(1, Math.Round(h * scale));

            var transform = new BitmapTransform { ScaledWidth = sw, ScaledHeight = sh };
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

            var bmp = new Bitmap((int)sw, (int)sh, PixelFormat.Format32bppPArgb);
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
            // Fall back to System.Drawing for formats it can decode.
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
            g.CompositingMode = System.Drawing.Drawing2D.CompositingMode.SourceOver;
            g.CompositingQuality = System.Drawing.Drawing2D.CompositingQuality.HighSpeed;
            g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.HighSpeed;
            g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBilinear;
            g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;

            g.Clear(Color.Transparent);

            using var brush = new SolidBrush(Color.White);
            using var shadow = new SolidBrush(Color.FromArgb(140, 0, 0, 0));
            using var topBarBg = new SolidBrush(Color.FromArgb(0x88, 0, 0, 0));
            using var borderPen = new Pen(
                Color.FromArgb(_locked ? 0xC0 : 0xE0, 255, 255, 255),
                BorderThicknessPx
            );
            borderPen.Alignment = PenAlignment.Inset;
            using var closePen = new Pen(Color.FromArgb(0xD0, 255, 255, 255), 2);
            closePen.Alignment = PenAlignment.Inset;

            var font = GetFont();
            using var fmt = new StringFormat(StringFormat.GenericTypographic);

            // Border + top bar (always visible so user can manage the window).
            // Rounded corners look closer to Win11.
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
            g.DrawString(hint, uiFont, brush, 10, 8, fmt);

            // Close button (top-right).
            var closeX = _wPx - CloseBtnPadPx - CloseBtnSizePx;
            var closeY = CloseBtnPadPx;
            g.DrawRectangle(borderPen, closeX, closeY, CloseBtnSizePx, CloseBtnSizePx);
            g.DrawLine(closePen, closeX + 4, closeY + 4, closeX + CloseBtnSizePx - 4, closeY + CloseBtnSizePx - 4);
            g.DrawLine(closePen, closeX + CloseBtnSizePx - 4, closeY + 4, closeX + 4, closeY + CloseBtnSizePx - 4);

            foreach (var s in _sprites)
            {
                var x = (float)s.X;
                var y = (float)s.Y;

                Image? img = null;
                if (!string.IsNullOrWhiteSpace(s.ImageUrl))
                {
                    // Best-effort: ensure loading continues even if cache got cleared.
                    if (!string.IsNullOrWhiteSpace(s.SessionId))
                    {
                        TryEnsureImageLoadingBestEffort(s.SessionId, s.ImageUrl);
                    }

                    lock (_imageCache)
                    {
                        _ = _imageCache.TryGetValue(s.ImageUrl, out img);
                    }

                    if (img is not null)
                    {
                        try
                        {
                            g.DrawImage(img, x, y - 2, 28, 28);
                            x += 28 + 6;
                        }
                        catch
                        {
                            // ignore
                        }
                    }
                }

                // Cheap "shadow" for readability over bright backgrounds.
                var drawText = s.Text;
                if (img is not null && DanmakuRowVm.IsImagePlaceholderText(drawText))
                {
                    drawText = "";
                }
                if (!string.IsNullOrEmpty(drawText))
                {
                    g.DrawString(drawText, font, shadow, x + 1, y + 1, fmt);
                    g.DrawString(drawText, font, brush, x, y, fmt);
                }
            }

            _surface?.Present();
        }
        catch
        {
            // ignore
        }
    }

    private static GraphicsPath CreateRoundRectPath(int x, int y, int w, int h, int r)
    {
        var rr = Math.Max(0, Math.Min(r, Math.Min(w, h) / 2));
        var d = rr * 2;
        var path = new GraphicsPath();
        if (rr == 0)
        {
            path.AddRectangle(new Rectangle(x, y, w, h));
            path.CloseFigure();
            return path;
        }

        path.AddArc(x, y, d, d, 180, 90);
        path.AddArc(x + w - d, y, d, d, 270, 90);
        path.AddArc(x + w - d, y + h - d, d, d, 0, 90);
        path.AddArc(x, y + h - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }

    private void LoadBoundsOrDefault(out int x, out int y, out int w, out int h)
    {
        var s = SettingsService.Instance.Current;

        w = s.DanmakuOverlayWidth is > 100 and < 10_000 ? s.DanmakuOverlayWidth.Value : 960;
        h = s.DanmakuOverlayHeight is > 100 and < 10_000 ? s.DanmakuOverlayHeight.Value : 540;

        x = s.DanmakuOverlayX is > -50_000 and < 50_000 ? s.DanmakuOverlayX.Value : 100;
        y = s.DanmakuOverlayY is > -50_000 and < 50_000 ? s.DanmakuOverlayY.Value : 100;
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

        var w = Math.Max(100, rc.Right - rc.Left);
        var h = Math.Max(100, rc.Bottom - rc.Top);

        SettingsService.Instance.Update(s =>
        {
            s.DanmakuOverlayX = rc.Left;
            s.DanmakuOverlayY = rc.Top;
            s.DanmakuOverlayWidth = w;
            s.DanmakuOverlayHeight = h;
        });
    }

    private sealed class Sprite
    {
        public Sprite(
            string sessionId,
            string text,
            string imageUrl,
            double x,
            double y,
            double width,
            double speedPxPerSec
        )
        {
            SessionId = sessionId;
            Text = text;
            ImageUrl = imageUrl;
            X = x;
            Y = y;
            Width = width;
            SpeedPxPerSec = speedPxPerSec;
        }

        public string SessionId { get; }
        public string Text { get; }
        public string ImageUrl { get; }
        public double X { get; set; }
        public double Y { get; }
        public double Width { get; }
        public double SpeedPxPerSec { get; }
    }

    // --- Win32 interop + WndProc plumbing ---
    private readonly Win32Native.WndProc _wndProc;

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
                    // Client coords for WM_LBUTTONDOWN.
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

                        // Only allow moving in EDIT (unlocked) mode.
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
                    var dx = Win32Native.GetXParam(lParam);
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
}
