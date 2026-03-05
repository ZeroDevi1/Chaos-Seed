using ChaosSeed.WinUI3.Models.Voice;

namespace ChaosSeed.WinUI3.Services.VoiceChatBackends;

public sealed class ErrorVoiceChatBackend : IVoiceChatBackend
{
    private readonly string _name;
    private readonly string _message;

    public ErrorVoiceChatBackend(string name, string message)
    {
        _name = name;
        _message = message;
    }

    public string Name => _name;
    public string? InitNotice => _message;

    public event EventHandler<VoiceChatChunkNotif>? VoiceChatChunkReceived
    {
        add { }
        remove { }
    }

    public Task<VoiceChatStreamStartResult> StartAsync(VoiceChatStreamStartParams p, CancellationToken ct) =>
        Task.FromException<VoiceChatStreamStartResult>(new InvalidOperationException(_message));

    public Task CancelAsync(string sessionId, CancellationToken ct) =>
        Task.FromException(new InvalidOperationException(_message));

    public void Dispose() { }
}

