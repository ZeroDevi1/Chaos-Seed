using Newtonsoft.Json;
using System.Text.Json.Serialization;
using ChaosSeed.WinUI3.Models.Llm;

namespace ChaosSeed.WinUI3.Models.Voice;

public sealed class VoiceChatStreamStartParams
{
    [JsonProperty("modelDir")]
    [JsonPropertyName("modelDir")]
    public string ModelDir { get; set; } = "";

    [JsonProperty("spkId")]
    [JsonPropertyName("spkId")]
    public string SpkId { get; set; } = "";

    [JsonProperty("messages")]
    [JsonPropertyName("messages")]
    public List<ChatMessage> Messages { get; set; } = new();

    // "normal" | "reasoning"
    [JsonProperty("reasoningMode")]
    [JsonPropertyName("reasoningMode")]
    public string? ReasoningMode { get; set; }

    // TTS options (same shape as tts.sft.start)
    [JsonProperty("promptText")]
    [JsonPropertyName("promptText")]
    public string? PromptText { get; set; }

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

    // Streaming options
    [JsonProperty("chunkMs")]
    [JsonPropertyName("chunkMs")]
    public uint? ChunkMs { get; set; }
}

public sealed class VoiceChatStreamStartResult
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("sampleRate")]
    [JsonPropertyName("sampleRate")]
    public uint SampleRate { get; set; }

    [JsonProperty("channels")]
    [JsonPropertyName("channels")]
    public ushort Channels { get; set; }

    [JsonProperty("format")]
    [JsonPropertyName("format")]
    public string Format { get; set; } = "";
}

public sealed class VoiceChatChunkNotif
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("seq")]
    [JsonPropertyName("seq")]
    public ulong Seq { get; set; }

    [JsonProperty("pcmBase64")]
    [JsonPropertyName("pcmBase64")]
    public string PcmBase64 { get; set; } = "";

    [JsonProperty("isLast")]
    [JsonPropertyName("isLast")]
    public bool IsLast { get; set; }
}

