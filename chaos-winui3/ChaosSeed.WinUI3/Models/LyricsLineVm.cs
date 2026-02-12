using System.Globalization;
using Microsoft.UI.Xaml;

namespace ChaosSeed.WinUI3.Models;

public sealed class LyricsLineVm
{
    public LyricsLineVm(ulong? timeMs, string original, string? translation)
    {
        TimeMs = timeMs;
        Original = original ?? "";
        Translation = translation;
    }

    public ulong? TimeMs { get; }

    public string TimeLabel
    {
        get
        {
            if (TimeMs is null)
            {
                return "";
            }

            var ts = TimeSpan.FromMilliseconds((double)TimeMs.Value);
            if (ts.TotalHours >= 1)
            {
                return $"{(int)ts.TotalHours:D2}:{ts.Minutes:D2}:{ts.Seconds:D2}";
            }

            return $"{ts.Minutes:D2}:{ts.Seconds:D2}";
        }
    }

    public string Original { get; }
    public string? Translation { get; }

    public Visibility TranslationVisibility =>
        string.IsNullOrWhiteSpace(Translation) ? Visibility.Collapsed : Visibility.Visible;

    public string TranslationText => string.IsNullOrWhiteSpace(Translation) ? "" : Translation!;

    public string DebugKey =>
        $"{TimeMs?.ToString(CultureInfo.InvariantCulture) ?? "na"}|{Original}|{Translation ?? ""}";
}

