using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Graphics.Canvas;
using Microsoft.Graphics.Canvas.Text;
using Windows.Foundation;
using Windows.UI;

namespace ChaosSeed.WinUI3.Services.NativeOverlay;

internal sealed class NativeOverlayColorTextRenderer : IDisposable
{
    private readonly CanvasDevice _device = CanvasDevice.GetSharedDevice();
    private bool _disposed;

    public static bool ShouldUseColorFontRendering(string? text)
    {
        if (string.IsNullOrWhiteSpace(text))
        {
            return false;
        }

        foreach (var rune in text.EnumerateRunes())
        {
            var value = rune.Value;
            if (value == 0x200D || value == 0xFE0F)
            {
                return true;
            }

            if ((value >= 0x1F000 && value <= 0x1FAFF)
                || (value >= 0x2600 && value <= 0x27BF)
                || (value >= 0x2300 && value <= 0x23FF))
            {
                return true;
            }
        }

        return false;
    }

    public Bitmap? TryRender(string? text, float fontSizePx)
    {
        ThrowIfDisposed();

        var normalized = (text ?? string.Empty).Trim();
        if (!ShouldUseColorFontRendering(normalized))
        {
            return null;
        }

        try
        {
            using var colorFormat = CreateTextFormat(fontSizePx, CanvasDrawTextOptions.EnableColorFont);
            using var colorLayout = new CanvasTextLayout(_device, normalized, colorFormat, 4096, 4096);
            var bounds = colorLayout.DrawBounds;
            if (bounds.Width <= 0 || bounds.Height <= 0)
            {
                return null;
            }

            using var shadowFormat = CreateTextFormat(fontSizePx, default);
            using var shadowLayout = new CanvasTextLayout(_device, normalized, shadowFormat, 4096, 4096);

            const float shadowOffsetPx = 1f;
            var padding = Math.Max(4f, fontSizePx * 0.35f);
            var width = (int)Math.Ceiling(bounds.Width + padding * 2f + shadowOffsetPx);
            var height = (int)Math.Ceiling(bounds.Height + padding * 2f + shadowOffsetPx);
            var originX = padding - (float)bounds.X;
            var originY = padding - (float)bounds.Y;

            using var target = new CanvasRenderTarget(_device, width, height, 96);
            using (var ds = target.CreateDrawingSession())
            {
                ds.Clear(global::Windows.UI.Color.FromArgb(0, 0, 0, 0));
                ds.DrawTextLayout(shadowLayout, originX + shadowOffsetPx, originY + shadowOffsetPx, global::Windows.UI.Color.FromArgb(160, 0, 0, 0));
                ds.DrawTextLayout(colorLayout, originX, originY, global::Windows.UI.Color.FromArgb(255, 255, 255, 255));
            }

            var bytes = target.GetPixelBytes();
            return CreateBitmap(width, height, bytes);
        }
        catch
        {
            return null;
        }
    }

    public void Dispose()
    {
        _disposed = true;
    }

    private static CanvasTextFormat CreateTextFormat(float fontSizePx, CanvasDrawTextOptions options)
    {
        return new CanvasTextFormat
        {
            FontFamily = "Segoe UI",
            FontSize = fontSizePx,
            LocaleName = "zh-CN",
            Options = options,
            WordWrapping = CanvasWordWrapping.NoWrap,
        };
    }

    private static Bitmap CreateBitmap(int width, int height, byte[] bytes)
    {
        var bitmap = new Bitmap(width, height, PixelFormat.Format32bppPArgb);
        var rect = new Rectangle(0, 0, width, height);
        var data = bitmap.LockBits(rect, ImageLockMode.WriteOnly, PixelFormat.Format32bppPArgb);
        try
        {
            Marshal.Copy(bytes, 0, data.Scan0, Math.Min(bytes.Length, Math.Abs(data.Stride) * data.Height));
        }
        finally
        {
            bitmap.UnlockBits(data);
        }

        return bitmap;
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
    }
}



