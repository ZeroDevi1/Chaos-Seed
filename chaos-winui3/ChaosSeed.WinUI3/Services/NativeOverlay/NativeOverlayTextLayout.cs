using System.Collections.Concurrent;
using System.Drawing;
using System.Drawing.Imaging;
using System.Globalization;
using System.Runtime.InteropServices;

namespace ChaosSeed.WinUI3.Services.NativeOverlay;

internal sealed class NativeOverlayTextLayout : IDisposable
{
    public sealed record TextRun(string Text, string FontFamily, SizeF Size);

    private readonly ConcurrentDictionary<FontKey, Font> _fontCache = new();
    private readonly ConcurrentDictionary<GlyphCacheKey, bool> _glyphCache = new();
    private readonly Bitmap _measureBitmap = new(1, 1, PixelFormat.Format32bppPArgb);
    private readonly StringFormat _stringFormat;
    private bool _disposed;

    private static readonly string[] FontFallbackChain =
    {
        "Segoe UI",
        "Microsoft YaHei UI",
        "Segoe UI Symbol",
        "Segoe UI Emoji",
    };

    public NativeOverlayTextLayout()
    {
        _stringFormat = new StringFormat(StringFormat.GenericTypographic);
        _stringFormat.FormatFlags |= StringFormatFlags.MeasureTrailingSpaces;
    }

    public sealed record Layout(string Text, IReadOnlyList<TextRun> Runs, SizeF Size);

    public Layout CreateLayout(string text, float fontSizePx)
    {
        ThrowIfDisposed();

        var normalized = (text ?? string.Empty).Trim();
        if (normalized.Length == 0)
        {
            return new Layout(string.Empty, Array.Empty<TextRun>(), SizeF.Empty);
        }

        var elements = SplitTextElements(normalized);
        if (elements.Count == 0)
        {
            return new Layout(normalized, Array.Empty<TextRun>(), SizeF.Empty);
        }

        var runs = new List<TextRun>();
        using var g = Graphics.FromImage(_measureBitmap);
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;

        var currentFamily = string.Empty;
        var currentText = string.Empty;

        void FlushCurrent()
        {
            if (currentText.Length == 0)
            {
                return;
            }

            var font = GetFont(currentFamily, fontSizePx);
            var size = MeasureText(g, currentText, font);
            runs.Add(new TextRun(currentText, font.Name, size));
            currentText = string.Empty;
        }

        foreach (var element in elements)
        {
            var family = ResolveFontFamily(g, element, fontSizePx);
            if (currentText.Length == 0)
            {
                currentFamily = family;
                currentText = element;
                continue;
            }

            if (string.Equals(currentFamily, family, StringComparison.Ordinal))
            {
                currentText += element;
                continue;
            }

            FlushCurrent();
            currentFamily = family;
            currentText = element;
        }

        FlushCurrent();

        var width = 0f;
        var height = 0f;
        foreach (var run in runs)
        {
            width += run.Size.Width;
            height = Math.Max(height, run.Size.Height);
        }

        return new Layout(normalized, runs, new SizeF(width, height));
    }

    public void DrawLayout(
        Graphics g,
        Layout layout,
        Brush shadowBrush,
        Brush textBrush,
        StringFormat stringFormat,
        float x,
        float y,
        float fontSizePx
    )
    {
        ThrowIfDisposed();

        if (layout is null || layout.Runs.Count == 0)
        {
            return;
        }

        var drawX = x;
        foreach (var run in layout.Runs)
        {
            using var font = CreateDrawingFont(run.FontFamily, fontSizePx);
            g.DrawString(run.Text, font, shadowBrush, drawX + 1f, y + 1f, stringFormat);
            g.DrawString(run.Text, font, textBrush, drawX, y, stringFormat);
            drawX += run.Size.Width;
        }
    }

    public static IReadOnlyList<string> SplitTextElements(string text)
    {
        var normalized = text ?? string.Empty;
        if (normalized.Length == 0)
        {
            return Array.Empty<string>();
        }

        var elements = new List<string>();
        var enumerator = StringInfo.GetTextElementEnumerator(normalized);
        while (enumerator.MoveNext())
        {
            if (enumerator.Current is string element && element.Length > 0)
            {
                elements.Add(element);
            }
        }

        return elements;
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        foreach (var font in _fontCache.Values)
        {
            try { font.Dispose(); } catch { }
        }
        _fontCache.Clear();
        _glyphCache.Clear();
        _stringFormat.Dispose();
        _measureBitmap.Dispose();
    }

    private string ResolveFontFamily(Graphics g, string element, float fontSizePx)
    {
        foreach (var family in FontFallbackChain)
        {
            if (CanRenderTextElement(g, family, element, fontSizePx))
            {
                return family;
            }
        }

        return FontFallbackChain[0];
    }

    private bool CanRenderTextElement(Graphics g, string fontFamily, string element, float fontSizePx)
    {
        if (string.IsNullOrEmpty(element))
        {
            return true;
        }

        var cacheKey = new GlyphCacheKey(fontFamily, element);
        if (_glyphCache.TryGetValue(cacheKey, out var cached))
        {
            return cached;
        }

        var font = GetFont(fontFamily, fontSizePx);
        var result = HasGlyphs(g, font, element);
        _glyphCache[cacheKey] = result;
        return result;
    }

    private Font GetFont(string fontFamily, float fontSizePx)
    {
        var key = new FontKey(fontFamily, fontSizePx);
        return _fontCache.GetOrAdd(key, static key =>
            new Font(key.FontFamily, key.FontSizePx, FontStyle.Regular, GraphicsUnit.Pixel));
    }

    private static Font CreateDrawingFont(string fontFamily, float fontSizePx)
    {
        return new Font(fontFamily, fontSizePx, FontStyle.Regular, GraphicsUnit.Pixel);
    }

    private SizeF MeasureText(Graphics g, string text, Font font)
    {
        try
        {
            return g.MeasureString(text, font, int.MaxValue, _stringFormat);
        }
        catch
        {
            return new SizeF(Math.Max(20f, text.Length * font.Size), font.GetHeight(g));
        }
    }

    private static bool HasGlyphs(Graphics g, Font font, string text)
    {
        IntPtr hdc = IntPtr.Zero;
        IntPtr hFont = IntPtr.Zero;
        IntPtr oldObject = IntPtr.Zero;

        try
        {
            hdc = g.GetHdc();
            hFont = font.ToHfont();
            oldObject = SelectObject(hdc, hFont);

            var glyphs = new ushort[text.Length];
            var result = GetGlyphIndicesW(hdc, text, text.Length, glyphs, GGI_MARK_NONEXISTING_GLYPHS);
            if (result == GDI_ERROR)
            {
                return false;
            }

            for (var i = 0; i < glyphs.Length; i++)
            {
                if (glyphs[i] == MissingGlyph)
                {
                    return false;
                }
            }

            return true;
        }
        catch
        {
            return false;
        }
        finally
        {
            if (hdc != IntPtr.Zero)
            {
                if (oldObject != IntPtr.Zero)
                {
                    _ = SelectObject(hdc, oldObject);
                }
                g.ReleaseHdc(hdc);
            }

            if (hFont != IntPtr.Zero)
            {
                _ = DeleteObject(hFont);
            }
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
    }

    private const uint GGI_MARK_NONEXISTING_GLYPHS = 0x0001;
    private const uint GDI_ERROR = 0xFFFFFFFF;
    private const ushort MissingGlyph = 0xFFFF;

    private readonly record struct FontKey(string FontFamily, float FontSizePx);

    private readonly record struct GlyphCacheKey(string FontFamily, string TextElement);

    [DllImport("gdi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetGlyphIndicesW(
        IntPtr hdc,
        string text,
        int textLength,
        [Out] ushort[] glyphIndices,
        uint flags
    );

    [DllImport("gdi32.dll", SetLastError = true)]
    private static extern IntPtr SelectObject(IntPtr hdc, IntPtr h);

    [DllImport("gdi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeleteObject(IntPtr ho);
}
