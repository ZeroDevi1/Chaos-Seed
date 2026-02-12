using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.DanmakuBackends;

public interface IDanmakuBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    event EventHandler<DanmakuMessage>? DanmakuMessageReceived;

    Task<DanmakuConnectResult> ConnectAsync(string input, CancellationToken ct);
    Task DisconnectAsync(string sessionId, CancellationToken ct);
    Task<DanmakuFetchImageResult> FetchImageAsync(string sessionId, string url, CancellationToken ct);
}

