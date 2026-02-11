using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LiveOpenResult
{
    [JsonProperty("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("site")]
    public string Site { get; set; } = "";

    [JsonProperty("roomId")]
    public string RoomId { get; set; } = "";

    [JsonProperty("title")]
    public string Title { get; set; } = "";

    [JsonProperty("variantId")]
    public string VariantId { get; set; } = "";

    [JsonProperty("variantLabel")]
    public string VariantLabel { get; set; } = "";

    [JsonProperty("url")]
    public string Url { get; set; } = "";

    [JsonProperty("backupUrls")]
    public string[] BackupUrls { get; set; } = Array.Empty<string>();

    [JsonProperty("referer")]
    public string? Referer { get; set; }

    [JsonProperty("userAgent")]
    public string? UserAgent { get; set; }
}
