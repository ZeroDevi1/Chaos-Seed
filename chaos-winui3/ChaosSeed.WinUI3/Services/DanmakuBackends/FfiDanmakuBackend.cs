using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Text.Json;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Models.Ffi;

namespace ChaosSeed.WinUI3.Services.DanmakuBackends;

public sealed class FfiDanmakuBackend : IDanmakuBackend
{
    public string Name => "FFI";
    public string? InitNotice => null;

    public event EventHandler<DanmakuMessage>? DanmakuMessageReceived;

    private readonly SemaphoreSlim _ffiGate = new(1, 1);
    private readonly HttpClient _http;
    private readonly bool _ownsHttp;
    private readonly JsonSerializerOptions _jsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    private IntPtr _danmakuHandle;
    private ChaosFfi.chaos_danmaku_callback? _danmakuCallback;
    private string? _activeSessionId;
    private string? _activeSite;
    private string? _activeRoomId;

    public FfiDanmakuBackend(HttpClient? http = null)
    {
        if (http is null)
        {
            _http = new HttpClient(new SocketsHttpHandler
            {
                PooledConnectionLifetime = TimeSpan.FromMinutes(3),
                AutomaticDecompression = DecompressionMethods.GZip | DecompressionMethods.Deflate | DecompressionMethods.Brotli,
            })
            {
                Timeout = TimeSpan.FromSeconds(12),
            };
            _ownsHttp = true;
        }
        else
        {
            _http = http;
            _ownsHttp = false;
        }
    }

    public async Task<DanmakuConnectResult> ConnectAsync(string input, CancellationToken ct)
    {
        var raw = (input ?? "").Trim();
        if (string.IsNullOrWhiteSpace(raw))
        {
            throw new ArgumentException("empty input", nameof(input));
        }

        await _ffiGate.WaitAsync(ct);
        try
        {
            await DisconnectUnsafeAsync();

            // Resolve site/roomId for headers and UI display.
            string? site = null;
            string? roomId = null;
            try
            {
                var manJson = await Task.Run(
                    () => ChaosFfi.TakeString(ChaosFfi.chaos_livestream_decode_manifest_json(raw, 0)),
                    ct
                );
                if (!string.IsNullOrWhiteSpace(manJson))
                {
                    var man = JsonSerializer.Deserialize<FfiLiveManifest>(manJson!, _jsonOptions);
                    site = (man?.Site ?? "").Trim();
                    roomId = (man?.RoomId ?? "").Trim();
                }
            }
            catch
            {
                // best effort; connect may still work
            }

            var handle = await Task.Run(() => ChaosFfi.chaos_danmaku_connect(raw), ct);
            if (handle == IntPtr.Zero)
            {
                var err = FormatFfiError(ChaosFfi.TakeLastErrorJson(), "danmaku connect failed");
                throw new Exception(err);
            }

            var sid = $"ffi-dm-{Guid.NewGuid():N}";

            _danmakuHandle = handle;
            _activeSessionId = sid;
            _activeSite = string.IsNullOrWhiteSpace(site) ? null : site;
            _activeRoomId = string.IsNullOrWhiteSpace(roomId) ? null : roomId;

            _danmakuCallback = OnDanmakuCallback;
            var ok = await Task.Run(
                () => ChaosFfi.chaos_danmaku_set_callback(handle, _danmakuCallback, IntPtr.Zero),
                ct
            );
            if (ok != 0)
            {
                await DisconnectUnsafeAsync();
                throw new Exception("danmaku callback registration failed");
            }

            return new DanmakuConnectResult
            {
                SessionId = sid,
                Site = _activeSite ?? "",
                RoomId = _activeRoomId ?? "",
            };
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task DisconnectAsync(string sessionId, CancellationToken ct)
    {
        _ = ct;
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            return;
        }

        await _ffiGate.WaitAsync(ct);
        try
        {
            if (!string.Equals(_activeSessionId, sid, StringComparison.Ordinal))
            {
                return;
            }
            await DisconnectUnsafeAsync();
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<DanmakuFetchImageResult> FetchImageAsync(string sessionId, string url, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (!string.Equals(_activeSessionId, sid, StringComparison.Ordinal))
        {
            return new DanmakuFetchImageResult();
        }

        var u = (url ?? "").Trim();
        if (!Uri.TryCreate(u, UriKind.Absolute, out var uri))
        {
            return new DanmakuFetchImageResult();
        }

        if (uri.Scheme != Uri.UriSchemeHttp && uri.Scheme != Uri.UriSchemeHttps)
        {
            return new DanmakuFetchImageResult();
        }

        if (IsLocalOrPrivateHost(uri))
        {
            return new DanmakuFetchImageResult();
        }

        using var req = new HttpRequestMessage(HttpMethod.Get, uri);
        ApplyImageHeaders(req, uri);

        const int maxBytes = 768 * 1024; // just for emotes
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
                DisconnectUnsafeAsync().GetAwaiter().GetResult();
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
            try
            {
                _http.Dispose();
            }
            catch
            {
                // ignore
            }
        }

        _ffiGate.Dispose();
        DanmakuMessageReceived = null;
    }

    private async Task DisconnectUnsafeAsync()
    {
        var handle = _danmakuHandle;
        _danmakuHandle = IntPtr.Zero;
        _danmakuCallback = null;
        _activeSessionId = null;
        _activeSite = null;
        _activeRoomId = null;

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

            var user = (ev.User ?? "").Trim();
            var receivedAt = ev.ReceivedAtMs;

            var dms = ev.Dms;
            if (dms is { Count: > 0 })
            {
                foreach (var dm in dms)
                {
                    var imageUrl = string.IsNullOrWhiteSpace(dm?.ImageUrl) ? null : dm!.ImageUrl!.Trim();
                    var text = (dm?.Text ?? "").Trim();
                    if (string.IsNullOrWhiteSpace(text) && string.IsNullOrWhiteSpace(imageUrl))
                    {
                        continue;
                    }

                    DanmakuMessageReceived?.Invoke(this, new DanmakuMessage
                    {
                        SessionId = sid!,
                        ReceivedAtMs = receivedAt,
                        User = user,
                        Text = string.IsNullOrWhiteSpace(text) ? "[图片]" : text,
                        ImageUrl = imageUrl,
                        ImageWidth = dm?.ImageWidth,
                    });
                }
                return;
            }

            // Fallback when FFI doesn't provide `dms`.
            var text1 = (ev.Text ?? "").Trim();
            if (string.IsNullOrWhiteSpace(text1))
            {
                return;
            }

            DanmakuMessageReceived?.Invoke(this, new DanmakuMessage
            {
                SessionId = sid!,
                ReceivedAtMs = receivedAt,
                User = user,
                Text = text1,
                ImageUrl = null,
                ImageWidth = null,
            });
        }
        catch
        {
            // ignore callback failures
        }
    }

    private void ApplyImageHeaders(HttpRequestMessage req, Uri u)
    {
        try
        {
            var host = (u.Host ?? "").Trim().ToLowerInvariant();
            var site = (_activeSite ?? "").Trim().ToLowerInvariant();
            var roomId = (_activeRoomId ?? "").Trim();

            string? referer = null;
            if (site.Contains("bili") || host.Contains("bilibili.com") || host.Contains("hdslb.com"))
            {
                referer = string.IsNullOrWhiteSpace(roomId)
                    ? "https://live.bilibili.com/"
                    : $"https://live.bilibili.com/{roomId}/";
            }

            if (!string.IsNullOrWhiteSpace(referer))
            {
                try
                {
                    req.Headers.Remove("Referer");
                }
                catch
                {
                    // ignore
                }
                if (Uri.TryCreate(referer.Trim(), UriKind.Absolute, out var uri))
                {
                    req.Headers.Referrer = uri;
                }
                req.Headers.TryAddWithoutValidation("Referer", referer.Trim());
            }

            const string ua = "chaos-seed/winui3";
            try
            {
                req.Headers.Remove("User-Agent");
            }
            catch
            {
                // ignore
            }
            req.Headers.TryAddWithoutValidation("User-Agent", ua);
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

        // 127.0.0.0/8
        if (b[0] == 127)
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

    private static async Task<byte[]> ReadUpToAsync(Stream s, int maxBytes, CancellationToken ct)
    {
        var buf = new byte[8192];
        using var ms = new MemoryStream();

        while (ms.Length < maxBytes)
        {
            var toRead = (int)Math.Min(buf.Length, maxBytes - ms.Length);
            var n = await s.ReadAsync(buf.AsMemory(0, toRead), ct);
            if (n <= 0)
            {
                break;
            }
            ms.Write(buf, 0, n);
        }

        return ms.ToArray();
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

            return fallback;
        }
        catch
        {
            return fallback;
        }
    }
}

