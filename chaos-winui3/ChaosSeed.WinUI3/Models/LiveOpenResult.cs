namespace ChaosSeed.WinUI3.Models;

public sealed class LiveOpenResult
{
    public string SessionId { get; set; } = "";
    public string Site { get; set; } = "";
    public string RoomId { get; set; } = "";
    public string Title { get; set; } = "";
    public string VariantId { get; set; } = "";
    public string VariantLabel { get; set; } = "";
    public string Url { get; set; } = "";
    public string[] BackupUrls { get; set; } = Array.Empty<string>();
    public string? Referer { get; set; }
    public string? UserAgent { get; set; }
}

