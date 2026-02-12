using ChaosSeed.WinUI3.Chaos;

namespace ChaosSeed.WinUI3.Services.LyricsBackends;

public static class LyricsBackendFactory
{
    public static ILyricsBackend Create()
    {
        var mode = SettingsService.Instance.Current.LyricsBackendMode;
        return mode switch
        {
            Models.LiveBackendMode.Daemon => new DaemonLyricsBackend(),
            Models.LiveBackendMode.Ffi => CreateFfiOrError(),
            _ => CreateAuto(),
        };
    }

    private static ILyricsBackend CreateAuto()
    {
        try
        {
            ProbeFfi();
            return new FfiLyricsBackend();
        }
        catch (Exception ex)
        {
            var msg = $"Auto：FFI 不可用，已回退到 daemon。\n原因：{ex.GetType().Name}: {ex.Message}";
            return new DaemonLyricsBackend(msg);
        }
    }

    private static ILyricsBackend CreateFfiOrError()
    {
        try
        {
            ProbeFfi();
            return new FfiLyricsBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI 初始化失败：{ex.GetType().Name}: {ex.Message}\n" +
                      "请确认 `chaos_ffi.dll` 已放在 WinUI 可执行文件同目录，或先在 Windows 侧运行：cargo xtask build-winui3 --release。";
            return new ErrorLyricsBackend("FFI", msg);
        }
    }

    private static void ProbeFfi()
    {
        _ = ChaosFfi.chaos_ffi_api_version();
    }
}

