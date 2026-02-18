using System.Text.Json;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models.Bili;
using ChaosSeed.WinUI3.Models.Music;
using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Services.BiliBackends;

public sealed class FfiBiliBackend : IBiliBackend
{
    private readonly SemaphoreSlim _ffiGate = new(1, 1);

    public string Name => "FFI";
    public string? InitNotice => null;

    public async Task<BiliLoginQr> LoginQrCreateAsync(CancellationToken ct)
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_bili_login_qr_create_json();
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "bili login qr create failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<BiliLoginQr>(json) ?? throw new InvalidOperationException("invalid BiliLoginQr json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<BiliLoginQrPollResult> LoginQrPollAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_bili_login_qr_poll_json(sid);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "bili login qr poll failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<BiliLoginQrPollResult>(json) ?? throw new InvalidOperationException("invalid BiliLoginQrPollResult json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public Task<BiliRefreshCookieResult> RefreshCookieAsync(BiliRefreshCookieParams p, CancellationToken ct)
        => InvokeObjectAsync<BiliRefreshCookieResult>(
            "refreshCookie",
            json => ChaosFfi.chaos_bili_refresh_cookie_json(json),
            p,
            ct
        );

    public Task<BiliParseResult> ParseAsync(BiliParseParams p, CancellationToken ct)
        => InvokeObjectAsync<BiliParseResult>(
            "parse",
            json => ChaosFfi.chaos_bili_parse_json(json),
            p,
            ct
        );

    public Task<BiliDownloadStartResult> DownloadStartAsync(BiliDownloadStartParams p, CancellationToken ct)
        => InvokeObjectAsync<BiliDownloadStartResult>(
            "downloadStart",
            json => ChaosFfi.chaos_bili_download_start_json(json),
            p,
            ct
        );

    public Task<BiliDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct)
        => InvokeSessionAsync<BiliDownloadStatus>(
            "downloadStatus",
            sid => ChaosFfi.chaos_bili_download_status_json(sid),
            sessionId,
            ct
        );

    public Task CancelDownloadAsync(string sessionId, CancellationToken ct)
        => InvokeSessionAsync<OkReply>(
            "downloadCancel",
            sid => ChaosFfi.chaos_bili_download_cancel_json(sid),
            sessionId,
            ct
        );

    public void Dispose()
    {
        _ffiGate.Dispose();
    }

    private async Task<T> InvokeObjectAsync<T>(
        string opName,
        Func<string, IntPtr> ffiFunc,
        object payload,
        CancellationToken ct
    )
    {
        if (payload is null) throw new ArgumentNullException(nameof(payload));
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var jsonPayload = JsonConvert.SerializeObject(payload);
            var json = await Task.Run(() =>
            {
                var p = ffiFunc(jsonPayload);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, $"bili.{opName} failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<T>(json) ?? throw new InvalidOperationException($"invalid json: {typeof(T).Name}");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    private async Task<T> InvokeSessionAsync<T>(
        string opName,
        Func<string, IntPtr> ffiFunc,
        string sessionId,
        CancellationToken ct
    )
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ffiFunc(sid);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, $"bili.{opName} failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<T>(json) ?? throw new InvalidOperationException($"invalid json: {typeof(T).Name}");
        }
        finally
        {
            _ffiGate.Release();
        }
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
            var msg = root.TryGetProperty("message", out var msgEl) ? (msgEl.GetString() ?? "") : "";
            return string.IsNullOrWhiteSpace(msg) ? fallback : msg;
        }
        catch
        {
            return fallback;
        }
    }
}
