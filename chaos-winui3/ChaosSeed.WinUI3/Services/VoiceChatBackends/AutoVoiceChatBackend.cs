using System.Collections.Concurrent;
using ChaosSeed.WinUI3.Models.Voice;

namespace ChaosSeed.WinUI3.Services.VoiceChatBackends;

/// <summary>
/// Auto：优先 daemon，失败回落 FFI；并记录 sessionId -> backend 映射，确保 cancel 打到同一后端。
/// </summary>
public sealed class AutoVoiceChatBackend : IVoiceChatBackend
{
    private readonly IVoiceChatBackend _daemon;
    private readonly Func<IVoiceChatBackend> _createFfi;
    private IVoiceChatBackend? _ffi;

    private readonly ConcurrentDictionary<string, string> _sessionBackend = new();

    public AutoVoiceChatBackend(IVoiceChatBackend daemon, Func<IVoiceChatBackend> createFfi)
    {
        _daemon = daemon ?? throw new ArgumentNullException(nameof(daemon));
        _createFfi = createFfi ?? throw new ArgumentNullException(nameof(createFfi));

        _daemon.VoiceChatChunkReceived += (_, msg) => ForwardChunk("daemon", msg);
    }

    public string Name => "Auto";
    public string? InitNotice => null;

    public event EventHandler<VoiceChatChunkNotif>? VoiceChatChunkReceived;

    private void ForwardChunk(string source, VoiceChatChunkNotif msg)
    {
        try
        {
            var sid = (msg.SessionId ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(sid)
                && _sessionBackend.TryGetValue(sid, out var b)
                && !string.Equals(b, source, StringComparison.OrdinalIgnoreCase))
            {
                return;
            }

            VoiceChatChunkReceived?.Invoke(this, msg);
        }
        catch
        {
            // ignore
        }
    }

    public async Task<VoiceChatStreamStartResult> StartAsync(VoiceChatStreamStartParams p, CancellationToken ct)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));

        try
        {
            var r = await _daemon.StartAsync(p, ct);
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
            var r = await ffi.StartAsync(p, ct);
            var sid = (r.SessionId ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(sid))
            {
                _sessionBackend[sid] = "ffi";
            }
            return r;
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

        return CancelUnknownAsync(sid, ct);
    }

    private async Task CancelUnknownAsync(string sid, CancellationToken ct)
    {
        try { await _daemon.CancelAsync(sid, ct); } catch { }
        try { await GetOrCreateFfi().CancelAsync(sid, ct); } catch { }
    }

    private IVoiceChatBackend GetOrCreateFfi()
    {
        var x = _ffi;
        if (x is not null) return x;
        x = _createFfi();
        x.VoiceChatChunkReceived += (_, msg) => ForwardChunk("ffi", msg);
        _ffi = x;
        return x;
    }

    public void Dispose()
    {
        try { _daemon.Dispose(); } catch { }
        try { _ffi?.Dispose(); } catch { }
    }
}

