using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

namespace ChaosSeed.WinUI3.Models;

public sealed class LyricsSearchResult
{
    [JsonProperty("service")]
    public string Service { get; set; } = "";

    [JsonProperty("serviceToken")]
    public string ServiceToken { get; set; } = "";

    [JsonProperty("title")]
    public string? Title { get; set; }

    [JsonProperty("artist")]
    public string? Artist { get; set; }

    [JsonProperty("album")]
    public string? Album { get; set; }

    [JsonProperty("durationMs")]
    public ulong? DurationMs { get; set; }

    [JsonProperty("matchPercentage")]
    public byte MatchPercentage { get; set; }

    [JsonProperty("quality")]
    public double Quality { get; set; }

    [JsonProperty("matched")]
    public bool Matched { get; set; }

    [JsonProperty("hasTranslation")]
    public bool HasTranslation { get; set; }

    [JsonProperty("hasInlineTimetags")]
    public bool HasInlineTimetags { get; set; }

    [JsonProperty("lyricsOriginal")]
    public string LyricsOriginal { get; set; } = "";

    [JsonProperty("lyricsTranslation")]
    public string? LyricsTranslation { get; set; }

    [JsonProperty("debug")]
    public JToken? Debug { get; set; }
}

