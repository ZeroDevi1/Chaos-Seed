namespace ChaosSeed.WinUI3.Models;

public sealed class DanmakuMessage
{
    public string SessionId { get; set; } = "";
    public long ReceivedAtMs { get; set; }
    public string User { get; set; } = "";
    public string Text { get; set; } = "";
    public string? ImageUrl { get; set; }
    public uint? ImageWidth { get; set; }
}

