using ChaosSeed.WinUI3.Models.Music;

namespace ChaosSeed.WinUI3.Services.MusicBackends;

public sealed class ErrorMusicBackend : IMusicBackend
{
    private readonly string _name;
    private readonly string _message;

    public ErrorMusicBackend(string name, string message)
    {
        _name = name;
        _message = message;
    }

    public string Name => _name;
    public string? InitNotice => _message;

    public Task ConfigSetAsync(MusicProviderConfig cfg, CancellationToken ct)
        => Task.FromException(new InvalidOperationException(_message));

    public Task<MusicTrack[]> SearchTracksAsync(MusicSearchParams p, CancellationToken ct)
        => Task.FromException<MusicTrack[]>(new InvalidOperationException(_message));

    public Task<MusicAlbum[]> SearchAlbumsAsync(MusicSearchParams p, CancellationToken ct)
        => Task.FromException<MusicAlbum[]>(new InvalidOperationException(_message));

    public Task<MusicArtist[]> SearchArtistsAsync(MusicSearchParams p, CancellationToken ct)
        => Task.FromException<MusicArtist[]>(new InvalidOperationException(_message));

    public Task<MusicTrack[]> AlbumTracksAsync(MusicAlbumTracksParams p, CancellationToken ct)
        => Task.FromException<MusicTrack[]>(new InvalidOperationException(_message));

    public Task<MusicAlbum[]> ArtistAlbumsAsync(MusicArtistAlbumsParams p, CancellationToken ct)
        => Task.FromException<MusicAlbum[]>(new InvalidOperationException(_message));

    public Task<MusicTrackPlayUrlResult> TrackPlayUrlAsync(MusicTrackPlayUrlParams p, CancellationToken ct)
        => Task.FromException<MusicTrackPlayUrlResult>(new InvalidOperationException(_message));

    public Task<MusicLoginQr> QqLoginQrCreateAsync(string loginType, CancellationToken ct)
        => Task.FromException<MusicLoginQr>(new InvalidOperationException(_message));

    public Task<MusicLoginQrPollResult> QqLoginQrPollAsync(string sessionId, CancellationToken ct)
        => Task.FromException<MusicLoginQrPollResult>(new InvalidOperationException(_message));

    public Task<QqMusicCookie> QqRefreshCookieAsync(QqMusicCookie cookie, CancellationToken ct)
        => Task.FromException<QqMusicCookie>(new InvalidOperationException(_message));

    public Task<MusicLoginQr> KugouLoginQrCreateAsync(string loginType, CancellationToken ct)
        => Task.FromException<MusicLoginQr>(new InvalidOperationException(_message));

    public Task<MusicLoginQrPollResult> KugouLoginQrPollAsync(string sessionId, CancellationToken ct)
        => Task.FromException<MusicLoginQrPollResult>(new InvalidOperationException(_message));

    public Task<MusicDownloadStartResult> DownloadStartAsync(MusicDownloadStartParams p, CancellationToken ct)
        => Task.FromException<MusicDownloadStartResult>(new InvalidOperationException(_message));

    public Task<MusicDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct)
        => Task.FromException<MusicDownloadStatus>(new InvalidOperationException(_message));

    public Task CancelDownloadAsync(string sessionId, CancellationToken ct)
        => Task.FromException(new InvalidOperationException(_message));

    public void Dispose()
    {
        // nothing
    }
}
