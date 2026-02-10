namespace ChaosSeed.WinUI3.Models;

public sealed class DanmakuFetchImageResult
{
    public string Mime { get; set; } = "image/png";
    public string Base64 { get; set; } = "";
    public uint? Width { get; set; }
}

