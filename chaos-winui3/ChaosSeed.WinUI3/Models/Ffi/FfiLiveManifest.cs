using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Models.Ffi;

public sealed class FfiLiveManifest
{
    [JsonPropertyName("site")]
    public string Site { get; set; } = "";

    [JsonPropertyName("room_id")]
    public string RoomId { get; set; } = "";

    [JsonPropertyName("raw_input")]
    public string RawInput { get; set; } = "";

    [JsonPropertyName("info")]
    public FfiLiveInfo? Info { get; set; }

    [JsonPropertyName("playback")]
    public FfiPlaybackHints? Playback { get; set; }

    [JsonPropertyName("variants")]
    public List<FfiStreamVariant>? Variants { get; set; }
}

public sealed class FfiLiveInfo
{
    [JsonPropertyName("title")]
    public string? Title { get; set; }

    [JsonPropertyName("name")]
    public string? Name { get; set; }

    [JsonPropertyName("avatar")]
    public string? Avatar { get; set; }

    [JsonPropertyName("cover")]
    public string? Cover { get; set; }

    [JsonPropertyName("is_living")]
    public bool IsLiving { get; set; }
}

public sealed class FfiPlaybackHints
{
    [JsonPropertyName("referer")]
    public string? Referer { get; set; }

    [JsonPropertyName("user_agent")]
    public string? UserAgent { get; set; }
}

public sealed class FfiStreamVariant
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("label")]
    public string? Label { get; set; }

    [JsonPropertyName("quality")]
    public int Quality { get; set; }

    [JsonPropertyName("rate")]
    public int? Rate { get; set; }

    [JsonPropertyName("url")]
    public string? Url { get; set; }

    [JsonPropertyName("backup_urls")]
    public List<string>? BackupUrls { get; set; }
}

