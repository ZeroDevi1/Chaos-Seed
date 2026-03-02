using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.TtsBackends;

public static class TtsBackendFactory
{
    private const uint MinTtsApi = 11;

    public static ITtsBackend Create()
    {
        var mode = SettingsService.Instance.Current.TtsBackendMode;
        return mode switch
        {
            LiveBackendMode.Daemon => new DaemonTtsBackend(DaemonClient.Instance),
            LiveBackendMode.Ffi => CreateFfiOrError(),
            _ => CreateAuto(),
        };
    }

    private static ITtsBackend CreateAuto()
    {
        return new AutoTtsBackend(
            new DaemonTtsBackend(DaemonClient.Instance),
            createFfi: () =>
            {
                ProbeFfiOrThrow();
                return new FfiTtsBackend();
            }
        );
    }

    private static ITtsBackend CreateFfiOrError()
    {
        try
        {
            ProbeFfiOrThrow();
            return new FfiTtsBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI 初始化失败：{ex.GetType().Name}: {ex.Message}\n" +
                      "请确认 `chaos_ffi.dll` 已放在 WinUI 可执行文件同目录，或先在 Windows 侧运行：cargo xtask build-winui3 --release。";
            return new ErrorTtsBackend("FFI", msg);
        }
    }

    private static void ProbeFfiOrThrow()
    {
        // Calling into P/Invoke is the most reliable way to validate dll loadability.
        var api = ChaosFfi.chaos_ffi_api_version();
        if (api < MinTtsApi)
        {
            throw new InvalidOperationException($"FFI API_VERSION too old: {api} (need >= {MinTtsApi})");
        }
    }
}

