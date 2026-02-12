using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LyricsBackends;

public sealed class ErrorLyricsBackend : ILyricsBackend
{
    private readonly string _name;
    private readonly string _message;

    public ErrorLyricsBackend(string name, string message)
    {
        _name = name;
        _message = message;
    }

    public string Name => _name;

    public string? InitNotice => _message;

    public Task<NowPlayingSnapshot> SnapshotNowPlayingAsync(bool includeThumbnail, int maxThumbBytes, int maxSessions, CancellationToken ct)
        => Task.FromException<NowPlayingSnapshot>(new InvalidOperationException(_message));

    public Task<LyricsSearchResult[]> SearchLyricsAsync(LyricsSearchParams p, CancellationToken ct)
        => Task.FromException<LyricsSearchResult[]>(new InvalidOperationException(_message));

    public void Dispose()
    {
        // nothing to dispose
    }
}

