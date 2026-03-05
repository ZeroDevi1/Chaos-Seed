using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.VoiceChatBackends;

public static class VoiceChatBackendFactory
{
    private const uint MinVoiceChatApi = 11;

    public static IVoiceChatBackend Create()
    {
        var mode = SettingsService.Instance.Current.VoiceChatBackendMode;
        return mode switch
        {
            LiveBackendMode.Daemon => new DaemonVoiceChatBackend(DaemonClient.Instance),
            LiveBackendMode.Ffi => CreateFfiOrError(),
            _ => CreateAuto(),
        };
    }

    private static IVoiceChatBackend CreateAuto()
    {
        return new AutoVoiceChatBackend(
            new DaemonVoiceChatBackend(DaemonClient.Instance),
            createFfi: () =>
            {
                ProbeFfiOrThrow();
                return new FfiVoiceChatBackend();
            }
        );
    }

    private static IVoiceChatBackend CreateFfiOrError()
    {
        try
        {
            ProbeFfiOrThrow();
            return new FfiVoiceChatBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI 初始化失败：{ex.GetType().Name}: {ex.Message}\n"
                + "请确认 `chaos_ffi.dll` 已放在 WinUI 可执行文件同目录，或先在 Windows 侧运行：cargo xtask build-winui3 --release。";
            return new ErrorVoiceChatBackend("FFI", msg);
        }
    }

    private static void ProbeFfiOrThrow()
    {
        var api = ChaosFfi.chaos_ffi_api_version();
        if (api < MinVoiceChatApi)
        {
            throw new InvalidOperationException($"FFI API_VERSION too old: {api} (need >= {MinVoiceChatApi})");
        }
    }
}

