using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LiveDirSubCategory
{
    [JsonProperty("id")]
    public string Id { get; set; } = "";

    [JsonProperty("parentId")]
    public string ParentId { get; set; } = "";

    [JsonProperty("name")]
    public string Name { get; set; } = "";

    [JsonProperty("pic")]
    public string? Pic { get; set; }
}

