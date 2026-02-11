namespace ChaosSeed.WinUI3.Models;

public sealed class StreamVariant
{
    public string Id { get; set; } = "";
    public string Label { get; set; } = "";
    public int Quality { get; set; }
    public int? Rate { get; set; }
    public string? Url { get; set; }
    public string[] BackupUrls { get; set; } = Array.Empty<string>();
}

