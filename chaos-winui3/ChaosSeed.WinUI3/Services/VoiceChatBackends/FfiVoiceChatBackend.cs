using System.Collections.Concurrent;
using System.Text.Json;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models.Voice;

namespace ChaosSeed.WinUI3.Services.VoiceChatBackends;

public sealed class FfiVoiceChatBackend : IVoiceChatBackend
{
    private static readonly JsonSerializerOptions _jsonOptions = new(JsonSerializerDefaults.Web)
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly SemaphoreSlim _ffiGate = new(1, 1);
    private readonly ConcurrentDictionary<string, CancellationTokenSource> _pollCts = new();
    private readonly string? _initNotice;

    public FfiVoiceChatBackend(string? initNotice = null)
    {
        _initNotice = initNotice;
    }

    public string Name => "FFI";
    public string? InitNotice => _initNotice;

    public event EventHandler<VoiceChatChunkNotif>? VoiceChatChunkReceived;

    private sealed class PollResult
    {
        public bool HasChunk { get; set; }
        public VoiceChatChunkNotif? Chunk { get; set; }
    }

    public async Task<VoiceChatStreamStartResult> StartAsync(VoiceChatStreamStartParams p, CancellationToken ct)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            var payload = new
            {
                modelDir = (p.ModelDir ?? "").Trim(),
                spkId = (p.SpkId ?? "").Trim(),
                messages = p.Messages,
                reasoningMode = p.ReasoningMode,

                promptText = p.PromptText,
                promptStrategy = p.PromptStrategy,
                guideSep = p.GuideSep,
                speed = p.Speed,
                seed = p.Seed,
                temperature = p.Temperature,
                topP = p.TopP,
                topK = p.TopK,
                winSize = p.WinSize,
                tauR = p.TauR,
                textFrontend = p.TextFrontend,
                chunkMs = p.ChunkMs,
            };
            var jsonIn = JsonSerializer.Serialize(payload, _jsonOptions);

            var json = await Task.Run(() =>
            {
                var pJson = ChaosFfi.chaos_voice_chat_stream_start_json(jsonIn);
                var s = ChaosFfi.TakeString(pJson);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "voice chat start failed"));
                }
                return s!;
            }, ct);

            var res = JsonSerializer.Deserialize<VoiceChatStreamStartResult>(json, _jsonOptions)
                      ?? throw new InvalidOperationException("invalid voice chat start json");

            var sid = (res.SessionId ?? "").Trim();
            if (string.IsNullOrWhiteSpace(sid))
            {
                throw new InvalidOperationException("ffi voice chat returned empty sessionId");
            }

            StartPolling(sid);
            return res;
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    private void StartPolling(string sessionId)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) return;

        var cts = new CancellationTokenSource();
        if (!_pollCts.TryAdd(sid, cts))
        {
            try { cts.Dispose(); } catch { }
            return;
        }

        _ = Task.Run(async () =>
        {
            try
            {
                var ct = cts.Token;
                while (!ct.IsCancellationRequested)
                {
                    PollResult? pr = null;
                    try
                    {
                        pr = await PollOnceAsync(sid, ct);
                    }
                    catch
                    {
                        // Poll errors: stop this session and let UI close.
                        VoiceChatChunkReceived?.Invoke(this, new VoiceChatChunkNotif
                        {
                            SessionId = sid,
                            Seq = 0,
                            PcmBase64 = "",
                            IsLast = true,
                        });
                        return;
                    }

                    if (pr is not null && pr.HasChunk && pr.Chunk is not null)
                    {
                        VoiceChatChunkReceived?.Invoke(this, pr.Chunk);
                        if (pr.Chunk.IsLast)
                        {
                            return;
                        }
                        continue;
                    }

                    await Task.Delay(TimeSpan.FromMilliseconds(15), ct);
                }
            }
            catch
            {
                // ignore
            }
            finally
            {
                CleanupSession(sid);
            }
        });
    }

    private async Task<PollResult> PollOnceAsync(string sid, CancellationToken ct)
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            var json = await Task.Run(() =>
            {
                var pJson = ChaosFfi.chaos_voice_chat_stream_poll_json(sid);
                var s = ChaosFfi.TakeString(pJson);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "voice chat poll failed"));
                }
                return s!;
            }, ct);

            return JsonSerializer.Deserialize<PollResult>(json, _jsonOptions)
                   ?? new PollResult { HasChunk = false, Chunk = null };
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task CancelAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        if (_pollCts.TryRemove(sid, out var cts))
        {
            try { cts.Cancel(); } catch { }
            try { cts.Dispose(); } catch { }
        }

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            _ = await Task.Run(() =>
            {
                var pJson = ChaosFfi.chaos_voice_chat_stream_cancel_json(sid);
                var s = ChaosFfi.TakeString(pJson);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "voice chat cancel failed"));
                }
                return s!;
            }, ct);
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    private void CleanupSession(string sid)
    {
        if (_pollCts.TryRemove(sid, out var cts))
        {
            try { cts.Dispose(); } catch { }
        }
    }

    public void Dispose()
    {
        foreach (var kv in _pollCts)
        {
            try { kv.Value.Cancel(); } catch { }
        }

        foreach (var kv in _pollCts)
        {
            try { kv.Value.Dispose(); } catch { }
        }

        _pollCts.Clear();
        _ffiGate.Dispose();
    }

    private static string FormatFfiError(string? errJson, string fallback)
    {
        if (string.IsNullOrWhiteSpace(errJson))
        {
            return fallback;
        }

        try
        {
            using var doc = JsonDocument.Parse(errJson);
            var root = doc.RootElement;
            var message = root.TryGetProperty("message", out var m) ? (m.GetString() ?? "") : "";
            var context = root.TryGetProperty("context", out var c) ? (c.GetString() ?? "") : "";

            message = message.Trim();
            context = context.Trim();

            if (!string.IsNullOrWhiteSpace(message) && !string.IsNullOrWhiteSpace(context))
            {
                return $"{message}\n{context}";
            }
            if (!string.IsNullOrWhiteSpace(message))
            {
                return message;
            }
            if (!string.IsNullOrWhiteSpace(context))
            {
                return context;
            }
        }
        catch
        {
            // ignore
        }

        return fallback;
    }
}

