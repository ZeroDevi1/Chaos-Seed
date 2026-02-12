using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.DanmakuBackends;

public sealed class DaemonDanmakuBackend : IDanmakuBackend
{
    public DaemonDanmakuBackend(string? initNotice = null)
    {
        InitNotice = initNotice;
        DaemonClient.Instance.DanmakuMessageReceived += OnDaemonMsg;
    }

    public string Name => "Daemon";
    public string? InitNotice { get; }

    public event EventHandler<DanmakuMessage>? DanmakuMessageReceived;

    public async Task<DanmakuConnectResult> ConnectAsync(string input, CancellationToken ct)
    {
        return await DaemonClient.Instance.DanmakuConnectAsync(input, ct);
    }

    public async Task DisconnectAsync(string sessionId, CancellationToken ct)
    {
        await DaemonClient.Instance.DanmakuDisconnectAsync(sessionId, ct);
    }

    public async Task<DanmakuFetchImageResult> FetchImageAsync(string sessionId, string url, CancellationToken ct)
    {
        return await DaemonClient.Instance.FetchDanmakuImageAsync(sessionId, url, ct);
    }

    public void Dispose()
    {
        DaemonClient.Instance.DanmakuMessageReceived -= OnDaemonMsg;
    }

    private void OnDaemonMsg(object? sender, DanmakuMessage msg)
    {
        _ = sender;
        DanmakuMessageReceived?.Invoke(this, msg);
    }
}

