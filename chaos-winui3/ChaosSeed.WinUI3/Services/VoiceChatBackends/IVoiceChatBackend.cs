using ChaosSeed.WinUI3.Models.Voice;

namespace ChaosSeed.WinUI3.Services.VoiceChatBackends;

public interface IVoiceChatBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    event EventHandler<VoiceChatChunkNotif>? VoiceChatChunkReceived;

    Task<VoiceChatStreamStartResult> StartAsync(VoiceChatStreamStartParams p, CancellationToken ct);
    Task CancelAsync(string sessionId, CancellationToken ct);
}

