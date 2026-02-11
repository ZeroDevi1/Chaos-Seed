using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveBackends;

public static class LiveBackendFactory
{
    public static ILiveBackend Create()
    {
        var mode = SettingsService.Instance.Current.LiveBackendMode;
        return mode switch
        {
            LiveBackendMode.Daemon => new DaemonLiveBackend(),
            LiveBackendMode.Ffi => CreateFfiOrError(),
            _ => CreateAuto(),
        };
    }

    private static ILiveBackend CreateAuto()
    {
        try
        {
            ProbeFfi();
            return new FfiLiveBackend();
        }
        catch (Exception ex)
        {
            var msg = $"Auto：FFI 不可用，已回退到 daemon。\n原因：{ex.GetType().Name}: {ex.Message}";
            return new DaemonLiveBackend(msg);
        }
    }

    private static ILiveBackend CreateFfiOrError()
    {
        try
        {
            ProbeFfi();
            return new FfiLiveBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI 初始化失败：{ex.GetType().Name}: {ex.Message}\n" +
                      "请确认 `chaos_ffi.dll` 已放在 WinUI 可执行文件同目录，或先在 Windows 侧运行：cargo xtask build-winui3 --release。";
            return new ErrorLiveBackend("FFI", msg);
        }
    }

    private static void ProbeFfi()
    {
        // Calling into P/Invoke is the most reliable way to validate dll loadability.
        _ = ChaosFfi.chaos_ffi_api_version();
    }
}

