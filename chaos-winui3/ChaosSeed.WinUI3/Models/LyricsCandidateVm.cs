using Microsoft.UI.Xaml;

namespace ChaosSeed.WinUI3.Models;

public sealed class LyricsCandidateVm
{
    public LyricsCandidateVm(LyricsSearchResult item)
    {
        Item = item ?? throw new ArgumentNullException(nameof(item));
    }

    public LyricsSearchResult Item { get; }

    public string Service => (Item.Service ?? "").Trim();
    public int MatchPercentage => Item.MatchPercentage;

    public string Title => (Item.Title ?? "").Trim();
    public string Artist => (Item.Artist ?? "").Trim();
    public string Album => (Item.Album ?? "").Trim();

    public bool HasTranslation => Item.HasTranslation;
    public bool HasInlineTimetags => Item.HasInlineTimetags;

    public string Display
    {
        get
        {
            var svc = string.IsNullOrWhiteSpace(Service) ? "unknown" : Service;
            var t = string.IsNullOrWhiteSpace(Title) ? "-" : Title;
            var a = string.IsNullOrWhiteSpace(Artist) ? "-" : Artist;
            return $"{svc} · {MatchPercentage} · {t} - {a}";
        }
    }

    public Visibility TranslationBadgeVisibility => HasTranslation ? Visibility.Visible : Visibility.Collapsed;
}

