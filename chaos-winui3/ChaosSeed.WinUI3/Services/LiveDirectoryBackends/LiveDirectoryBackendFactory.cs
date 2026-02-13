namespace ChaosSeed.WinUI3.Services.LiveDirectoryBackends;

public static class LiveDirectoryBackendFactory
{
    public static ILiveDirectoryBackend Create()
    {
        var mode = SettingsService.Instance.Current.LiveBackendMode;
        return mode switch
        {
            Models.LiveBackendMode.Daemon => new DaemonLiveDirectoryBackend(),
            Models.LiveBackendMode.Ffi => CreateFfiOrError(),
            _ => CreateAuto(),
        };
    }

    private static ILiveDirectoryBackend CreateAuto()
    {
        try
        {
            return new FfiLiveDirectoryBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI backend unavailable: {ex.Message}";
            return new DaemonLiveDirectoryBackend(msg);
        }
    }

    private static ILiveDirectoryBackend CreateFfiOrError()
    {
        try
        {
            return new FfiLiveDirectoryBackend();
        }
        catch (Exception ex)
        {
            var msg = $"FFI backend unavailable: {ex.Message}";
            return new ErrorLiveDirectoryBackend("FFI", msg);
        }
    }
}

