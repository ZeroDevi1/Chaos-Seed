using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Models.Ffi;

public sealed class FfiDanmakuEvent
{
    [JsonPropertyName("room_id")]
    public string? RoomId { get; set; }

    [JsonPropertyName("received_at_ms")]
    public long ReceivedAtMs { get; set; }

    [JsonPropertyName("method")]
    public string? Method { get; set; }

    [JsonPropertyName("user")]
    public string? User { get; set; }

    [JsonPropertyName("text")]
    public string? Text { get; set; }

    [JsonPropertyName("dms")]
    public List<FfiDanmakuComment>? Dms { get; set; }
}

public sealed class FfiDanmakuComment
{
    [JsonPropertyName("text")]
    public string? Text { get; set; }

    [JsonPropertyName("image_url")]
    public string? ImageUrl { get; set; }

    [JsonPropertyName("image_width")]
    public uint? ImageWidth { get; set; }
}

