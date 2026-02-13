using System.Text.Json;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.LiveDirectoryBackends;

public sealed class FfiLiveDirectoryBackend : ILiveDirectoryBackend
{
    private static readonly JsonSerializerOptions _jsonOptions = new(JsonSerializerDefaults.Web)
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly SemaphoreSlim _ffiGate = new(1, 1);

    public string Name => "FFI";
    public string? InitNotice => null;

    public async Task<LiveDirCategory[]> GetCategoriesAsync(string site, CancellationToken ct)
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_live_dir_categories_json(site.Trim());
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "liveDir.categories failed"));
                }
                return s!;
            }, ct);

            return JsonSerializer.Deserialize<LiveDirCategory[]>(json, _jsonOptions) ?? Array.Empty<LiveDirCategory>();
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<LiveDirRoomListResult> GetRecommendRoomsAsync(string site, int page, CancellationToken ct)
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_live_dir_recommend_rooms_json(site.Trim(), (uint)Math.Max(1, page));
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "liveDir.recommendRooms failed"));
                }
                return s!;
            }, ct);

            return JsonSerializer.Deserialize<LiveDirRoomListResult>(json, _jsonOptions) ?? new LiveDirRoomListResult();
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<LiveDirRoomListResult> GetCategoryRoomsAsync(string site, string? parentId, string categoryId, int page, CancellationToken ct)
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_live_dir_category_rooms_json(
                    site.Trim(),
                    string.IsNullOrWhiteSpace(parentId) ? null : parentId.Trim(),
                    categoryId.Trim(),
                    (uint)Math.Max(1, page)
                );
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "liveDir.categoryRooms failed"));
                }
                return s!;
            }, ct);

            return JsonSerializer.Deserialize<LiveDirRoomListResult>(json, _jsonOptions) ?? new LiveDirRoomListResult();
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<LiveDirRoomListResult> SearchRoomsAsync(string site, string keyword, int page, CancellationToken ct)
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_live_dir_search_rooms_json(site.Trim(), keyword.Trim(), (uint)Math.Max(1, page));
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "liveDir.searchRooms failed"));
                }
                return s!;
            }, ct);

            return JsonSerializer.Deserialize<LiveDirRoomListResult>(json, _jsonOptions) ?? new LiveDirRoomListResult();
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public void Dispose()
    {
        _ffiGate.Dispose();
    }

    private static string FormatFfiError(string? errJson, string fallback)
    {
        if (string.IsNullOrWhiteSpace(errJson))
        {
            return fallback;
        }

        try
        {
            using var doc = JsonDocument.Parse(errJson);
            var root = doc.RootElement;
            var message = root.TryGetProperty("message", out var m) ? (m.GetString() ?? "") : "";
            var context = root.TryGetProperty("context", out var c) ? (c.GetString() ?? "") : "";

            message = message.Trim();
            context = context.Trim();

            if (!string.IsNullOrWhiteSpace(message) && !string.IsNullOrWhiteSpace(context))
            {
                return $"{message}\n{context}";
            }

            if (!string.IsNullOrWhiteSpace(message))
            {
                return message;
            }

            if (!string.IsNullOrWhiteSpace(context))
            {
                return context;
            }
        }
        catch
        {
            // ignore
        }

        return fallback;
    }
}

