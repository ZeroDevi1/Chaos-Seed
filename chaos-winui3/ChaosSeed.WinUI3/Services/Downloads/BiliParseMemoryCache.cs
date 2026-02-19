using System;
using System.Linq;
using ChaosSeed.WinUI3.Models.Bili;

namespace ChaosSeed.WinUI3.Services.Downloads;

public sealed class BiliParseSnapshot
{
    public string Input { get; set; } = "";
    public string Title { get; set; } = "";
    public BiliPage[] Pages { get; set; } = Array.Empty<BiliPage>();
    public uint[] SelectedPages { get; set; } = Array.Empty<uint>();
    public long UpdatedAtUnixMs { get; set; }

    public bool HasData => Pages.Length > 0 && !string.IsNullOrWhiteSpace(Title);
}

public sealed class BiliParseMemoryCache
{
    private readonly object _lock = new();
    private BiliParseSnapshot? _snapshot;

    private static BiliParseSnapshot Clone(BiliParseSnapshot s)
        => new()
        {
            Input = (s.Input ?? "").Trim(),
            Title = (s.Title ?? "").Trim(),
            Pages = (s.Pages ?? Array.Empty<BiliPage>()).ToArray(),
            SelectedPages = (s.SelectedPages ?? Array.Empty<uint>()).ToArray(),
            UpdatedAtUnixMs = s.UpdatedAtUnixMs,
        };

    public BiliParseSnapshot? Get()
    {
        lock (_lock)
        {
            return _snapshot is null ? null : Clone(_snapshot);
        }
    }

    public void Set(BiliParseSnapshot? snapshot)
    {
        lock (_lock)
        {
            _snapshot = snapshot is null ? null : Clone(snapshot);
        }
    }

    public void Clear() => Set(null);
}

