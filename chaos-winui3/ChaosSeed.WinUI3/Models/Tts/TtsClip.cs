namespace ChaosSeed.WinUI3.Models.Tts;

/// <summary>
/// TTS 生成结果（仅内存态）。
/// </summary>
public sealed class TtsClip
{
    public string SessionId { get; set; } = "";
    public string Text { get; set; } = "";

    public string Mime { get; set; } = "audio/wav";
    public byte[] WavBytes { get; set; } = [];

    public uint SampleRate { get; set; }
    public ushort Channels { get; set; } = 1;
    public ulong DurationMs { get; set; }

    public DateTimeOffset CreatedAt { get; set; } = DateTimeOffset.Now;
}
