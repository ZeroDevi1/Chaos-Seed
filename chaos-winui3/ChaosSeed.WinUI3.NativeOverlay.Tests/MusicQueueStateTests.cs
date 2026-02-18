using ChaosSeed.WinUI3.Models.Music;
using ChaosSeed.WinUI3.Services;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class MusicQueueStateTests
{
    private static MusicTrack Track(string service, string id, string title = "t")
        => new()
        {
            Service = service,
            Id = id,
            Title = title,
        };

    [Fact]
    public void Enqueue_DedupByKey()
    {
        var q = new MusicQueueState();
        var a = new MusicQueueItem(Track("qq", "1"), "flac");
        var b = new MusicQueueItem(Track("qq", "1"), "mp3_320");

        Assert.True(q.Enqueue(a, out var idx1));
        Assert.Equal(0, idx1);

        Assert.False(q.Enqueue(b, out var idx2));
        Assert.Equal(0, idx2);
        Assert.Equal(1, q.Items.Count);
    }

    [Fact]
    public void PlayNow_InsertsAfterCurrent()
    {
        var q = new MusicQueueState();
        var a = new MusicQueueItem(Track("qq", "1", "a"), "flac");
        var b = new MusicQueueItem(Track("qq", "2", "b"), "flac");
        var c = new MusicQueueItem(Track("qq", "3", "c"), "flac");

        q.PlayNow(a, out var idxA, out var insA);
        Assert.True(insA);
        Assert.Equal(0, idxA);
        Assert.Equal(0, q.CurrentIndex);

        q.Enqueue(b, out var idxB);
        Assert.Equal(1, idxB);

        q.PlayNow(c, out var idxC, out var insC);
        Assert.True(insC);
        Assert.Equal(1, idxC);
        Assert.Equal(1, q.CurrentIndex);
        Assert.Equal("a", q.Items[0].Track.Title);
        Assert.Equal("c", q.Items[1].Track.Title);
        Assert.Equal("b", q.Items[2].Track.Title);
    }

    [Fact]
    public void PlayNow_Existing_JustSelects()
    {
        var q = new MusicQueueState();
        var a = new MusicQueueItem(Track("qq", "1", "a"), "flac");
        var b = new MusicQueueItem(Track("qq", "2", "b"), "flac");

        q.Enqueue(a, out _);
        q.Enqueue(b, out _);

        q.PlayNow(b, out var idx, out var inserted);
        Assert.False(inserted);
        Assert.Equal(1, idx);
        Assert.Equal(1, q.CurrentIndex);
    }

    [Fact]
    public void NextPrev_RespectsLoopMode()
    {
        var q = new MusicQueueState();
        q.Enqueue(new MusicQueueItem(Track("qq", "1"), "flac"), out _);
        q.Enqueue(new MusicQueueItem(Track("qq", "2"), "flac"), out _);
        q.TrySetCurrentIndex(1);

        Assert.False(q.TryGetNextIndex(MusicLoopMode.Single, out _));
        Assert.True(q.TryGetNextIndex(MusicLoopMode.All, out var next));
        Assert.Equal(0, next);

        q.TrySetCurrentIndex(0);
        Assert.False(q.TryGetPrevIndex(MusicLoopMode.Off, out _));
        Assert.True(q.TryGetPrevIndex(MusicLoopMode.All, out var prev));
        Assert.Equal(1, prev);
    }
}

