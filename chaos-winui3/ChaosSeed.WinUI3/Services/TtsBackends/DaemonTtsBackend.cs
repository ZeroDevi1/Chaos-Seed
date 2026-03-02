using ChaosSeed.WinUI3.Models.Tts;

namespace ChaosSeed.WinUI3.Services.TtsBackends;

public sealed class DaemonTtsBackend : ITtsBackend
{
    private readonly DaemonClient _daemon;
    private readonly string? _initNotice;

    public DaemonTtsBackend(DaemonClient daemon, string? initNotice = null)
    {
        _daemon = daemon ?? throw new ArgumentNullException(nameof(daemon));
        _initNotice = initNotice;
    }

    public string Name => "daemon";
    public string? InitNotice => _initNotice;

    public Task<TtsSftStartResult> StartSftAsync(TtsSftStartParams p, CancellationToken ct) =>
        _daemon.TtsSftStartAsync(p, ct);

    public Task<TtsSftStatus> StatusAsync(string sessionId, CancellationToken ct) =>
        _daemon.TtsSftStatusAsync(sessionId, ct);

    public Task CancelAsync(string sessionId, CancellationToken ct) => _daemon.TtsSftCancelAsync(sessionId, ct);

    public void Dispose()
    {
        // daemon client is a singleton; do not dispose here.
    }
}

