using System.Drawing;
using System.Drawing.Imaging;

namespace ChaosSeed.WinUI3.Services.NativeOverlay;

internal sealed class NativeLayeredSurface : IDisposable
{
    private readonly IntPtr _hwnd;

    private int _wPx;
    private int _hPx;

    private IntPtr _memDc;
    private IntPtr _hbm;
    private IntPtr _hbmOld;
    private IntPtr _bits;
    private Bitmap? _bitmap;

    public NativeLayeredSurface(IntPtr hwnd)
    {
        _hwnd = hwnd;
    }

    public int WidthPx => _wPx;
    public int HeightPx => _hPx;
    public Bitmap? Bitmap => _bitmap;

    public void Resize(int w, int h)
    {
        w = Math.Clamp(w, 100, 10_000);
        h = Math.Clamp(h, 100, 10_000);

        if (w == _wPx && h == _hPx && _bitmap is not null && _memDc != IntPtr.Zero && _hbm != IntPtr.Zero)
        {
            return;
        }

        Destroy();

        _wPx = w;
        _hPx = h;

        var screenDc = Win32Native.GetDC(IntPtr.Zero);
        try
        {
            _memDc = Win32Native.CreateCompatibleDC(screenDc);
        }
        finally
        {
            _ = Win32Native.ReleaseDC(IntPtr.Zero, screenDc);
        }

        var bmi = new Win32Native.BITMAPINFO();
        bmi.bmiHeader.biSize = (uint)System.Runtime.InteropServices.Marshal.SizeOf<Win32Native.BITMAPINFOHEADER>();
        bmi.bmiHeader.biWidth = _wPx;
        bmi.bmiHeader.biHeight = -_hPx; // top-down DIB
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = Win32Native.BI_RGB;

        _hbm = Win32Native.CreateDIBSection(
            _memDc,
            ref bmi,
            Win32Native.DIB_RGB_COLORS,
            out _bits,
            IntPtr.Zero,
            0
        );

        if (_hbm == IntPtr.Zero || _bits == IntPtr.Zero)
        {
            Destroy();
            return;
        }

        _hbmOld = Win32Native.SelectObject(_memDc, _hbm);

        // Stride is width * 4 for BI_RGB 32bpp.
        _bitmap = new Bitmap(_wPx, _hPx, _wPx * 4, PixelFormat.Format32bppPArgb, _bits);
    }

    public void Present()
    {
        if (_hwnd == IntPtr.Zero || _memDc == IntPtr.Zero || _bitmap is null || _wPx <= 1 || _hPx <= 1)
        {
            return;
        }

        if (!Win32Native.GetWindowRect(_hwnd, out var rc))
        {
            return;
        }

        var ptDst = new Win32Native.POINT { x = rc.Left, y = rc.Top };
        var size = new Win32Native.SIZE { cx = _wPx, cy = _hPx };
        var ptSrc = new Win32Native.POINT();

        var blend = new Win32Native.BLENDFUNCTION
        {
            BlendOp = Win32Native.AC_SRC_OVER,
            BlendFlags = 0,
            SourceConstantAlpha = 255,
            AlphaFormat = Win32Native.AC_SRC_ALPHA,
        };

        var screenDc = Win32Native.GetDC(IntPtr.Zero);
        try
        {
            _ = Win32Native.UpdateLayeredWindow(
                _hwnd,
                screenDc,
                ref ptDst,
                ref size,
                _memDc,
                ref ptSrc,
                0,
                ref blend,
                Win32Native.ULW_ALPHA
            );
        }
        finally
        {
            _ = Win32Native.ReleaseDC(IntPtr.Zero, screenDc);
        }
    }

    public void Dispose()
    {
        Destroy();
    }

    private void Destroy()
    {
        try { _bitmap?.Dispose(); } catch { }
        _bitmap = null;

        if (_memDc != IntPtr.Zero && _hbmOld != IntPtr.Zero)
        {
            try { _ = Win32Native.SelectObject(_memDc, _hbmOld); } catch { }
        }
        _hbmOld = IntPtr.Zero;

        if (_hbm != IntPtr.Zero)
        {
            try { _ = Win32Native.DeleteObject(_hbm); } catch { }
        }
        _hbm = IntPtr.Zero;
        _bits = IntPtr.Zero;

        if (_memDc != IntPtr.Zero)
        {
            try { _ = Win32Native.DeleteDC(_memDc); } catch { }
        }
        _memDc = IntPtr.Zero;

        _wPx = 0;
        _hPx = 0;
    }
}

