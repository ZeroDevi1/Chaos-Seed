using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LiveDirCategory
{
    [JsonProperty("id")]
    public string Id { get; set; } = "";

    [JsonProperty("name")]
    public string Name { get; set; } = "";

    [JsonProperty("children")]
    public LiveDirSubCategory[] Children { get; set; } = Array.Empty<LiveDirSubCategory>();
}

