using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveDirectoryBackends;

public sealed class DaemonLiveDirectoryBackend : ILiveDirectoryBackend
{
    private readonly string? _initNotice;

    public DaemonLiveDirectoryBackend(string? initNotice = null)
    {
        _initNotice = initNotice;
    }

    public string Name => "Daemon";
    public string? InitNotice => _initNotice;

    public Task<LiveDirCategory[]> GetCategoriesAsync(string site, CancellationToken ct)
        => DaemonClient.Instance.LiveDirCategoriesAsync(site, ct);

    public Task<LiveDirRoomListResult> GetRecommendRoomsAsync(string site, int page, CancellationToken ct)
        => DaemonClient.Instance.LiveDirRecommendRoomsAsync(site, page, ct);

    public Task<LiveDirRoomListResult> GetCategoryRoomsAsync(string site, string? parentId, string categoryId, int page, CancellationToken ct)
        => DaemonClient.Instance.LiveDirCategoryRoomsAsync(site, parentId, categoryId, page, ct);

    public Task<LiveDirRoomListResult> SearchRoomsAsync(string site, string keyword, int page, CancellationToken ct)
        => DaemonClient.Instance.LiveDirSearchRoomsAsync(site, keyword, page, ct);

    public void Dispose()
    {
        // DaemonClient is shared singleton.
    }
}

