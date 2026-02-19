using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Media;
using System;

namespace ChaosSeed.WinUI3.Converters;

public sealed class BoolToBrushConverter : IValueConverter
{
    public Brush? TrueBrush { get; set; }
    public Brush? FalseBrush { get; set; }

    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var b = value is bool x && x;
        return b ? (TrueBrush ?? new SolidColorBrush()) : (FalseBrush ?? new SolidColorBrush());
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotSupportedException();
}

