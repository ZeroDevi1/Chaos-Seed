using System.Text.Json;
using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// 后端接口抽象（支持 FFI 和 Daemon 两种模式）
/// </summary>
public interface ICliBackend : IAsyncDisposable
{
    /// <summary>
    /// 初始化后端
    /// </summary>
    Task InitializeAsync(CancellationToken ct = default);

    /// <summary>
    /// 解析直播源 Manifest
    /// </summary>
    Task<LiveManifest?> DecodeManifestAsync(string input, bool dropInaccessibleHighQualities = true, CancellationToken ct = default);

    /// <summary>
    /// 解析指定清晰度的播放地址
    /// </summary>
    Task<StreamVariant?> ResolveVariantAsync(string site, string roomId, string variantId, CancellationToken ct = default);

    /// <summary>
    /// 连接弹幕
    /// </summary>
    Task<IDanmakuSession?> ConnectDanmakuAsync(string input, CancellationToken ct = default);
}

/// <summary>
/// 直播源 Manifest
/// </summary>
public class LiveManifest
{
    [JsonPropertyName("site")]
    public string Site { get; set; } = "";

    [JsonPropertyName("room_id")]
    public string RoomId { get; set; } = "";

    [JsonPropertyName("raw_input")]
    public string RawInput { get; set; } = "";

    [JsonPropertyName("info")]
    public LiveInfo Info { get; set; } = new();

    [JsonPropertyName("playback")]
    public PlaybackHints Playback { get; set; } = new();

    [JsonPropertyName("variants")]
    public List<StreamVariant> Variants { get; set; } = new();
}

public class LiveInfo
{
    [JsonPropertyName("title")]
    public string Title { get; set; } = "";

    [JsonPropertyName("name")]
    public string? Name { get; set; }

    [JsonPropertyName("avatar")]
    public string? Avatar { get; set; }

    [JsonPropertyName("cover")]
    public string? Cover { get; set; }

    [JsonPropertyName("is_living")]
    public bool IsLiving { get; set; }
}

public class PlaybackHints
{
    [JsonPropertyName("referer")]
    public string? Referer { get; set; }

    [JsonPropertyName("user_agent")]
    public string? UserAgent { get; set; }
}

public class StreamVariant
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("label")]
    public string Label { get; set; } = "";

    [JsonPropertyName("quality")]
    public int Quality { get; set; }

    [JsonPropertyName("rate")]
    public int? Rate { get; set; }

    [JsonPropertyName("url")]
    public string? Url { get; set; }

    [JsonPropertyName("backup_urls")]
    public List<string> BackupUrls { get; set; } = new();
}

/// <summary>
/// 弹幕事件
/// </summary>
public class DanmakuEvent
{
    [JsonPropertyName("site")]
    public string Site { get; set; } = "";

    [JsonPropertyName("room_id")]
    public string RoomId { get; set; } = "";

    [JsonPropertyName("received_at_ms")]
    public long ReceivedAtMs { get; set; }

    [JsonPropertyName("method")]
    public string Method { get; set; } = "";

    [JsonPropertyName("user")]
    public string User { get; set; } = "";

    [JsonPropertyName("text")]
    public string Text { get; set; } = "";

    [JsonPropertyName("dms")]
    public List<DanmakuComment>? Dms { get; set; }
}

public class DanmakuComment
{
    [JsonPropertyName("text")]
    public string Text { get; set; } = "";

    [JsonPropertyName("image_url")]
    public string? ImageUrl { get; set; }

    [JsonPropertyName("image_width")]
    public int? ImageWidth { get; set; }
}

/// <summary>
/// 弹幕会话接口
/// </summary>
public interface IDanmakuSession : IAsyncDisposable
{
    /// <summary>
    /// 轮询弹幕事件
    /// </summary>
    Task<List<DanmakuEvent>> PollAsync(int maxEvents = 50, CancellationToken ct = default);
}

/// <summary>
/// 后端工厂
/// </summary>
public static class CliBackendFactory
{
    public static ICliBackend Create(string backend)
    {
        return backend.ToLowerInvariant() switch
        {
            "ffi" => new FfiCliBackend(),
            "daemon" => new DaemonCliBackend(),
            "auto" => TryCreateFfiBackend() ?? new DaemonCliBackend(),
            _ => throw new ArgumentException($"未知后端模式: {backend}")
        };
    }

    private static ICliBackend? TryCreateFfiBackend()
    {
        try
        {
            var dllPath = Path.Combine(AppContext.BaseDirectory, "chaos_ffi.dll");
            if (File.Exists(dllPath))
            {
                return new FfiCliBackend();
            }
        }
        catch { }
        return null;
    }
}
