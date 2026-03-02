using ChaosSeed.WinUI3.Models.Tts;

namespace ChaosSeed.WinUI3.Services.TtsBackends;

public interface ITtsBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    Task<TtsSftStartResult> StartSftAsync(TtsSftStartParams p, CancellationToken ct);
    Task<TtsSftStatus> StatusAsync(string sessionId, CancellationToken ct);
    Task CancelAsync(string sessionId, CancellationToken ct);
}

