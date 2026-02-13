using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveDirectoryBackends;

public sealed class ErrorLiveDirectoryBackend : ILiveDirectoryBackend
{
    private readonly string _name;
    private readonly string _message;

    public ErrorLiveDirectoryBackend(string name, string message)
    {
        _name = name;
        _message = message;
    }

    public string Name => _name;
    public string? InitNotice => _message;

    public Task<LiveDirCategory[]> GetCategoriesAsync(string site, CancellationToken ct)
        => Task.FromException<LiveDirCategory[]>(new InvalidOperationException(_message));

    public Task<LiveDirRoomListResult> GetRecommendRoomsAsync(string site, int page, CancellationToken ct)
        => Task.FromException<LiveDirRoomListResult>(new InvalidOperationException(_message));

    public Task<LiveDirRoomListResult> GetCategoryRoomsAsync(string site, string? parentId, string categoryId, int page, CancellationToken ct)
        => Task.FromException<LiveDirRoomListResult>(new InvalidOperationException(_message));

    public Task<LiveDirRoomListResult> SearchRoomsAsync(string site, string keyword, int page, CancellationToken ct)
        => Task.FromException<LiveDirRoomListResult>(new InvalidOperationException(_message));

    public void Dispose()
    {
        // nothing
    }
}

