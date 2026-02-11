using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class PlaybackHints
{
    [JsonProperty("referer")]
    public string? Referer { get; set; }

    [JsonProperty("userAgent")]
    public string? UserAgent { get; set; }
}
