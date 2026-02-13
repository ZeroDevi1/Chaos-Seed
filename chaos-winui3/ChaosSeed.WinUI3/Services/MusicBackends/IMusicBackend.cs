using ChaosSeed.WinUI3.Models.Music;

namespace ChaosSeed.WinUI3.Services.MusicBackends;

public interface IMusicBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    Task ConfigSetAsync(MusicProviderConfig cfg, CancellationToken ct);

    Task<MusicTrack[]> SearchTracksAsync(MusicSearchParams p, CancellationToken ct);
    Task<MusicAlbum[]> SearchAlbumsAsync(MusicSearchParams p, CancellationToken ct);
    Task<MusicArtist[]> SearchArtistsAsync(MusicSearchParams p, CancellationToken ct);

    Task<MusicTrack[]> AlbumTracksAsync(MusicAlbumTracksParams p, CancellationToken ct);
    Task<MusicAlbum[]> ArtistAlbumsAsync(MusicArtistAlbumsParams p, CancellationToken ct);

    Task<MusicTrackPlayUrlResult> TrackPlayUrlAsync(MusicTrackPlayUrlParams p, CancellationToken ct);

    Task<MusicLoginQr> QqLoginQrCreateAsync(string loginType, CancellationToken ct);
    Task<MusicLoginQrPollResult> QqLoginQrPollAsync(string sessionId, CancellationToken ct);
    Task<QqMusicCookie> QqRefreshCookieAsync(QqMusicCookie cookie, CancellationToken ct);

    Task<MusicLoginQr> KugouLoginQrCreateAsync(string loginType, CancellationToken ct);
    Task<MusicLoginQrPollResult> KugouLoginQrPollAsync(string sessionId, CancellationToken ct);

    Task<MusicDownloadStartResult> DownloadStartAsync(MusicDownloadStartParams p, CancellationToken ct);
    Task<MusicDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct);
    Task CancelDownloadAsync(string sessionId, CancellationToken ct);
}
