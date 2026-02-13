using ChaosSeed.WinUI3.Models.Music;

namespace ChaosSeed.WinUI3.Services.MusicBackends;

public sealed class DaemonMusicBackend : IMusicBackend
{
    private readonly string? _initNotice;

    public DaemonMusicBackend(string? initNotice = null)
    {
        _initNotice = initNotice;
    }

    public string Name => "Daemon";
    public string? InitNotice => _initNotice;

    public Task ConfigSetAsync(MusicProviderConfig cfg, CancellationToken ct)
        => DaemonClient.Instance.MusicConfigSetAsync(cfg, ct);

    public Task<MusicTrack[]> SearchTracksAsync(MusicSearchParams p, CancellationToken ct)
        => DaemonClient.Instance.MusicSearchTracksAsync(p, ct);

    public Task<MusicAlbum[]> SearchAlbumsAsync(MusicSearchParams p, CancellationToken ct)
        => DaemonClient.Instance.MusicSearchAlbumsAsync(p, ct);

    public Task<MusicArtist[]> SearchArtistsAsync(MusicSearchParams p, CancellationToken ct)
        => DaemonClient.Instance.MusicSearchArtistsAsync(p, ct);

    public Task<MusicTrack[]> AlbumTracksAsync(MusicAlbumTracksParams p, CancellationToken ct)
        => DaemonClient.Instance.MusicAlbumTracksAsync(p, ct);

    public Task<MusicAlbum[]> ArtistAlbumsAsync(MusicArtistAlbumsParams p, CancellationToken ct)
        => DaemonClient.Instance.MusicArtistAlbumsAsync(p, ct);

    public Task<MusicTrackPlayUrlResult> TrackPlayUrlAsync(MusicTrackPlayUrlParams p, CancellationToken ct)
        => DaemonClient.Instance.MusicTrackPlayUrlAsync(p, ct);

    public Task<MusicLoginQr> QqLoginQrCreateAsync(string loginType, CancellationToken ct)
        => DaemonClient.Instance.MusicQqLoginQrCreateAsync(loginType, ct);

    public Task<MusicLoginQrPollResult> QqLoginQrPollAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.MusicQqLoginQrPollAsync(sessionId, ct);

    public Task<QqMusicCookie> QqRefreshCookieAsync(QqMusicCookie cookie, CancellationToken ct)
        => DaemonClient.Instance.MusicQqRefreshCookieAsync(cookie, ct);

    public Task<MusicLoginQr> KugouLoginQrCreateAsync(string loginType, CancellationToken ct)
        => DaemonClient.Instance.MusicKugouLoginQrCreateAsync(loginType, ct);

    public Task<MusicLoginQrPollResult> KugouLoginQrPollAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.MusicKugouLoginQrPollAsync(sessionId, ct);

    public Task<MusicDownloadStartResult> DownloadStartAsync(MusicDownloadStartParams p, CancellationToken ct)
        => DaemonClient.Instance.MusicDownloadStartAsync(p, ct);

    public Task<MusicDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.MusicDownloadStatusAsync(sessionId, ct);

    public Task CancelDownloadAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.MusicDownloadCancelAsync(sessionId, ct);

    public void Dispose()
    {
        // DaemonClient is shared singleton.
    }
}
