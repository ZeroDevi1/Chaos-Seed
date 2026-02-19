using ChaosSeed.WinUI3.Models.Bili;

namespace ChaosSeed.WinUI3.Services.BiliBackends;

public interface IBiliBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    Task<BiliLoginQr> LoginQrCreateAsync(CancellationToken ct);
    Task<BiliLoginQrPollResult> LoginQrPollAsync(string sessionId, CancellationToken ct);
    Task<BiliRefreshCookieResult> RefreshCookieAsync(BiliRefreshCookieParams p, CancellationToken ct);

    // v2 (web/tv) + task API
    Task<BiliLoginQr> LoginQrCreateV2Async(string loginType, CancellationToken ct);
    Task<BiliLoginQrPollResultV2> LoginQrPollV2Async(string sessionId, CancellationToken ct);
    Task<BiliCheckLoginResult> CheckLoginAsync(BiliAuthBundle auth, CancellationToken ct);

    Task<BiliTaskAddResult> TaskAddAsync(BiliTaskAddParams p, CancellationToken ct);
    Task<BiliTasksGetResult> TasksGetAsync(CancellationToken ct);
    Task<BiliTaskDetail> TaskGetAsync(string taskId, CancellationToken ct);
    Task TaskCancelAsync(string taskId, CancellationToken ct);
    Task TasksRemoveFinishedAsync(BiliTasksRemoveFinishedParams p, CancellationToken ct);

    Task<BiliParseResult> ParseAsync(BiliParseParams p, CancellationToken ct);

    Task<BiliDownloadStartResult> DownloadStartAsync(BiliDownloadStartParams p, CancellationToken ct);
    Task<BiliDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct);
    Task CancelDownloadAsync(string sessionId, CancellationToken ct);
}
