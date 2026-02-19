using ChaosSeed.WinUI3.Models.Bili;

namespace ChaosSeed.WinUI3.Services.BiliBackends;

public sealed class ErrorBiliBackend : IBiliBackend
{
    private readonly string _name;
    private readonly string _message;

    public ErrorBiliBackend(string name, string message)
    {
        _name = name;
        _message = message;
    }

    public string Name => _name;
    public string? InitNotice => _message;

    public Task<BiliLoginQr> LoginQrCreateAsync(CancellationToken ct)
        => Task.FromException<BiliLoginQr>(new InvalidOperationException(_message));

    public Task<BiliLoginQrPollResult> LoginQrPollAsync(string sessionId, CancellationToken ct)
        => Task.FromException<BiliLoginQrPollResult>(new InvalidOperationException(_message));

    public Task<BiliRefreshCookieResult> RefreshCookieAsync(BiliRefreshCookieParams p, CancellationToken ct)
        => Task.FromException<BiliRefreshCookieResult>(new InvalidOperationException(_message));

    public Task<BiliLoginQr> LoginQrCreateV2Async(string loginType, CancellationToken ct)
        => Task.FromException<BiliLoginQr>(new InvalidOperationException(_message));

    public Task<BiliLoginQrPollResultV2> LoginQrPollV2Async(string sessionId, CancellationToken ct)
        => Task.FromException<BiliLoginQrPollResultV2>(new InvalidOperationException(_message));

    public Task<BiliCheckLoginResult> CheckLoginAsync(BiliAuthBundle auth, CancellationToken ct)
        => Task.FromException<BiliCheckLoginResult>(new InvalidOperationException(_message));

    public Task<BiliTaskAddResult> TaskAddAsync(BiliTaskAddParams p, CancellationToken ct)
        => Task.FromException<BiliTaskAddResult>(new InvalidOperationException(_message));

    public Task<BiliTasksGetResult> TasksGetAsync(CancellationToken ct)
        => Task.FromException<BiliTasksGetResult>(new InvalidOperationException(_message));

    public Task<BiliTaskDetail> TaskGetAsync(string taskId, CancellationToken ct)
        => Task.FromException<BiliTaskDetail>(new InvalidOperationException(_message));

    public Task TaskCancelAsync(string taskId, CancellationToken ct)
        => Task.FromException(new InvalidOperationException(_message));

    public Task TasksRemoveFinishedAsync(BiliTasksRemoveFinishedParams p, CancellationToken ct)
        => Task.FromException(new InvalidOperationException(_message));

    public Task<BiliParseResult> ParseAsync(BiliParseParams p, CancellationToken ct)
        => Task.FromException<BiliParseResult>(new InvalidOperationException(_message));

    public Task<BiliDownloadStartResult> DownloadStartAsync(BiliDownloadStartParams p, CancellationToken ct)
        => Task.FromException<BiliDownloadStartResult>(new InvalidOperationException(_message));

    public Task<BiliDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct)
        => Task.FromException<BiliDownloadStatus>(new InvalidOperationException(_message));

    public Task CancelDownloadAsync(string sessionId, CancellationToken ct)
        => Task.FromException(new InvalidOperationException(_message));

    public void Dispose()
    {
        // nothing
    }
}
