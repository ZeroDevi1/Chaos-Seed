using ChaosSeed.WinUI3.Models.Voice;

namespace ChaosSeed.WinUI3.Services.VoiceChatBackends;

public sealed class DaemonVoiceChatBackend : IVoiceChatBackend
{
    private readonly DaemonClient _daemon;
    private readonly string? _initNotice;

    public DaemonVoiceChatBackend(DaemonClient daemon, string? initNotice = null)
    {
        _daemon = daemon ?? throw new ArgumentNullException(nameof(daemon));
        _initNotice = initNotice;
        _daemon.VoiceChatChunkReceived += OnDaemonChunk;
    }

    public string Name => "daemon";
    public string? InitNotice => _initNotice;

    public event EventHandler<VoiceChatChunkNotif>? VoiceChatChunkReceived;

    private void OnDaemonChunk(object? sender, VoiceChatChunkNotif msg)
    {
        _ = sender;
        try
        {
            VoiceChatChunkReceived?.Invoke(this, msg);
        }
        catch
        {
            // ignore
        }
    }

    public Task<VoiceChatStreamStartResult> StartAsync(VoiceChatStreamStartParams p, CancellationToken ct) =>
        _daemon.VoiceChatStreamStartAsync(p, ct);

    public Task CancelAsync(string sessionId, CancellationToken ct) =>
        _daemon.VoiceChatStreamCancelAsync(sessionId, ct);

    public void Dispose()
    {
        try { _daemon.VoiceChatChunkReceived -= OnDaemonChunk; } catch { }
    }
}

