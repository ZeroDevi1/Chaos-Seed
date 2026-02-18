using ChaosSeed.WinUI3.Models.Bili;

namespace ChaosSeed.WinUI3.Services.BiliBackends;

public interface IBiliBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    Task<BiliLoginQr> LoginQrCreateAsync(CancellationToken ct);
    Task<BiliLoginQrPollResult> LoginQrPollAsync(string sessionId, CancellationToken ct);
    Task<BiliRefreshCookieResult> RefreshCookieAsync(BiliRefreshCookieParams p, CancellationToken ct);

    Task<BiliParseResult> ParseAsync(BiliParseParams p, CancellationToken ct);

    Task<BiliDownloadStartResult> DownloadStartAsync(BiliDownloadStartParams p, CancellationToken ct);
    Task<BiliDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct);
    Task CancelDownloadAsync(string sessionId, CancellationToken ct);
}

