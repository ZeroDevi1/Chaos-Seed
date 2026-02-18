using System;
using Microsoft.UI.Xaml.Media.Imaging;

namespace ChaosSeed.WinUI3.Services;

public static class MusicUiUtil
{
    public static BitmapImage? TryCreateBitmap(string? url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            return null;
        }

        try
        {
            var s = url.Trim();
            if (s.StartsWith("//", StringComparison.Ordinal))
            {
                s = "https:" + s;
            }

            if (!Uri.TryCreate(s, UriKind.Absolute, out var u))
            {
                return null;
            }

            return new BitmapImage(u);
        }
        catch
        {
            return null;
        }
    }
}

