using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LyricsBackends;

public sealed class DaemonLyricsBackend : ILyricsBackend
{
    private readonly string? _initNotice;

    public DaemonLyricsBackend(string? initNotice = null)
    {
        _initNotice = initNotice;
    }

    public string Name => "Daemon";

    public string? InitNotice => _initNotice;

    public Task<NowPlayingSnapshot> SnapshotNowPlayingAsync(
        bool includeThumbnail,
        int maxThumbBytes,
        int maxSessions,
        CancellationToken ct
    )
        => DaemonClient.Instance.NowPlayingSnapshotAsync(includeThumbnail, maxThumbBytes, maxSessions, ct);

    public Task<LyricsSearchResult[]> SearchLyricsAsync(LyricsSearchParams p, CancellationToken ct)
        => DaemonClient.Instance.LyricsSearchAsync(p, ct);

    public void Dispose()
    {
        // DaemonClient is shared singleton.
    }
}

