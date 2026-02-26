using Newtonsoft.Json;
using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Models.Tts;

public sealed class TtsSftStartParams
{
    [JsonProperty("modelDir")]
    [JsonPropertyName("modelDir")]
    public string ModelDir { get; set; } = "";

    [JsonProperty("spkId")]
    [JsonPropertyName("spkId")]
    public string SpkId { get; set; } = "";

    [JsonProperty("text")]
    [JsonPropertyName("text")]
    public string Text { get; set; } = "";

    [JsonProperty("promptText")]
    [JsonPropertyName("promptText")]
    public string? PromptText { get; set; }

    // "inject" | "guide_prefix"
    [JsonProperty("promptStrategy")]
    [JsonPropertyName("promptStrategy")]
    public string? PromptStrategy { get; set; }

    [JsonProperty("guideSep")]
    [JsonPropertyName("guideSep")]
    public string? GuideSep { get; set; }

    [JsonProperty("speed")]
    [JsonPropertyName("speed")]
    public double? Speed { get; set; }

    [JsonProperty("seed")]
    [JsonPropertyName("seed")]
    public ulong? Seed { get; set; }

    [JsonProperty("temperature")]
    [JsonPropertyName("temperature")]
    public double? Temperature { get; set; }

    [JsonProperty("topP")]
    [JsonPropertyName("topP")]
    public double? TopP { get; set; }

    [JsonProperty("topK")]
    [JsonPropertyName("topK")]
    public uint? TopK { get; set; }

    [JsonProperty("winSize")]
    [JsonPropertyName("winSize")]
    public uint? WinSize { get; set; }

    [JsonProperty("tauR")]
    [JsonPropertyName("tauR")]
    public double? TauR { get; set; }

    [JsonProperty("textFrontend")]
    [JsonPropertyName("textFrontend")]
    public bool? TextFrontend { get; set; }
}

public sealed class TtsSftStartResult
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";
}

public sealed class TtsAudioResult
{
    [JsonProperty("mime")]
    [JsonPropertyName("mime")]
    public string Mime { get; set; } = "";

    [JsonProperty("wavBase64")]
    [JsonPropertyName("wavBase64")]
    public string WavBase64 { get; set; } = "";

    [JsonProperty("sampleRate")]
    [JsonPropertyName("sampleRate")]
    public uint SampleRate { get; set; }

    [JsonProperty("channels")]
    [JsonPropertyName("channels")]
    public ushort Channels { get; set; }

    [JsonProperty("durationMs")]
    [JsonPropertyName("durationMs")]
    public ulong DurationMs { get; set; }
}

public sealed class TtsSftStatus
{
    [JsonProperty("done")]
    [JsonPropertyName("done")]
    public bool Done { get; set; }

    // "pending" | "running" | "done" | "failed" | "canceled"
    [JsonProperty("state")]
    [JsonPropertyName("state")]
    public string State { get; set; } = "";

    [JsonProperty("stage")]
    [JsonPropertyName("stage")]
    public string? Stage { get; set; }

    [JsonProperty("error")]
    [JsonPropertyName("error")]
    public string? Error { get; set; }

    [JsonProperty("result")]
    [JsonPropertyName("result")]
    public TtsAudioResult? Result { get; set; }
}

