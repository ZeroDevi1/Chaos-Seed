using System.Diagnostics;
using System.Text.Json;

namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// Daemon 后端实现 (简化版，暂不支持实际功能)
/// </summary>
public class DaemonCliBackend : ICliBackend
{
    private Process? _daemonProcess;
    private IDanmakuSession? _danmakuSession;

    public Task InitializeAsync(CancellationToken ct = default)
    {
        // 检查 daemon 是否存在
        var daemonPath = Path.Combine(AppContext.BaseDirectory, "chaos-daemon.exe");
        if (!File.Exists(daemonPath))
        {
            throw new FileNotFoundException("未找到 chaos-daemon.exe，将使用 FFI 模式", daemonPath);
        }

        // 由于 CLI 模式下 FFI 更方便，Daemon 模式仅做架构占位
        // 实际使用时，推荐直接使用 FFI 模式
        throw new NotSupportedException(
            "Daemon 模式在 CLI 中暂未完全实现。\n" +
            "请使用: --backend ffi\n" +
            "或设置环境变量: CHAOS_CLI_BACKEND=ffi"
        );
    }

    public Task<LiveManifest?> DecodeManifestAsync(string input, bool dropInaccessibleHighQualities = true, CancellationToken ct = default)
    {
        throw new NotImplementedException();
    }

    public Task<StreamVariant?> ResolveVariantAsync(string site, string roomId, string variantId, CancellationToken ct = default)
    {
        throw new NotImplementedException();
    }

    public Task<IDanmakuSession?> ConnectDanmakuAsync(string input, CancellationToken ct = default)
    {
        throw new NotImplementedException();
    }

    public async ValueTask DisposeAsync()
    {
        if (_danmakuSession != null)
        {
            await _danmakuSession.DisposeAsync();
        }

        if (_daemonProcess != null && !_daemonProcess.HasExited)
        {
            try
            {
                _daemonProcess.Kill();
                await _daemonProcess.WaitForExitAsync();
            }
            catch { }
            _daemonProcess.Dispose();
        }
    }
}
