using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.DanmakuBackends;

public sealed class ErrorDanmakuBackend : IDanmakuBackend
{
    public ErrorDanmakuBackend(string name, string message)
    {
        Name = name;
        _message = message;
    }

    private readonly string _message;

    public string Name { get; }
    public string? InitNotice => _message;

    public event EventHandler<DanmakuMessage>? DanmakuMessageReceived
    {
        add { }
        remove { }
    }

    public Task<DanmakuConnectResult> ConnectAsync(string input, CancellationToken ct)
    {
        _ = input;
        ct.ThrowIfCancellationRequested();
        return Task.FromException<DanmakuConnectResult>(new Exception(_message));
    }

    public Task DisconnectAsync(string sessionId, CancellationToken ct)
    {
        _ = sessionId;
        ct.ThrowIfCancellationRequested();
        return Task.CompletedTask;
    }

    public Task<DanmakuFetchImageResult> FetchImageAsync(string sessionId, string url, CancellationToken ct)
    {
        _ = sessionId;
        _ = url;
        ct.ThrowIfCancellationRequested();
        return Task.FromResult(new DanmakuFetchImageResult());
    }

    public void Dispose()
    {
    }
}
