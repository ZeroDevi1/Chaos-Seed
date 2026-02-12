using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LyricsBackends;

public interface ILyricsBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    Task<NowPlayingSnapshot> SnapshotNowPlayingAsync(
        bool includeThumbnail,
        int maxThumbBytes,
        int maxSessions,
        CancellationToken ct
    );

    Task<LyricsSearchResult[]> SearchLyricsAsync(LyricsSearchParams p, CancellationToken ct);
}

