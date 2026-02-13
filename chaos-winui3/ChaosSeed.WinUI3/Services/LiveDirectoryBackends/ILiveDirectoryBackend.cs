using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveDirectoryBackends;

public interface ILiveDirectoryBackend : IDisposable
{
    string Name { get; }
    string? InitNotice { get; }

    Task<LiveDirCategory[]> GetCategoriesAsync(string site, CancellationToken ct);

    Task<LiveDirRoomListResult> GetRecommendRoomsAsync(string site, int page, CancellationToken ct);

    Task<LiveDirRoomListResult> GetCategoryRoomsAsync(
        string site,
        string? parentId,
        string categoryId,
        int page,
        CancellationToken ct
    );

    Task<LiveDirRoomListResult> SearchRoomsAsync(string site, string keyword, int page, CancellationToken ct);
}

