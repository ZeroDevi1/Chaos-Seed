using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models;

public sealed class NowPlayingThumbnail
{
    [JsonProperty("mime")]
    public string Mime { get; set; } = "";

    [JsonProperty("base64")]
    public string Base64 { get; set; } = "";
}

public sealed class NowPlayingSession
{
    [JsonProperty("appId")]
    public string AppId { get; set; } = "";

    [JsonProperty("isCurrent")]
    public bool IsCurrent { get; set; }

    [JsonProperty("playbackStatus")]
    public string PlaybackStatus { get; set; } = "";

    [JsonProperty("title")]
    public string? Title { get; set; }

    [JsonProperty("artist")]
    public string? Artist { get; set; }

    [JsonProperty("albumTitle")]
    public string? AlbumTitle { get; set; }

    [JsonProperty("positionMs")]
    public ulong? PositionMs { get; set; }

    [JsonProperty("durationMs")]
    public ulong? DurationMs { get; set; }

    [JsonProperty("genres")]
    public string[] Genres { get; set; } = Array.Empty<string>();

    [JsonProperty("songId")]
    public string? SongId { get; set; }

    [JsonProperty("thumbnail")]
    public NowPlayingThumbnail? Thumbnail { get; set; }

    [JsonProperty("error")]
    public string? Error { get; set; }
}

public sealed class NowPlayingSnapshot
{
    [JsonProperty("supported")]
    public bool Supported { get; set; }

    [JsonProperty("nowPlaying")]
    public NowPlayingSession? NowPlaying { get; set; }

    [JsonProperty("sessions")]
    public NowPlayingSession[] Sessions { get; set; } = Array.Empty<NowPlayingSession>();

    [JsonProperty("pickedAppId")]
    public string? PickedAppId { get; set; }

    [JsonProperty("retrievedAtUnixMs")]
    public ulong RetrievedAtUnixMs { get; set; }
}

