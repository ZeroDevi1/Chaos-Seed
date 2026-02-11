using System.Diagnostics;
using System.Text;
using System.IO.Pipes;
using ChaosSeed.WinUI3.Models;
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

    public async Task CloseLiveAsync(string sessionId, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct);
        if (_rpc is null)
        {
            return;
        }

        await _rpc.InvokeWithParameterObjectAsync<object>("live.close", new { sessionId }, ct);
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
