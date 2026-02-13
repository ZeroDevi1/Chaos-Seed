using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LiveDirRoomCard
{
    [JsonProperty("site")]
    public string Site { get; set; } = "";

    [JsonProperty("roomId")]
    public string RoomId { get; set; } = "";

    [JsonProperty("input")]
    public string Input { get; set; } = "";

    [JsonProperty("title")]
    public string Title { get; set; } = "";

    [JsonProperty("cover")]
    public string? Cover { get; set; }

    [JsonProperty("userName")]
    public string? UserName { get; set; }

    [JsonProperty("online")]
    public long? Online { get; set; }
}

