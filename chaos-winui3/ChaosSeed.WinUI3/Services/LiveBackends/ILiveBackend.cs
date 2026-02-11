using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveBackends;

public interface ILiveBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    event EventHandler<DanmakuMessage>? DanmakuMessageReceived;

    Task<LivestreamDecodeManifestResult> DecodeManifestAsync(string input, CancellationToken ct);

    Task<LiveOpenResult> OpenLiveAsync(string input, string? variantId, CancellationToken ct);

    Task CloseLiveAsync(string sessionId, CancellationToken ct);

    Task<DanmakuFetchImageResult> FetchDanmakuImageAsync(string sessionId, string url, CancellationToken ct);
}
