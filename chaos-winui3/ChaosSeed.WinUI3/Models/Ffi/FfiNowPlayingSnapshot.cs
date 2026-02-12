using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Models.Ffi;

public sealed class FfiNowPlayingThumbnail
{
    [JsonPropertyName("mime")]
    public string Mime { get; set; } = "";

    [JsonPropertyName("base64")]
    public string Base64 { get; set; } = "";
}

public sealed class FfiNowPlayingSession
{
    [JsonPropertyName("app_id")]
    public string AppId { get; set; } = "";

    [JsonPropertyName("is_current")]
    public bool IsCurrent { get; set; }

    [JsonPropertyName("playback_status")]
    public string PlaybackStatus { get; set; } = "";

    [JsonPropertyName("title")]
    public string? Title { get; set; }

    [JsonPropertyName("artist")]
    public string? Artist { get; set; }

    [JsonPropertyName("album_title")]
    public string? AlbumTitle { get; set; }

    [JsonPropertyName("position_ms")]
    public ulong? PositionMs { get; set; }

    [JsonPropertyName("duration_ms")]
    public ulong? DurationMs { get; set; }

    [JsonPropertyName("genres")]
    public List<string>? Genres { get; set; }

    [JsonPropertyName("song_id")]
    public string? SongId { get; set; }

    [JsonPropertyName("thumbnail")]
    public FfiNowPlayingThumbnail? Thumbnail { get; set; }

    [JsonPropertyName("error")]
    public string? Error { get; set; }
}

public sealed class FfiNowPlayingSnapshot
{
    [JsonPropertyName("supported")]
    public bool Supported { get; set; }

    [JsonPropertyName("now_playing")]
    public FfiNowPlayingSession? NowPlaying { get; set; }

    [JsonPropertyName("sessions")]
    public List<FfiNowPlayingSession>? Sessions { get; set; }

    [JsonPropertyName("picked_app_id")]
    public string? PickedAppId { get; set; }

    [JsonPropertyName("retrieved_at_unix_ms")]
    public ulong RetrievedAtUnixMs { get; set; }
}

