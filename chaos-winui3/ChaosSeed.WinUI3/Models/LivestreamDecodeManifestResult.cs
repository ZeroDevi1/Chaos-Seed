using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class LivestreamDecodeManifestResult
{
    [JsonProperty("site")]
    public string Site { get; set; } = "";

    [JsonProperty("roomId")]
    public string RoomId { get; set; } = "";

    [JsonProperty("rawInput")]
    public string RawInput { get; set; } = "";

    [JsonProperty("info")]
    public LiveInfo Info { get; set; } = new();

    [JsonProperty("playback")]
    public PlaybackHints Playback { get; set; } = new();

    [JsonProperty("variants")]
    public StreamVariant[] Variants { get; set; } = Array.Empty<StreamVariant>();
}
