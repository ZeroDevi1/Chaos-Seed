using Microsoft.UI.Xaml.Data;
using System;

namespace ChaosSeed.WinUI3.Converters;

public sealed class SecondsToTimeConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        try
        {
            if (value is double d)
            {
                if (double.IsNaN(d) || double.IsInfinity(d))
                {
                    return "00:00";
                }
                if (d < 0) d = 0;
                return Format(TimeSpan.FromSeconds(d));
            }
            if (value is float f)
            {
                if (float.IsNaN(f) || float.IsInfinity(f))
                {
                    return "00:00";
                }
                if (f < 0) f = 0;
                return Format(TimeSpan.FromSeconds(f));
            }
        }
        catch
        {
            // ignore
        }
        return "00:00";
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language) => throw new NotSupportedException();

    private static string Format(TimeSpan t)
    {
        if (t.TotalHours >= 1)
        {
            return $"{(int)t.TotalHours:00}:{t.Minutes:00}:{t.Seconds:00}";
        }
        return $"{(int)t.TotalMinutes:00}:{t.Seconds:00}";
    }
}

