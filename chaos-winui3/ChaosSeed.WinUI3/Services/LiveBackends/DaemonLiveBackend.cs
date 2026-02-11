using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveBackends;

public sealed class DaemonLiveBackend : ILiveBackend
{
    private readonly string? _initNotice;

    public DaemonLiveBackend(string? initNotice = null)
    {
        _initNotice = initNotice;
        DaemonClient.Instance.DanmakuMessageReceived += OnDaemonDanmakuMessageReceived;
    }

    public string Name => "Daemon";

    public string? InitNotice => _initNotice;

    public event EventHandler<DanmakuMessage>? DanmakuMessageReceived;

    public Task<LivestreamDecodeManifestResult> DecodeManifestAsync(string input, CancellationToken ct)
        => DaemonClient.Instance.DecodeManifestAsync(input, ct);

    public Task<LiveOpenResult> OpenLiveAsync(string input, string? variantId, CancellationToken ct)
        => DaemonClient.Instance.OpenLiveAsync(input, variantId, ct);

    public Task CloseLiveAsync(string sessionId, CancellationToken ct)
        => DaemonClient.Instance.CloseLiveAsync(sessionId, ct);

    public Task<DanmakuFetchImageResult> FetchDanmakuImageAsync(string sessionId, string url, CancellationToken ct)
        => DaemonClient.Instance.FetchDanmakuImageAsync(sessionId, url, ct);

    public void Dispose()
    {
        DaemonClient.Instance.DanmakuMessageReceived -= OnDaemonDanmakuMessageReceived;
    }

    private void OnDaemonDanmakuMessageReceived(object? sender, DanmakuMessage msg)
    {
        DanmakuMessageReceived?.Invoke(this, msg);
    }
}

