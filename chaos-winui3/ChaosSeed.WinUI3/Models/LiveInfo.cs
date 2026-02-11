using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LiveInfo
{
    [JsonProperty("title")]
    public string Title { get; set; } = "";

    [JsonProperty("name")]
    public string? Name { get; set; }

    [JsonProperty("avatar")]
    public string? Avatar { get; set; }

    [JsonProperty("cover")]
    public string? Cover { get; set; }

    [JsonProperty("isLiving")]
    public bool IsLiving { get; set; }
}
