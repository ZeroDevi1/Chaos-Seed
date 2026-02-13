using System;
using System.Collections.Generic;
using ChaosSeed.WinUI3.Models.Music;

namespace ChaosSeed.WinUI3.Services;

public sealed class MusicSearchStateService
{
    public static MusicSearchStateService Instance { get; } = new();

    private MusicSearchStateService() { }

    public MusicSearchState? State { get; private set; }

    public void Save(MusicSearchState state)
    {
        State = state ?? throw new ArgumentNullException(nameof(state));
    }

    public void Clear()
    {
        State = null;
    }
}

public sealed class MusicSearchState
{
    public string ServiceTag { get; set; } = "qq";
    public string SearchModeTag { get; set; } = "track";
    public string Keyword { get; set; } = "";
    public string DefaultQualityTag { get; set; } = "flac";
    public int SearchPage { get; set; } = 1;

    public bool PagingEnabled { get; set; }
    public bool CanPrev { get; set; }
    public bool CanNext { get; set; }
    public string? PagingHint { get; set; }

    public MusicTrack[] Tracks { get; set; } = Array.Empty<MusicTrack>();
    public Dictionary<string, string> TrackQualityByKey { get; set; } = new(StringComparer.Ordinal);

    public MusicAlbum[] Albums { get; set; } = Array.Empty<MusicAlbum>();
    public MusicArtist[] Artists { get; set; } = Array.Empty<MusicArtist>();

    // Detail panel (album tracks / artist albums)
    public bool DetailVisible { get; set; }
    public MusicAlbum? DetailAlbum { get; set; }
    public MusicArtist? DetailArtist { get; set; }
    public MusicTrack[] DetailTracks { get; set; } = Array.Empty<MusicTrack>();
    public Dictionary<string, string> DetailTrackQualityByKey { get; set; } = new(StringComparer.Ordinal);
    public MusicAlbum[] DetailAlbums { get; set; } = Array.Empty<MusicAlbum>();
}

