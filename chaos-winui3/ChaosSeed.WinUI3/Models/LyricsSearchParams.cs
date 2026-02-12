using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LyricsSearchParams
{
    [JsonProperty("title")]
    public string Title { get; set; } = "";

    [JsonProperty("album")]
    public string? Album { get; set; }

    [JsonProperty("artist")]
    public string? Artist { get; set; }

    [JsonProperty("durationMs")]
    public ulong? DurationMs { get; set; }

    [JsonProperty("limit")]
    public uint? Limit { get; set; }

    [JsonProperty("strictMatch")]
    public bool? StrictMatch { get; set; }

    [JsonProperty("services")]
    public string[]? Services { get; set; }

    [JsonProperty("timeoutMs")]
    public ulong? TimeoutMs { get; set; }
}

