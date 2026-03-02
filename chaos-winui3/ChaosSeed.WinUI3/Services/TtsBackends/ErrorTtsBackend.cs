using ChaosSeed.WinUI3.Models.Tts;

namespace ChaosSeed.WinUI3.Services.TtsBackends;

public sealed class ErrorTtsBackend : ITtsBackend
{
    private readonly string _name;
    private readonly string _message;

    public ErrorTtsBackend(string name, string message)
    {
        _name = name;
        _message = message;
    }

    public string Name => _name;
    public string? InitNotice => _message;

    public Task<TtsSftStartResult> StartSftAsync(TtsSftStartParams p, CancellationToken ct) =>
        Task.FromException<TtsSftStartResult>(new InvalidOperationException(_message));

    public Task<TtsSftStatus> StatusAsync(string sessionId, CancellationToken ct) =>
        Task.FromException<TtsSftStatus>(new InvalidOperationException(_message));

    public Task CancelAsync(string sessionId, CancellationToken ct) =>
        Task.FromException(new InvalidOperationException(_message));

    public void Dispose() { }
}

