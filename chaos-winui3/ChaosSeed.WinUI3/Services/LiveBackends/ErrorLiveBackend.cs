using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveBackends;

public sealed class ErrorLiveBackend : ILiveBackend
{
    private readonly string _name;
    private readonly string _message;

    public ErrorLiveBackend(string name, string message)
    {
        _name = string.IsNullOrWhiteSpace(name) ? "Unavailable" : name.Trim();
        _message = string.IsNullOrWhiteSpace(message) ? "backend unavailable" : message.Trim();
    }

    public string Name => _name;

    public string? InitNotice => _message;

    public event EventHandler<DanmakuMessage>? DanmakuMessageReceived
    {
        add { }
        remove { }
    }

    public Task<LivestreamDecodeManifestResult> DecodeManifestAsync(string input, CancellationToken ct)
        => Task.FromException<LivestreamDecodeManifestResult>(new InvalidOperationException(_message));

    public Task<LiveOpenResult> OpenLiveAsync(string input, string? variantId, CancellationToken ct)
        => Task.FromException<LiveOpenResult>(new InvalidOperationException(_message));

    public Task CloseLiveAsync(string sessionId, CancellationToken ct)
        => Task.CompletedTask;

    public Task<DanmakuFetchImageResult> FetchDanmakuImageAsync(string sessionId, string url, CancellationToken ct)
        => Task.FromException<DanmakuFetchImageResult>(new InvalidOperationException(_message));

    public void Dispose()
    {
    }
}

