using System.Diagnostics;
using System.IO.Pipes;
using ChaosSeed.WinUI3.Models;
using StreamJsonRpc;
using StreamJsonRpc.Protocol;

namespace ChaosSeed.WinUI3.Services;

public sealed class DaemonClient : IDisposable
{
    public static DaemonClient Instance { get; } = new();

    private readonly SemaphoreSlim _connectLock = new(1, 1);
    private readonly RpcNotifications _notifications;

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

    public async Task EnsureConnectedAsync()
    {
        await _connectLock.WaitAsync();
        try
        {
            if (_rpc is not null)
            {
                return;
            }

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
                    CreateNoWindow = true
                }
            };
            _proc.Start();

            _pipe = new NamedPipeClientStream(".", _pipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
            await _pipe.ConnectAsync(cts.Token);

            var formatter = new JsonMessageFormatter();
            var handler = new HeaderDelimitedMessageHandler(_pipe, _pipe, formatter);
            _rpc = new JsonRpc(handler, _notifications);
            _rpc.StartListening();

            await _rpc.InvokeWithParameterObjectAsync<DaemonPingResult>("daemon.ping", new { authToken = _authToken });
        }
        finally
        {
            _connectLock.Release();
        }
    }

    public async Task<LiveOpenResult> OpenLiveAsync(string input)
    {
        await EnsureConnectedAsync();
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        return await _rpc.InvokeWithParameterObjectAsync<LiveOpenResult>(
            "live.open",
            new { input, preferredQuality = "highest" }
        );
    }

    public async Task CloseLiveAsync(string sessionId)
    {
        await EnsureConnectedAsync();
        if (_rpc is null)
        {
            return;
        }

        await _rpc.InvokeWithParameterObjectAsync<object>("live.close", new { sessionId });
    }

    public async Task<DanmakuFetchImageResult> FetchDanmakuImageAsync(string sessionId, string url)
    {
        await EnsureConnectedAsync();
        if (_rpc is null)
        {
            throw new InvalidOperationException("rpc not connected");
        }

        return await _rpc.InvokeWithParameterObjectAsync<DanmakuFetchImageResult>(
            "danmaku.fetchImage",
            new { sessionId, url }
        );
    }

    private void OnDanmakuMessage(DanmakuMessage msg)
    {
        DanmakuMessageReceived?.Invoke(this, msg);
    }

    public void Dispose()
    {
        try
        {
            _rpc?.Dispose();
            _pipe?.Dispose();
        }
        finally
        {
            _rpc = null;
            _pipe = null;
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

