using ChaosSeed.WinUI3.Chaos;

namespace ChaosSeed.WinUI3.Services.BiliBackends;

public static class BiliBackendFactory
{
    public static IBiliBackend Create()
    {
        var mode = SettingsService.Instance.Current.BiliBackendMode;
        return mode switch
        {
            Models.LiveBackendMode.Daemon => new DaemonBiliBackend(),
            Models.LiveBackendMode.Ffi => CreateFfiOrError(),
            _ => CreateAuto(),
        };
    }

    private static IBiliBackend CreateAuto()
    {
        try
        {
            ProbeFfi();
            return new FfiBiliBackend();
        }
        catch (Exception ex)
        {
            var msg = $"Auto：FFI 不可用，已回退到 daemon。\n原因：{ex.GetType().Name}: {ex.Message}";
            return new DaemonBiliBackend(msg);
        }
    }

    private static IBiliBackend CreateFfiOrError()
    {
        try
        {
            ProbeFfi();
            return new FfiBiliBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI 初始化失败：{ex.GetType().Name}: {ex.Message}\n" +
                      "请确认 `chaos_ffi.dll` 已放在 WinUI 可执行文件同目录，或先在 Windows 侧运行：cargo xtask build-winui3 --release。";
            return new ErrorBiliBackend("FFI", msg);
        }
    }

    private static void ProbeFfi()
    {
        var api = ChaosFfi.chaos_ffi_api_version();
        if (api < 8)
        {
            throw new InvalidOperationException($"FFI API_VERSION too old: {api} (need >= 8)");
        }
    }
}

