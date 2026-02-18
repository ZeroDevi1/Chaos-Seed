using ChaosSeed.WinUI3.Models.Bili;

namespace ChaosSeed.WinUI3.Services.BiliBackends;

public sealed class DaemonBiliBackend : IBiliBackend
{
    private readonly string? _initNotice;

    public DaemonBiliBackend(string? initNotice = null)
    {
        _initNotice = initNotice;
    }

    public string Name => "Daemon";
    public string? InitNotice => _initNotice;

    public Task<BiliLoginQr> LoginQrCreateAsync(CancellationToken ct)
        => DaemonClient.Instance.BiliLoginQrCreateAsync(ct);

    public Task<BiliLoginQrPollResult> LoginQrPollAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.BiliLoginQrPollAsync(sessionId, ct);

    public Task<BiliRefreshCookieResult> RefreshCookieAsync(BiliRefreshCookieParams p, CancellationToken ct)
        => DaemonClient.Instance.BiliRefreshCookieAsync(p, ct);

    public Task<BiliParseResult> ParseAsync(BiliParseParams p, CancellationToken ct)
        => DaemonClient.Instance.BiliParseAsync(p, ct);

    public Task<BiliDownloadStartResult> DownloadStartAsync(BiliDownloadStartParams p, CancellationToken ct)
        => DaemonClient.Instance.BiliDownloadStartAsync(p, ct);

    public Task<BiliDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.BiliDownloadStatusAsync(sessionId, ct);

    public Task CancelDownloadAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.BiliDownloadCancelAsync(sessionId, ct);

    public void Dispose()
    {
        // DaemonClient is a shared singleton.
    }
}

