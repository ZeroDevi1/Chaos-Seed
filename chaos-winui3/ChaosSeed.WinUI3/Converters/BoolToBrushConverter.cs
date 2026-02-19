using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using System;

namespace ChaosSeed.WinUI3.Converters;

public sealed class BoolToBrushConverter : IValueConverter
{
    // NOTE:
    // XAML ThemeResource lookups can yield values that are not Brush (e.g., Color or UnsetValue) depending on
    // resource availability and platform/runtime. Keep these as object to avoid XAML parse-time assignment errors.
    public object? TrueBrush { get; set; }
    public object? FalseBrush { get; set; }

    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var b = value is bool x && x;
        return CoerceBrush(b ? TrueBrush : FalseBrush) ?? new SolidColorBrush();
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotSupportedException();

    private static Brush? CoerceBrush(object? v)
    {
        if (v is null || ReferenceEquals(v, DependencyProperty.UnsetValue))
        {
            return null;
        }

        if (v is Brush b)
        {
            return b;
        }

        if (v is Windows.UI.Color c)
        {
            return new SolidColorBrush(c);
        }

        if (v is string s && TryParseHexColor(s, out var hc))
        {
            return new SolidColorBrush(hc);
        }

        return null;
    }

    private static bool TryParseHexColor(string s, out Windows.UI.Color c)
    {
        c = default;
        var t = (s ?? "").Trim();
        if (t.Length == 0)
        {
            return false;
        }

        if (t[0] == '#')
        {
            t = t.Substring(1);
        }

        static bool ByteFromHex(string two, out byte b)
            => byte.TryParse(two, System.Globalization.NumberStyles.HexNumber, provider: null, out b);

        // #RRGGBB or #AARRGGBB
        if (t.Length == 6)
        {
            if (!ByteFromHex(t.Substring(0, 2), out var r)) return false;
            if (!ByteFromHex(t.Substring(2, 2), out var g)) return false;
            if (!ByteFromHex(t.Substring(4, 2), out var b)) return false;
            c = Windows.UI.ColorHelper.FromArgb(255, r, g, b);
            return true;
        }
        if (t.Length == 8)
        {
            if (!ByteFromHex(t.Substring(0, 2), out var a)) return false;
            if (!ByteFromHex(t.Substring(2, 2), out var r)) return false;
            if (!ByteFromHex(t.Substring(4, 2), out var g)) return false;
            if (!ByteFromHex(t.Substring(6, 2), out var b)) return false;
            c = Windows.UI.ColorHelper.FromArgb(a, r, g, b);
            return true;
        }

        return false;
    }
}
