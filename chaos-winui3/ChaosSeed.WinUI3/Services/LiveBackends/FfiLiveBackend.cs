using System.Text.Json;
using System.Runtime.InteropServices;
using System.Net;
using System.Net.Sockets;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Models.Ffi;

namespace ChaosSeed.WinUI3.Services.LiveBackends;

public sealed class FfiLiveBackend : ILiveBackend
{
    private static readonly JsonSerializerOptions _jsonOptions = new(JsonSerializerDefaults.Web)
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly SemaphoreSlim _ffiGate = new(1, 1);
    private readonly HttpClient _http;
    private readonly bool _ownsHttp;

    private FfiLiveManifest? _lastManifestRaw;
    private LivestreamDecodeManifestResult? _lastManifest;
    private string? _activeSessionId;
    private IntPtr _danmakuHandle;
    private ChaosFfi.chaos_danmaku_callback? _danmakuCallback;

    public FfiLiveBackend(HttpClient? httpClient = null)
    {
        _ownsHttp = httpClient is null;
        _http = httpClient ?? new HttpClient(new HttpClientHandler
        {
            AutomaticDecompression = System.Net.DecompressionMethods.All,
        });
    }

    public string Name => "FFI";

    public string? InitNotice => null;

    public event EventHandler<DanmakuMessage>? DanmakuMessageReceived;

    public async Task<LivestreamDecodeManifestResult> DecodeManifestAsync(string input, CancellationToken ct)
    {
        if (string.IsNullOrWhiteSpace(input))
        {
            throw new ArgumentException("empty input", nameof(input));
        }

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_livestream_decode_manifest_json(input.Trim(), 1);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "decodeManifest failed"));
                }
                return s!;
            }, ct);

            var man = JsonSerializer.Deserialize<FfiLiveManifest>(json, _jsonOptions)
                ?? throw new InvalidOperationException("invalid manifest json");

            var mapped = MapManifest(man);
            _lastManifestRaw = man;
            _lastManifest = mapped;
            return mapped;
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<LiveOpenResult> OpenLiveAsync(string input, string? variantId, CancellationToken ct)
    {
        if (string.IsNullOrWhiteSpace(input))
        {
            throw new ArgumentException("empty input", nameof(input));
        }

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            var manRaw = _lastManifestRaw;
            var man = _lastManifest;
            if (manRaw is null || man is null)
            {
                throw new InvalidOperationException("请先解析直播源（DecodeManifestAsync）再打开播放。");
            }

            var requestedId = (variantId ?? "").Trim();
            var variants = manRaw.Variants ?? new List<FfiStreamVariant>();

            FfiStreamVariant picked;
            if (!string.IsNullOrWhiteSpace(requestedId))
            {
                picked = variants.FirstOrDefault(v => string.Equals((v.Id ?? "").Trim(), requestedId, StringComparison.Ordinal))
                    ?? throw new InvalidOperationException($"variant not found: {requestedId}");
            }
            else
            {
                picked = variants
                    .OrderByDescending(v => v.Quality)
                    .FirstOrDefault(v => !string.IsNullOrWhiteSpace(v.Url) || (v.BackupUrls?.Count ?? 0) > 0)
                    ?? variants.OrderByDescending(v => v.Quality).FirstOrDefault()
                    ?? throw new InvalidOperationException("no variants");
            }

            var final = picked;
            if (string.IsNullOrWhiteSpace(final.Url) && (final.BackupUrls?.Count ?? 0) == 0)
            {
                if (string.IsNullOrWhiteSpace(picked.Id))
                {
                    throw new InvalidOperationException("variant id missing");
                }

                var resolvedJson = await Task.Run(() =>
                {
                    var p = ChaosFfi.chaos_livestream_resolve_variant2_json(
                        (manRaw.Site ?? "").Trim(),
                        (manRaw.RoomId ?? "").Trim(),
                        picked.Id.Trim()
                    );
                    var s = ChaosFfi.TakeString(p);
                    if (string.IsNullOrWhiteSpace(s))
                    {
                        var err = ChaosFfi.TakeLastErrorJson();
                        throw new InvalidOperationException(FormatFfiError(err, "resolveVariant failed"));
                    }
                    return s!;
                }, ct);

                final = JsonSerializer.Deserialize<FfiStreamVariant>(resolvedJson, _jsonOptions)
                    ?? throw new InvalidOperationException("invalid variant json");
            }

            var url = (final.Url ?? "").Trim();
            var backups = (final.BackupUrls ?? new List<string>())
                .Select(u => (u ?? "").Trim())
                .Where(u => !string.IsNullOrWhiteSpace(u))
                .ToArray();

            if (string.IsNullOrWhiteSpace(url) && backups.Length == 0)
            {
                throw new InvalidOperationException("empty url");
            }

            await DisconnectDanmakuUnsafeAsync();

            var sessionId = "ffi-" + Guid.NewGuid().ToString("N");
            _activeSessionId = sessionId;

            try
            {
                await Task.Run(() =>
                {
                    var handle = IntPtr.Zero;
                    try
                    {
                        handle = ChaosFfi.chaos_danmaku_connect(input.Trim());
                        if (handle == IntPtr.Zero)
                        {
                            var err = ChaosFfi.TakeLastErrorJson();
                            throw new InvalidOperationException(FormatFfiError(err, "danmaku connect failed"));
                        }

                        ChaosFfi.chaos_danmaku_callback cb = OnDanmakuCallback;
                        var rc = ChaosFfi.chaos_danmaku_set_callback(handle, cb, IntPtr.Zero);
                        if (rc != 0)
                        {
                            var err = ChaosFfi.TakeLastErrorJson();
                            throw new InvalidOperationException(FormatFfiError(err, "danmaku set_callback failed"));
                        }

                        _danmakuHandle = handle;
                        _danmakuCallback = cb;
                        handle = IntPtr.Zero;
                    }
                    finally
                    {
                        if (handle != IntPtr.Zero)
                        {
                            try { ChaosFfi.chaos_danmaku_disconnect(handle); } catch { }
                        }
                    }
                }, ct);
            }
            catch
            {
                _activeSessionId = null;
                throw;
            }

            return new LiveOpenResult
            {
                SessionId = sessionId,
                Site = man.Site ?? "",
                RoomId = man.RoomId ?? "",
                Title = man.Info?.Title ?? "",
                VariantId = (final.Id ?? picked.Id ?? requestedId).Trim(),
                VariantLabel = (final.Label ?? picked.Label ?? "").Trim(),
                Url = url,
                BackupUrls = backups,
                Referer = man.Playback?.Referer,
                UserAgent = man.Playback?.UserAgent,
            };
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task CloseLiveAsync(string sessionId, CancellationToken ct)
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            if (!string.Equals((_activeSessionId ?? "").Trim(), (sessionId ?? "").Trim(), StringComparison.Ordinal))
            {
                return;
            }

            await DisconnectDanmakuUnsafeAsync();
            _activeSessionId = null;
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<DanmakuFetchImageResult> FetchDanmakuImageAsync(string sessionId, string url, CancellationToken ct)
    {
        if (!string.Equals((_activeSessionId ?? "").Trim(), (sessionId ?? "").Trim(), StringComparison.Ordinal))
        {
            return new DanmakuFetchImageResult();
        }

        var raw = (url ?? "").Trim();
        if (string.IsNullOrWhiteSpace(raw))
        {
            return new DanmakuFetchImageResult();
        }

        if (raw.StartsWith("//", StringComparison.Ordinal))
        {
            raw = "https:" + raw;
        }

        if (!Uri.TryCreate(raw, UriKind.Absolute, out var u))
        {
            return new DanmakuFetchImageResult();
        }

        if (!string.Equals(u.Scheme, "http", StringComparison.OrdinalIgnoreCase) &&
            !string.Equals(u.Scheme, "https", StringComparison.OrdinalIgnoreCase))
        {
            return new DanmakuFetchImageResult();
        }

        if (IsLocalOrPrivateHost(u))
        {
            return new DanmakuFetchImageResult();
        }

        const int maxBytes = 2_500_000;

        using var req = new HttpRequestMessage(HttpMethod.Get, u);
        ApplyImageHeaders(req, u);

        using var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, ct);
        resp.EnsureSuccessStatusCode();

        if (resp.Content.Headers.ContentLength is long len && len > maxBytes)
        {
            return new DanmakuFetchImageResult();
        }

        var contentType = resp.Content.Headers.ContentType?.MediaType;
        using var stream = await resp.Content.ReadAsStreamAsync(ct);

        var bytes = await ReadUpToAsync(stream, maxBytes, ct);
        if (bytes.Length == 0)
        {
            return new DanmakuFetchImageResult();
        }

        return new DanmakuFetchImageResult
        {
            Mime = string.IsNullOrWhiteSpace(contentType) ? "image/png" : contentType!,
            Base64 = Convert.ToBase64String(bytes),
        };
    }

    public void Dispose()
    {
        try
        {
            _ffiGate.Wait();
            try
            {
                DisconnectDanmakuUnsafeAsync().GetAwaiter().GetResult();
                _activeSessionId = null;
            }
            finally
            {
                _ffiGate.Release();
            }
        }
        catch
        {
            // ignore
        }

        if (_ownsHttp)
        {
            try { _http.Dispose(); } catch { }
        }
        _ffiGate.Dispose();
    }

    private void ApplyPlaybackHeaders(HttpRequestMessage req)
    {
        var playback = _lastManifest?.Playback;
        if (playback is null)
        {
            return;
        }

        if (!string.IsNullOrWhiteSpace(playback.Referer))
        {
            if (Uri.TryCreate(playback.Referer.Trim(), UriKind.Absolute, out var uri))
            {
                req.Headers.Referrer = uri;
            }
            req.Headers.TryAddWithoutValidation("Referer", playback.Referer.Trim());
        }

        if (!string.IsNullOrWhiteSpace(playback.UserAgent))
        {
            req.Headers.TryAddWithoutValidation("User-Agent", playback.UserAgent.Trim());
        }
    }

    private void ApplyImageHeaders(HttpRequestMessage req, Uri u)
    {
        try
        {
            var host = (u.Host ?? "").Trim().ToLowerInvariant();
            var site = (_lastManifest?.Site ?? "").Trim().ToLowerInvariant();
            var roomId = (_lastManifest?.RoomId ?? "").Trim();
            var playback = _lastManifest?.Playback;

            string? referer = null;
            if (site.Contains("bili") || host.Contains("bilibili.com") || host.Contains("hdslb.com"))
            {
                referer = string.IsNullOrWhiteSpace(roomId)
                    ? "https://live.bilibili.com/"
                    : $"https://live.bilibili.com/{roomId}/";
            }
            else if (!string.IsNullOrWhiteSpace(playback?.Referer))
            {
                referer = playback!.Referer!.Trim();
            }

            if (!string.IsNullOrWhiteSpace(referer))
            {
                try { req.Headers.Remove("Referer"); } catch { }
                if (Uri.TryCreate(referer.Trim(), UriKind.Absolute, out var uri))
                {
                    req.Headers.Referrer = uri;
                }
                req.Headers.TryAddWithoutValidation("Referer", referer.Trim());
            }

            var ua = !string.IsNullOrWhiteSpace(playback?.UserAgent)
                ? playback!.UserAgent!.Trim()
                : "chaos-seed/winui3";
            if (!string.IsNullOrWhiteSpace(ua))
            {
                try { req.Headers.Remove("User-Agent"); } catch { }
                req.Headers.TryAddWithoutValidation("User-Agent", ua);
            }
        }
        catch
        {
            // ignore
        }
    }

    private static bool IsLocalOrPrivateHost(Uri u)
    {
        var host = (u.Host ?? "").Trim();
        if (host.Length == 0)
        {
            return true;
        }

        if (string.Equals(host, "localhost", StringComparison.OrdinalIgnoreCase))
        {
            return true;
        }

        if (!IPAddress.TryParse(host, out var ip))
        {
            return false;
        }

        if (IPAddress.IsLoopback(ip))
        {
            return true;
        }

        if (ip.AddressFamily == AddressFamily.InterNetwork)
        {
            return IsPrivateOrLinkLocalIpv4(ip);
        }

        if (ip.AddressFamily == AddressFamily.InterNetworkV6)
        {
            if (ip.IsIPv4MappedToIPv6)
            {
                return IsPrivateOrLinkLocalIpv4(ip.MapToIPv4());
            }
            if (ip.IsIPv6LinkLocal || ip.IsIPv6SiteLocal)
            {
                return true;
            }
            return IsUniqueLocalIpv6(ip);
        }

        return true;
    }

    private static bool IsPrivateOrLinkLocalIpv4(IPAddress ip)
    {
        var b = ip.GetAddressBytes();
        if (b.Length != 4)
        {
            return true;
        }

        // 10.0.0.0/8
        if (b[0] == 10)
        {
            return true;
        }

        // 172.16.0.0/12
        if (b[0] == 172 && b[1] >= 16 && b[1] <= 31)
        {
            return true;
        }

        // 192.168.0.0/16
        if (b[0] == 192 && b[1] == 168)
        {
            return true;
        }

        // 169.254.0.0/16 (link-local)
        if (b[0] == 169 && b[1] == 254)
        {
            return true;
        }

        return false;
    }

    private static bool IsUniqueLocalIpv6(IPAddress ip)
    {
        var b = ip.GetAddressBytes();
        if (b.Length != 16)
        {
            return true;
        }

        // fc00::/7
        return (b[0] & 0xFE) == 0xFC;
    }

    private async Task DisconnectDanmakuUnsafeAsync()
    {
        var handle = _danmakuHandle;
        _danmakuHandle = IntPtr.Zero;
        _danmakuCallback = null;

        if (handle == IntPtr.Zero)
        {
            return;
        }

        await Task.Run(() =>
        {
            try
            {
                ChaosFfi.chaos_danmaku_set_callback(handle, null, IntPtr.Zero);
            }
            catch
            {
                // ignore
            }

            try
            {
                ChaosFfi.chaos_danmaku_disconnect(handle);
            }
            catch
            {
                // ignore
            }
        });
    }

    private void OnDanmakuCallback(IntPtr eventJsonUtf8, IntPtr userData)
    {
        _ = userData;
        if (eventJsonUtf8 == IntPtr.Zero)
        {
            return;
        }

        var sid = _activeSessionId;
        if (string.IsNullOrWhiteSpace(sid))
        {
            return;
        }

        try
        {
            var json = Marshal.PtrToStringUTF8(eventJsonUtf8);
            if (string.IsNullOrWhiteSpace(json))
            {
                return;
            }

            var ev = JsonSerializer.Deserialize<FfiDanmakuEvent>(json!, _jsonOptions);
            if (ev is null)
            {
                return;
            }

            if (!string.Equals((ev.Method ?? "").Trim(), "SendDM", StringComparison.Ordinal))
            {
                return;
            }

            var dm0 = ev.Dms?.FirstOrDefault();
            var imageUrl = string.IsNullOrWhiteSpace(dm0?.ImageUrl) ? null : dm0!.ImageUrl!.Trim();
            var text = (dm0?.Text ?? ev.Text ?? "").Trim();
            if (string.IsNullOrWhiteSpace(text) && string.IsNullOrWhiteSpace(imageUrl))
            {
                return;
            }

            DanmakuMessageReceived?.Invoke(this, new DanmakuMessage
            {
                SessionId = sid!,
                ReceivedAtMs = ev.ReceivedAtMs,
                User = (ev.User ?? "").Trim(),
                Text = string.IsNullOrWhiteSpace(text) ? "[图片]" : text,
                ImageUrl = imageUrl,
                ImageWidth = dm0?.ImageWidth,
            });
        }
        catch
        {
            // ignore callback failures
        }
    }

    private static LivestreamDecodeManifestResult MapManifest(FfiLiveManifest man)
    {
        var mapped = new LivestreamDecodeManifestResult
        {
            Site = (man.Site ?? "").Trim(),
            RoomId = (man.RoomId ?? "").Trim(),
            RawInput = (man.RawInput ?? "").Trim(),
            Info = new LiveInfo
            {
                Title = (man.Info?.Title ?? "").Trim(),
                Name = man.Info?.Name,
                Avatar = man.Info?.Avatar,
                Cover = man.Info?.Cover,
                IsLiving = man.Info?.IsLiving == true,
            },
            Playback = new PlaybackHints
            {
                Referer = man.Playback?.Referer,
                UserAgent = man.Playback?.UserAgent,
            },
            Variants = (man.Variants ?? new List<FfiStreamVariant>())
                .Select(v => new StreamVariant
                {
                    Id = (v.Id ?? "").Trim(),
                    Label = (v.Label ?? "").Trim(),
                    Quality = v.Quality,
                    Rate = v.Rate,
                    Url = v.Url,
                    BackupUrls = (v.BackupUrls ?? new List<string>())
                        .Select(u => (u ?? "").Trim())
                        .Where(u => !string.IsNullOrWhiteSpace(u))
                        .ToArray(),
                })
                .ToArray(),
        };

        return mapped;
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
            // ignore, fall back to raw json
        }

        return errJson.Trim();
    }

    private static async Task<byte[]> ReadUpToAsync(Stream stream, int maxBytes, CancellationToken ct)
    {
        using var ms = new MemoryStream();
        var buf = new byte[16 * 1024];
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            var n = await stream.ReadAsync(buf.AsMemory(0, buf.Length), ct);
            if (n <= 0)
            {
                break;
            }

            if (ms.Length + n > maxBytes)
            {
                return Array.Empty<byte>();
            }

            ms.Write(buf, 0, n);
        }

        return ms.ToArray();
    }
}
