using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class DanmakuConnectResult
{
    [JsonProperty("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("site")]
    public string Site { get; set; } = "";

    [JsonProperty("roomId")]
    public string RoomId { get; set; } = "";
}

