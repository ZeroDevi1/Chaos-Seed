using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services.DanmakuBackends;

namespace ChaosSeed.WinUI3.Services;

public sealed class DanmakuService
{
    public static DanmakuService Instance { get; } = new();

    private readonly SemaphoreSlim _gate = new(1, 1);
    private IDanmakuBackend? _backend;

    private DanmakuService()
    {
        StatusText = "未连接";
    }

    public string? CurrentSessionId { get; private set; }
    public string? CurrentSite { get; private set; }
    public string? CurrentRoomId { get; private set; }

    public string StatusText { get; private set; }

    public string BackendName => _backend?.Name ?? "N/A";
    public string? BackendInitNotice => _backend?.InitNotice;

    public event EventHandler<DanmakuMessage>? Message;
    public event EventHandler<string>? StatusChanged;

    public async Task ConnectAsync(string input, CancellationToken ct)
    {
        await _gate.WaitAsync(ct);
        try
        {
            await DisconnectCoreAsync(ct, keepBackend: true);

            StatusText = "正在连接...";
            StatusChanged?.Invoke(this, StatusText);

            SwapBackend(DanmakuBackendFactory.Create());

            if (_backend is null)
            {
                throw new InvalidOperationException("danmaku backend not initialized");
            }

            var res = await _backend.ConnectAsync(input, ct);

            CurrentSessionId = res.SessionId;
            CurrentSite = res.Site;
            CurrentRoomId = res.RoomId;

            StatusText = "已连接";
            StatusChanged?.Invoke(this, StatusText);
        }
        finally
        {
            _gate.Release();
        }
    }

    public async Task DisconnectAsync(CancellationToken ct)
    {
        await _gate.WaitAsync(ct);
        try
        {
            await DisconnectCoreAsync(ct, keepBackend: false);
        }
        finally
        {
            _gate.Release();
        }
    }

    public async Task<DanmakuFetchImageResult> FetchImageAsync(string sessionId, string url, CancellationToken ct)
    {
        var b = _backend;
        if (b is null)
        {
            return new DanmakuFetchImageResult();
        }

        return await b.FetchImageAsync(sessionId, url, ct);
    }

    private async Task DisconnectCoreAsync(CancellationToken ct, bool keepBackend)
    {
        var sid = (CurrentSessionId ?? "").Trim();
        CurrentSessionId = null;
        CurrentSite = null;
        CurrentRoomId = null;

        if (!string.IsNullOrWhiteSpace(sid) && _backend is not null)
        {
            try
            {
                await _backend.DisconnectAsync(sid, ct);
            }
            catch
            {
                // ignore disconnect failures
            }
        }

        if (!keepBackend)
        {
            SwapBackend(null);
        }

        StatusText = "已断开";
        StatusChanged?.Invoke(this, StatusText);
    }

    private void SwapBackend(IDanmakuBackend? next)
    {
        if (ReferenceEquals(_backend, next))
        {
            return;
        }

        if (_backend is not null)
        {
            _backend.DanmakuMessageReceived -= OnBackendMsg;
            try
            {
                _backend.Dispose();
            }
            catch
            {
                // ignore
            }
        }

        _backend = next;
        if (_backend is not null)
        {
            _backend.DanmakuMessageReceived += OnBackendMsg;
        }
    }

    private void OnBackendMsg(object? sender, DanmakuMessage msg)
    {
        _ = sender;
        var sid = CurrentSessionId;
        if (string.IsNullOrWhiteSpace(sid))
        {
            return;
        }
        if (!string.Equals(msg.SessionId, sid, StringComparison.Ordinal))
        {
            return;
        }

        Message?.Invoke(this, msg);
    }
}

