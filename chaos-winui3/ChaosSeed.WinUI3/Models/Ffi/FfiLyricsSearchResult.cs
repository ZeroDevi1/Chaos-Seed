using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Models.Ffi;

public sealed class FfiLyricsSearchResult
{
    [JsonPropertyName("service")]
    public string Service { get; set; } = "";

    [JsonPropertyName("service_token")]
    public string ServiceToken { get; set; } = "";

    [JsonPropertyName("title")]
    public string? Title { get; set; }

    [JsonPropertyName("artist")]
    public string? Artist { get; set; }

    [JsonPropertyName("album")]
    public string? Album { get; set; }

    [JsonPropertyName("duration_ms")]
    public ulong? DurationMs { get; set; }

    [JsonPropertyName("match_percentage")]
    public byte MatchPercentage { get; set; }

    [JsonPropertyName("quality")]
    public double Quality { get; set; }

    [JsonPropertyName("matched")]
    public bool Matched { get; set; }

    [JsonPropertyName("has_translation")]
    public bool HasTranslation { get; set; }

    [JsonPropertyName("has_inline_timetags")]
    public bool HasInlineTimetags { get; set; }

    [JsonPropertyName("lyrics_original")]
    public string LyricsOriginal { get; set; } = "";

    [JsonPropertyName("lyrics_translation")]
    public string? LyricsTranslation { get; set; }

    [JsonPropertyName("debug")]
    public object? Debug { get; set; }
}

