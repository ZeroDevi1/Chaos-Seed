using System.Collections.Concurrent;
using ChaosSeed.WinUI3.Models.Tts;

namespace ChaosSeed.WinUI3.Services.TtsBackends;

/// <summary>
/// Auto：优先 daemon，失败回落 FFI；并记录 sessionId -> backend 映射，确保 status/cancel 打到同一后端。
/// </summary>
public sealed class AutoTtsBackend : ITtsBackend
{
    private readonly ITtsBackend _daemon;
    private readonly Func<ITtsBackend> _createFfi;
    private ITtsBackend? _ffi;

    private readonly ConcurrentDictionary<string, string> _sessionBackend = new();

    public AutoTtsBackend(ITtsBackend daemon, Func<ITtsBackend> createFfi)
    {
        _daemon = daemon ?? throw new ArgumentNullException(nameof(daemon));
        _createFfi = createFfi ?? throw new ArgumentNullException(nameof(createFfi));
    }

    public string Name => "Auto";
    public string? InitNotice => null;

    public async Task<TtsSftStartResult> StartSftAsync(TtsSftStartParams p, CancellationToken ct)
    {
        try
        {
            var r = await _daemon.StartSftAsync(p, ct);
            var sid = (r.SessionId ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(sid))
            {
                _sessionBackend[sid] = "daemon";
            }
            return r;
        }
        catch
        {
            var ffi = GetOrCreateFfi();
            var r = await ffi.StartSftAsync(p, ct);
            var sid = (r.SessionId ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(sid))
            {
                _sessionBackend[sid] = "ffi";
            }
            return r;
        }
    }

    public Task<TtsSftStatus> StatusAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        if (_sessionBackend.TryGetValue(sid, out var b))
        {
            return string.Equals(b, "ffi", StringComparison.OrdinalIgnoreCase)
                ? GetOrCreateFfi().StatusAsync(sid, ct)
                : _daemon.StatusAsync(sid, ct);
        }

        // 未知 session：优先查 daemon，失败再查 FFI（用于兼容旧会话/外部输入）。
        return StatusUnknownAsync(sid, ct);
    }

    private async Task<TtsSftStatus> StatusUnknownAsync(string sid, CancellationToken ct)
    {
        try
        {
            return await _daemon.StatusAsync(sid, ct);
        }
        catch
        {
            return await GetOrCreateFfi().StatusAsync(sid, ct);
        }
    }

    public Task CancelAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        if (_sessionBackend.TryGetValue(sid, out var b))
        {
            return string.Equals(b, "ffi", StringComparison.OrdinalIgnoreCase)
                ? GetOrCreateFfi().CancelAsync(sid, ct)
                : _daemon.CancelAsync(sid, ct);
        }

        // 未知 session：两边都尝试（忽略第一边失败）。
        return CancelUnknownAsync(sid, ct);
    }

    private async Task CancelUnknownAsync(string sid, CancellationToken ct)
    {
        try { await _daemon.CancelAsync(sid, ct); } catch { }
        try { await GetOrCreateFfi().CancelAsync(sid, ct); } catch { }
    }

    private ITtsBackend GetOrCreateFfi()
    {
        var x = _ffi;
        if (x is not null) return x;
        x = _createFfi();
        _ffi = x;
        return x;
    }

    public void Dispose()
    {
        try { _daemon.Dispose(); } catch { }
        try { _ffi?.Dispose(); } catch { }
    }
}

