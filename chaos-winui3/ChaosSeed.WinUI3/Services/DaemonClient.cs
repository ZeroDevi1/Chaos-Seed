using System.Diagnostics;
using System.Text;
using System.IO.Pipes;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Models.Music;
using StreamJsonRpc;
using StreamJsonRpc.Protocol;
using StreamJsonRpc.Reflection;

namespace ChaosSeed.WinUI3.Services;

public sealed class DaemonClient : IDisposable
{
    public static DaemonClient Instance { get; } = new();

    private readonly SemaphoreSlim _connectLock = new(1, 1);
    private readonly RpcNotifications _notifications;
    private readonly object _logGate = new();
    private StreamWriter? _daemonLog;
    private string? _daemonLogPath;
    private readonly Queue<string> _daemonLogTail = new();
    private const int DaemonLogTailMaxLines = 200;

    private Process? _proc;
    private NamedPipeClientStream? _pipe;
    private JsonRpc? _rpc;
    private string? _authToken;
    private string? _pipeName;

    public event EventHandler<DanmakuMessage>? DanmakuMessageReceived;

    private DaemonClient()
    {
        _notifications = new RpcNotifications(this);
    }

    public string? DaemonLogPath => _daemonLogPath;

    public async Task EnsureConnectedAsync(CancellationToken ct = default)
    {
        await _connectLock.WaitAsync(ct);
        try
        {
            if (_rpc is not null && IsConnectionHealthy())
            {
                return;
            }

            ResetConnection();
            StartNewDaemonLog();

            _authToken = Guid.NewGuid().ToString("N");
            _pipeName = $"chaos-seed-{Guid.NewGuid():N}";

            var daemonExe = Path.Combine(AppContext.BaseDirectory, "chaos-daemon.exe");
            if (!File.Exists(daemonExe))
            {
                throw new FileNotFoundException("Missing chaos-daemon.exe next to WinUI executable.", daemonExe);
            }

            _proc = new Process
            {
                StartInfo = new ProcessStartInfo
                {
                    FileName = daemonExe,
                    Arguments = $"--pipe-name {_pipeName} --auth-token {_authToken}",
                    UseShellExecute = false,
                    CreateNoWindow = true,
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    StandardOutputEncoding = Encoding.UTF8,
                    StandardErrorEncoding = Encoding.UTF8
                }
            };
            var proc = _proc;
            proc.EnableRaisingEvents = true;
            proc.OutputDataReceived += (_, e) =>
            {
                if (!string.IsNullOrWhiteSpace(e.Data))
                {
                    DaemonLog("stdout", e.Data!);
                }
            };
            proc.ErrorDataReceived += (_, e) =>
            {
                if (!string.IsNullOrWhiteSpace(e.Data))
                {
                    DaemonLog("stderr", e.Data!);
                }
            };
            proc.Exited += (_, _) =>
            {
                try
                {
                    DaemonLog("proc", $"exited with code={proc.ExitCode}");
                }
                catch
                {
                    // ignore
                }
            };
            proc.Start();
            proc.BeginOutputReadLine();
            proc.BeginErrorReadLine();

            _pipe = new NamedPipeClientStream(".", _pipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
            using var linked = CancellationTokenSource.CreateLinkedTokenSource(ct, cts.Token);
            await _pipe.ConnectAsync(linked.Token);

            var formatter = new JsonMessageFormatter();
            var handler = new HeaderDelimitedMessageHandler(_pipe, _pipe, formatter);
            _rpc = new JsonRpc(handler);
            _rpc.AddLocalRpcTarget(
                _notifications,
                new JsonRpcTargetOptions
                {
                    // daemon sends notification payload as a single object (params = { sessionId, ... })
                    // instead of wrapping it with the argument name (params = { msg: { ... } }).
                    // Enable single-object deserialization so DanmakuMessage(DanmakuMessage msg) can bind.
                    UseSingleObjectParameterDeserialization = true,
                }
            );
            _rpc.StartListening();

            await _rpc.InvokeWithParameterObjectAsync<DaemonPingResult>(
                "daemon.ping",
                new { authToken = _authToken },
                ct
            );
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            var hint = _daemonLogPath is null ? "" : $" (see daemon log: {_daemonLogPath})";
            throw new Exception($"daemon connect failed: {ex.Message}{hint}", ex);
        }
        finally
        {
            _connectLock.Release();
        }
    }

    public async Task<LiveOpenResult> OpenLiveAsync(string input, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<LiveOpenResult>(
                "live.open",
                new { input, preferredQuality = "highest" },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("live.open", ex);
        }
    }

    public async Task<LiveOpenResult> OpenLiveAsync(string input, string? variantId, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        object payload = string.IsNullOrWhiteSpace(variantId)
            ? new { input, preferredQuality = "highest" }
            : new { input, preferredQuality = "highest", variantId = variantId.Trim() };

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<LiveOpenResult>(
                "live.open",
                payload,
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("live.open", ex);
        }
    }

    public async Task<LivestreamDecodeManifestResult> DecodeManifestAsync(string input, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        return await _rpc.InvokeWithParameterObjectAsync<LivestreamDecodeManifestResult>(
            "livestream.decodeManifest",
            new { input },
            ct
        );
    }

    public async Task<LiveDirCategory[]> LiveDirCategoriesAsync(string site, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        return await _rpc.InvokeWithParameterObjectAsync<LiveDirCategory[]>(
            "liveDir.categories",
            new { site = (site ?? "").Trim() },
            ct
        );
    }

    public async Task<LiveDirRoomListResult> LiveDirRecommendRoomsAsync(string site, int page, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        return await _rpc.InvokeWithParameterObjectAsync<LiveDirRoomListResult>(
            "liveDir.recommendRooms",
            new { site = (site ?? "").Trim(), page = Math.Max(1, page) },
            ct
        );
    }

    public async Task<LiveDirRoomListResult> LiveDirCategoryRoomsAsync(
        string site,
        string? parentId,
        string categoryId,
        int page,
        CancellationToken ct = default
    )
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        object payload = new
        {
            site = (site ?? "").Trim(),
            parentId = string.IsNullOrWhiteSpace(parentId) ? null : parentId.Trim(),
            categoryId = (categoryId ?? "").Trim(),
            page = Math.Max(1, page),
        };

        return await _rpc.InvokeWithParameterObjectAsync<LiveDirRoomListResult>(
            "liveDir.categoryRooms",
            payload,
            ct
        );
    }

    public async Task<LiveDirRoomListResult> LiveDirSearchRoomsAsync(string site, string keyword, int page, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        return await _rpc.InvokeWithParameterObjectAsync<LiveDirRoomListResult>(
            "liveDir.searchRooms",
            new { site = (site ?? "").Trim(), keyword = (keyword ?? "").Trim(), page = Math.Max(1, page) },
            ct
        );
    }

    public async Task<NowPlayingSnapshot> NowPlayingSnapshotAsync(
        bool includeThumbnail,
        int maxThumbBytes,
        int maxSessions,
        CancellationToken ct = default
    )
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<NowPlayingSnapshot>(
                "nowPlaying.snapshot",
                new
                {
                    includeThumbnail,
                    maxThumbnailBytes = maxThumbBytes,
                    maxSessions,
                },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("nowPlaying.snapshot", ex);
        }
    }

    public async Task<LyricsSearchResult[]> LyricsSearchAsync(LyricsSearchParams p, CancellationToken ct = default)
    {
        if (p is null)
        {
            throw new ArgumentNullException(nameof(p));
        }

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        var title = (p.Title ?? "").Trim();
        if (string.IsNullOrWhiteSpace(title))
        {
            throw new ArgumentException("empty title", nameof(p));
        }

        object payload = new
        {
            title,
            album = string.IsNullOrWhiteSpace(p.Album) ? null : p.Album!.Trim(),
            artist = string.IsNullOrWhiteSpace(p.Artist) ? null : p.Artist!.Trim(),
            durationMs = p.DurationMs,
            limit = p.Limit,
            strictMatch = p.StrictMatch,
            services = p.Services,
            timeoutMs = p.TimeoutMs,
        };

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<LyricsSearchResult[]>(
                "lyrics.search",
                payload,
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("lyrics.search", ex);
        }
    }

    // ----- music -----

    public async Task MusicConfigSetAsync(MusicProviderConfig cfg, CancellationToken ct = default)
    {
        if (cfg is null) throw new ArgumentNullException(nameof(cfg));
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            _ = await _rpc.InvokeWithParameterObjectAsync<OkReply>("music.config.set", cfg, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.config.set", ex);
        }
    }

    public async Task<MusicTrack[]> MusicSearchTracksAsync(MusicSearchParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicTrack[]>("music.searchTracks", p, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.searchTracks", ex);
        }
    }

    public async Task<MusicAlbum[]> MusicSearchAlbumsAsync(MusicSearchParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicAlbum[]>("music.searchAlbums", p, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.searchAlbums", ex);
        }
    }

    public async Task<MusicArtist[]> MusicSearchArtistsAsync(MusicSearchParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicArtist[]>("music.searchArtists", p, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.searchArtists", ex);
        }
    }

    public async Task<MusicTrack[]> MusicAlbumTracksAsync(MusicAlbumTracksParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicTrack[]>("music.albumTracks", p, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.albumTracks", ex);
        }
    }

    public async Task<MusicAlbum[]> MusicArtistAlbumsAsync(MusicArtistAlbumsParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicAlbum[]>("music.artistAlbums", p, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.artistAlbums", ex);
        }
    }

    public async Task<MusicTrackPlayUrlResult> MusicTrackPlayUrlAsync(MusicTrackPlayUrlParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            // Use an anonymous payload with explicit camelCase keys to avoid serializer naming-policy differences
            // (daemon expects e.g. "trackId" rather than "TrackId").
            var payload = new
            {
                service = (p.Service ?? "").Trim(),
                trackId = (p.TrackId ?? "").Trim(),
                qualityId = string.IsNullOrWhiteSpace(p.QualityId) ? null : p.QualityId,
                auth = p.Auth,
            };
            return await _rpc.InvokeWithParameterObjectAsync<MusicTrackPlayUrlResult>("music.trackPlayUrl", payload, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.trackPlayUrl", ex);
        }
    }

    public async Task<MusicLoginQr> MusicQqLoginQrCreateAsync(string loginType, CancellationToken ct = default)
    {
        var lt = (loginType ?? "").Trim();
        if (string.IsNullOrWhiteSpace(lt))
        {
            throw new ArgumentException("empty loginType", nameof(loginType));
        }

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicLoginQr>(
                "music.qq.loginQrCreate",
                new { loginType = lt },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.qq.loginQrCreate", ex);
        }
    }

    public async Task<MusicLoginQrPollResult> MusicQqLoginQrPollAsync(string sessionId, CancellationToken ct = default)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            throw new ArgumentException("empty sessionId", nameof(sessionId));
        }

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicLoginQrPollResult>(
                "music.qq.loginQrPoll",
                new { sessionId = sid },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.qq.loginQrPoll", ex);
        }
    }

    public async Task<QqMusicCookie> MusicQqRefreshCookieAsync(QqMusicCookie cookie, CancellationToken ct = default)
    {
        if (cookie is null) throw new ArgumentNullException(nameof(cookie));

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<QqMusicCookie>(
                "music.qq.refreshCookie",
                new { cookie },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.qq.refreshCookie", ex);
        }
    }

    public async Task<MusicLoginQr> MusicKugouLoginQrCreateAsync(string loginType, CancellationToken ct = default)
    {
        var lt = (loginType ?? "").Trim();
        if (string.IsNullOrWhiteSpace(lt))
        {
            throw new ArgumentException("empty loginType", nameof(loginType));
        }

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicLoginQr>(
                "music.kugou.loginQrCreate",
                new { loginType = lt },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.kugou.loginQrCreate", ex);
        }
    }

    public async Task<MusicLoginQrPollResult> MusicKugouLoginQrPollAsync(string sessionId, CancellationToken ct = default)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            throw new ArgumentException("empty sessionId", nameof(sessionId));
        }

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicLoginQrPollResult>(
                "music.kugou.loginQrPoll",
                new { sessionId = sid },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.kugou.loginQrPoll", ex);
        }
    }

    public async Task<MusicDownloadStartResult> MusicDownloadStartAsync(MusicDownloadStartParams p, CancellationToken ct = default)
    {
        if (p is null) throw new ArgumentNullException(nameof(p));

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicDownloadStartResult>("music.download.start", p, ct);
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.download.start", ex);
        }
    }

    public async Task<MusicDownloadStatus> MusicDownloadStatusAsync(string sessionId, CancellationToken ct = default)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            throw new ArgumentException("empty sessionId", nameof(sessionId));
        }

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<MusicDownloadStatus>(
                "music.download.status",
                new { sessionId = sid },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.download.status", ex);
        }
    }

    public async Task MusicDownloadCancelAsync(string sessionId, CancellationToken ct = default)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            throw new ArgumentException("empty sessionId", nameof(sessionId));
        }

        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        try
        {
            _ = await _rpc.InvokeWithParameterObjectAsync<OkReply>(
                "music.download.cancel",
                new { sessionId = sid },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("music.download.cancel", ex);
        }
    }

    public async Task CloseLiveAsync(string sessionId, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            return;
        }

        await _rpc.InvokeWithParameterObjectAsync<object>("live.close", new { sessionId }, ct);
    }

    public async Task<DanmakuConnectResult> DanmakuConnectAsync(string input, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        var s = (input ?? "").Trim();
        if (string.IsNullOrWhiteSpace(s))
        {
            throw new ArgumentException("empty input", nameof(input));
        }

        try
        {
            return await _rpc.InvokeWithParameterObjectAsync<DanmakuConnectResult>(
                "danmaku.connect",
                new { input = s },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("danmaku.connect", ex);
        }
    }

    public async Task DanmakuDisconnectAsync(string sessionId, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            return;
        }

        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            return;
        }

        try
        {
            await _rpc.InvokeWithParameterObjectAsync<object>(
                "danmaku.disconnect",
                new { sessionId = sid },
                ct
            );
        }
        catch (RemoteInvocationException ex)
        {
            throw WrapRpcFailure("danmaku.disconnect", ex);
        }
    }

    public async Task<DanmakuFetchImageResult> FetchDanmakuImageAsync(
        string sessionId,
        string url,
        CancellationToken ct = default
    )
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        return await _rpc.InvokeWithParameterObjectAsync<DanmakuFetchImageResult>(
            "danmaku.fetchImage",
            new { sessionId, url },
            ct
        );
    }

    private void OnDanmakuMessage(DanmakuMessage msg)
    {
        DanmakuMessageReceived?.Invoke(this, msg);
    }

    public void Dispose()
    {
        ResetConnection();
    }

    private void ResetConnection()
    {
        lock (_logGate)
        {
            try
            {
                _daemonLog?.Dispose();
            }
            catch
            {
                // ignore
            }
            finally
            {
                _daemonLog = null;
                _daemonLogPath = null;
                _daemonLogTail.Clear();
            }
        }

        try
        {
            _rpc?.Dispose();
        }
        catch
        {
            // ignore
        }

        try
        {
            _pipe?.Dispose();
        }
        catch
        {
            // ignore
        }

        _rpc = null;
        _pipe = null;

        try
        {
            if (_proc is not null && !_proc.HasExited)
            {
                _proc.Kill(entireProcessTree: true);
            }
        }
        catch
        {
            // ignore
        }
        finally
        {
            _proc = null;
        }
    }

    private void StartNewDaemonLog()
    {
        try
        {
            var root = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "ChaosSeed.WinUI3",
                "logs"
            );
            Directory.CreateDirectory(root);
            var path = Path.Combine(root, $"chaos-daemon-{DateTime.Now:yyyyMMdd-HHmmss}.log");

            lock (_logGate)
            {
                _daemonLogPath = path;
                _daemonLog = new StreamWriter(new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.ReadWrite))
                {
                    AutoFlush = true
                };
            }

            DaemonLog("proc", "starting chaos-daemon.exe");
        }
        catch
        {
            // ignore logging failures
        }
    }

    private void DaemonLog(string tag, string msg)
    {
        var line = $"[{DateTime.Now:HH:mm:ss.fff}] [{tag}] {msg}";
        lock (_logGate)
        {
            try
            {
                _daemonLog?.WriteLine(line);
            }
            catch
            {
                // ignore
            }

            _daemonLogTail.Enqueue(line);
            while (_daemonLogTail.Count > DaemonLogTailMaxLines)
            {
                _daemonLogTail.Dequeue();
            }
        }
    }

    private Exception WrapRpcFailure(string method, RemoteInvocationException ex)
    {
        var msg = $"{method} failed: {ex.Message}";
        if (!string.IsNullOrWhiteSpace(_daemonLogPath))
        {
            msg += $"\n（daemon 日志：{_daemonLogPath}）";
        }
        return new Exception(msg, ex);
    }

    private bool IsConnectionHealthy()
    {
        try
        {
            if (_rpc is null || _pipe is null)
            {
                return false;
            }

            if (!_pipe.IsConnected)
            {
                return false;
            }

            if (_proc is not null && _proc.HasExited)
            {
                return false;
            }

            return true;
        }
        catch
        {
            return false;
        }
    }

    private sealed class RpcNotifications
    {
        private readonly DaemonClient _client;

        public RpcNotifications(DaemonClient client)
        {
            _client = client;
        }

        [JsonRpcMethod("danmaku.message")]
        public void DanmakuMessage(DanmakuMessage msg)
        {
            _client.OnDanmakuMessage(msg);
        }
    }
}
