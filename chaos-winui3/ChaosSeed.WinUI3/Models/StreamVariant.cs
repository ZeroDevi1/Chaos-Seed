using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class StreamVariant
{
    [JsonProperty("id")]
    public string Id { get; set; } = "";

    [JsonProperty("label")]
    public string Label { get; set; } = "";

    [JsonProperty("quality")]
    public int Quality { get; set; }

    [JsonProperty("rate")]
    public int? Rate { get; set; }

    [JsonProperty("url")]
    public string? Url { get; set; }

    [JsonProperty("backupUrls")]
    public string[] BackupUrls { get; set; } = Array.Empty<string>();
}
