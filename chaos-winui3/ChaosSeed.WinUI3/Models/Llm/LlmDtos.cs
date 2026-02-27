using Newtonsoft.Json;
using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Models.Llm;

public sealed class ChatMessage
{
    [JsonProperty("role")]
    [JsonPropertyName("role")]
    public string Role { get; set; } = "";

    [JsonProperty("content")]
    [JsonPropertyName("content")]
    public string Content { get; set; } = "";
}

public sealed class LlmConfigSetParams
{
    [JsonProperty("baseUrl")]
    [JsonPropertyName("baseUrl")]
    public string BaseUrl { get; set; } = "";

    [JsonProperty("apiKey")]
    [JsonPropertyName("apiKey")]
    public string ApiKey { get; set; } = "";

    [JsonProperty("model")]
    [JsonPropertyName("model")]
    public string Model { get; set; } = "";

    [JsonProperty("reasoningModel")]
    [JsonPropertyName("reasoningModel")]
    public string? ReasoningModel { get; set; }

    [JsonProperty("timeoutMs")]
    [JsonPropertyName("timeoutMs")]
    public ulong? TimeoutMs { get; set; }

    [JsonProperty("defaultTemperature")]
    [JsonPropertyName("defaultTemperature")]
    public double? DefaultTemperature { get; set; }
}

public sealed class LlmChatParams
{
    [JsonProperty("system")]
    [JsonPropertyName("system")]
    public string? System { get; set; }

    [JsonProperty("messages")]
    [JsonPropertyName("messages")]
    public List<ChatMessage> Messages { get; set; } = new();

    // "normal" | "reasoning"
    [JsonProperty("reasoningMode")]
    [JsonPropertyName("reasoningMode")]
    public string? ReasoningMode { get; set; }

    [JsonProperty("temperature")]
    [JsonPropertyName("temperature")]
    public double? Temperature { get; set; }

    [JsonProperty("maxTokens")]
    [JsonPropertyName("maxTokens")]
    public uint? MaxTokens { get; set; }
}

public sealed class LlmChatResult
{
    [JsonProperty("text")]
    [JsonPropertyName("text")]
    public string Text { get; set; } = "";
}

