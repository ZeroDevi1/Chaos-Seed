namespace ChaosSeed.WinUI3.Models;

public sealed class LivestreamDecodeManifestResult
{
    public string Site { get; set; } = "";
    public string RoomId { get; set; } = "";
    public string RawInput { get; set; } = "";
    public LiveInfo Info { get; set; } = new();
    public PlaybackHints Playback { get; set; } = new();
    public StreamVariant[] Variants { get; set; } = Array.Empty<StreamVariant>();
}

