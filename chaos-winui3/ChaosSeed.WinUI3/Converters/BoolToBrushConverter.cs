using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using System;

namespace ChaosSeed.WinUI3.Converters;

public sealed class BoolToBrushConverter : DependencyObject, IValueConverter
{
    public static readonly DependencyProperty TrueBrushProperty =
        DependencyProperty.Register(
            nameof(TrueBrush),
            typeof(object),
            typeof(BoolToBrushConverter),
            new PropertyMetadata(null));

    public static readonly DependencyProperty FalseBrushProperty =
        DependencyProperty.Register(
            nameof(FalseBrush),
            typeof(object),
            typeof(BoolToBrushConverter),
            new PropertyMetadata(null));

    // NOTE:
    // - ThemeResource is only supported on DependencyProperties; keep these as DPs so XAML can assign ThemeResource.
    // - Keep value type as object: ThemeResource lookups can yield values that are not Brush depending on availability.
    public object? TrueBrush
    {
        get => (object?)GetValue(TrueBrushProperty);
        set => SetValue(TrueBrushProperty, value);
    }

    public object? FalseBrush
    {
        get => (object?)GetValue(FalseBrushProperty);
        set => SetValue(FalseBrushProperty, value);
    }

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

        if (v is global::Windows.UI.Color c)
        {
            var sb = new SolidColorBrush();
            sb.Color = c;
            return sb;
        }

        if (v is string s && TryParseHexColor(s, out var hc))
        {
            var sb = new SolidColorBrush();
            sb.Color = hc;
            return sb;
        }

        return null;
    }

    private static bool TryParseHexColor(string s, out global::Windows.UI.Color c)
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
            c = new global::Windows.UI.Color { A = 255, R = r, G = g, B = b };
            return true;
        }
        if (t.Length == 8)
        {
            if (!ByteFromHex(t.Substring(0, 2), out var a)) return false;
            if (!ByteFromHex(t.Substring(2, 2), out var r)) return false;
            if (!ByteFromHex(t.Substring(4, 2), out var g)) return false;
            if (!ByteFromHex(t.Substring(6, 2), out var b)) return false;
            c = new global::Windows.UI.Color { A = a, R = r, G = g, B = b };
            return true;
        }

        return false;
    }
}
