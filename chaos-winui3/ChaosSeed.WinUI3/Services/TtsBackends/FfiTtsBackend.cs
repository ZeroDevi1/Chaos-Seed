using System.Text.Json;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models.Tts;

namespace ChaosSeed.WinUI3.Services.TtsBackends;

public sealed class FfiTtsBackend : ITtsBackend
{
    private static readonly JsonSerializerOptions _jsonOptions = new(JsonSerializerDefaults.Web)
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly SemaphoreSlim _ffiGate = new(1, 1);
    private readonly string? _initNotice;

    public FfiTtsBackend(string? initNotice = null)
    {
        _initNotice = initNotice;
    }

    public string Name => "FFI";
    public string? InitNotice => _initNotice;

    public async Task<TtsSftStartResult> StartSftAsync(TtsSftStartParams p, CancellationToken ct)
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
                text = (p.Text ?? "").Trim(),
                llmCkpt = string.IsNullOrWhiteSpace(p.LlmCkpt) ? null : p.LlmCkpt!.Trim(),
                flowCkpt = string.IsNullOrWhiteSpace(p.FlowCkpt) ? null : p.FlowCkpt!.Trim(),
                pythonWorkdir = string.IsNullOrWhiteSpace(p.PythonWorkdir) ? null : p.PythonWorkdir!.Trim(),
                pythonInferScript = string.IsNullOrWhiteSpace(p.PythonInferScript) ? null : p.PythonInferScript!.Trim(),
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
            };
            var jsonIn = JsonSerializer.Serialize(payload, _jsonOptions);

            var json = await Task.Run(() =>
            {
                var pJson = ChaosFfi.chaos_tts_sft_start_json(jsonIn);
                var s = ChaosFfi.TakeString(pJson);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "tts start failed"));
                }
                return s!;
            }, ct);

            return JsonSerializer.Deserialize<TtsSftStartResult>(json, _jsonOptions)
                   ?? throw new InvalidOperationException("invalid tts start json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<TtsSftStatus> StatusAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            var json = await Task.Run(() =>
            {
                var pJson = ChaosFfi.chaos_tts_sft_status_json(sid);
                var s = ChaosFfi.TakeString(pJson);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "tts status failed"));
                }
                return s!;
            }, ct);

            return JsonSerializer.Deserialize<TtsSftStatus>(json, _jsonOptions)
                   ?? throw new InvalidOperationException("invalid tts status json");
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

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            _ = await Task.Run(() =>
            {
                var pJson = ChaosFfi.chaos_tts_sft_cancel_json(sid);
                var s = ChaosFfi.TakeString(pJson);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "tts cancel failed"));
                }
                return s;
            }, ct);
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public void Dispose()
    {
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

