using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LiveDirRoomListResult
{
    [JsonProperty("hasMore")]
    public bool HasMore { get; set; }

    [JsonProperty("items")]
    public LiveDirRoomCard[] Items { get; set; } = Array.Empty<LiveDirRoomCard>();
}

