using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.DanmakuBackends;

public static class DanmakuBackendFactory
{
    public static IDanmakuBackend Create()
    {
        var mode = SettingsService.Instance.Current.DanmakuBackendMode;
        return mode switch
        {
            LiveBackendMode.Daemon => new DaemonDanmakuBackend(),
            LiveBackendMode.Ffi => CreateFfiOrError(),
            _ => CreateAuto(),
        };
    }

    private static IDanmakuBackend CreateAuto()
    {
        try
        {
            ProbeFfi();
            return new FfiDanmakuBackend();
        }
        catch (Exception ex)
        {
            var msg = $"Auto：FFI 不可用，已回退到 daemon。\n原因：{ex.GetType().Name}: {ex.Message}";
            return new DaemonDanmakuBackend(msg);
        }
    }

    private static IDanmakuBackend CreateFfiOrError()
    {
        try
        {
            ProbeFfi();
            return new FfiDanmakuBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI 初始化失败：{ex.GetType().Name}: {ex.Message}\n"
                + "请确认 `chaos_ffi.dll` 已放在 WinUI 可执行文件同目录，或先在 Windows 侧运行：cargo xtask build-winui3 --release。";
            return new ErrorDanmakuBackend("FFI", msg);
        }
    }

    private static void ProbeFfi()
    {
        _ = ChaosFfi.chaos_ffi_api_version();
    }
}

