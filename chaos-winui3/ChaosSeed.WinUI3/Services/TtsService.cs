using ChaosSeed.WinUI3.Models.Tts;

namespace ChaosSeed.WinUI3.Services;

using ChaosSeed.WinUI3.Services.TtsBackends;

public sealed class TtsService
{
    private readonly ITtsBackend _backend;
    private readonly DaemonClient? _daemon;

    private TtsService(ITtsBackend backend, DaemonClient? daemon)
    {
        _backend = backend ?? throw new ArgumentNullException(nameof(backend));
        _daemon = daemon;
    }

    public TtsService(DaemonClient daemon)
        : this(new DaemonTtsBackend(daemon), daemon)
    {
    }

    public TtsService(ITtsBackend backend)
        : this(backend, null)
    {
    }

    public static TtsService CreateWithDaemon(ITtsBackend backend, DaemonClient daemon) =>
        new(backend, daemon);

    public Task<TtsSftStartResult> StartSftAsync(TtsSftStartParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));
        return _backend.StartSftAsync(p, ct);
    }

    public Task<TtsSftStatus> StatusAsync(string sessionId, CancellationToken ct = default) =>
        _backend.StatusAsync(sessionId, ct);

    public Task CancelAsync(string sessionId, CancellationToken ct = default) =>
        _backend.CancelAsync(sessionId, ct);

    public async Task<(string SessionId, TtsAudioResult Meta, byte[] WavBytes)> SynthesizeSftToWavBytesAsync(
        TtsSftStartParams p,
        IProgress<TtsSftStatus>? progress = null,
        TimeSpan? pollInterval = null,
        CancellationToken ct = default
    )
        => await SynthesizeSftToWavBytesAsync(p, progress, pollInterval, onSessionId: null, ct);

    public async Task<(string SessionId, TtsAudioResult Meta, byte[] WavBytes)> SynthesizeSftToWavBytesAsync(
        TtsSftStartParams p,
        IProgress<TtsSftStatus>? progress,
        TimeSpan? pollInterval,
        Action<string>? onSessionId,
        CancellationToken ct = default
    )
    {
        if (p is null) throw new ArgumentNullException(nameof(p));

        var start = await _backend.StartSftAsync(p, ct);
        var sid = (start.SessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            throw new InvalidOperationException("tts.sft.start returned empty sessionId");
        }

        try { onSessionId?.Invoke(sid); } catch { }

        var interval = pollInterval ?? TimeSpan.FromMilliseconds(250);
        // 优先：daemon 推送回调（减少轮询压力）。若 daemon 版本不支持该通知，则自动回退到轮询。
        if (_daemon is not null)
        {
            var gotFirst = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            var doneTcs = new TaskCompletionSource<TtsSftStatus>(TaskCreationOptions.RunContinuationsAsynchronously);

            void OnNotif(object? _sender, TtsSftStatusNotif n)
            {
                try
                {
                    if (!string.Equals((n.SessionId ?? "").Trim(), sid, StringComparison.Ordinal))
                    {
                        return;
                    }

                    gotFirst.TrySetResult(true);
                    var st = n.Status ?? new TtsSftStatus();
                    progress?.Report(st);
                    if (st.Done)
                    {
                        doneTcs.TrySetResult(st);
                    }
                }
                catch
                {
                    // ignore
                }
            }

            _daemon.TtsSftStatusReceived += OnNotif;
            try
            {
                // 等待第一条通知；若超时则认为 daemon 不支持该通知，回退轮询。
                var first = await Task.WhenAny(gotFirst.Task, Task.Delay(TimeSpan.FromSeconds(1.2), ct));
                if (first == gotFirst.Task)
                {
                    var st = await doneTcs.Task.WaitAsync(ct);
                    return DecodeDoneStatusOrThrow(sid, st);
                }
            }
            finally
            {
                _daemon.TtsSftStatusReceived -= OnNotif;
            }
        }

        while (true)
        {
            ct.ThrowIfCancellationRequested();
            var st = await _backend.StatusAsync(sid, ct);
            progress?.Report(st);
            if (!st.Done)
            {
                await Task.Delay(interval, ct);
                continue;
            }

            return DecodeDoneStatusOrThrow(sid, st);
        }
    }

    private static (string SessionId, TtsAudioResult Meta, byte[] WavBytes) DecodeDoneStatusOrThrow(
        string sid,
        TtsSftStatus st
    )
    {
        if (!string.Equals(st.State, "done", StringComparison.OrdinalIgnoreCase))
        {
            var err = (st.Error ?? "").Trim();
            if (string.IsNullOrWhiteSpace(err))
            {
                err = $"tts job finished in state={st.State}";
            }
            throw new InvalidOperationException(err);
        }

        if (st.Result is null || string.IsNullOrWhiteSpace(st.Result.WavBase64))
        {
            throw new InvalidOperationException("tts.sft.status returned done but result.wavBase64 is empty");
        }

        byte[] wav;
        try
        {
            wav = Convert.FromBase64String(st.Result.WavBase64);
        }
        catch (FormatException ex)
        {
            throw new InvalidOperationException("invalid base64 wav payload", ex);
        }

        return (sid, st.Result, wav);
    }

    // 兼容旧代码：落盘到 LocalAppData。新 UI（TTS 调试页）默认只保存在内存中。
    public async Task<string> SynthesizeSftToWavFileAsync(
        TtsSftStartParams p,
        TimeSpan? pollInterval = null,
        CancellationToken ct = default
    )
    {
        var (sid, _meta, wav) = await SynthesizeSftToWavBytesAsync(p, null, pollInterval, ct);

        var root = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "ChaosSeed.WinUI3",
            "tts"
        );
        Directory.CreateDirectory(root);

        var path = Path.Combine(root, $"{sid}.wav");
        await File.WriteAllBytesAsync(path, wav, ct);
        return path;
    }
}
