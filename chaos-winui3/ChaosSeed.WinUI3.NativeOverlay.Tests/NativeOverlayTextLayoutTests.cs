using System.Linq;
using ChaosSeed.WinUI3.Services.NativeOverlay;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class NativeOverlayTextLayoutTests
{
    [Fact]
    public void SplitTextElements_KeepsEmojiClusterIntact()
    {
        var elements = NativeOverlayTextLayout.SplitTextElements("A👨‍👩‍👧‍👦B");

        Assert.Equal(3, elements.Count);
        Assert.Equal("A", elements[0]);
        Assert.Equal("👨‍👩‍👧‍👦", elements[1]);
        Assert.Equal("B", elements[2]);
    }

    [Fact]
    public void CreateLayout_MixedText_HasStableRunsAndSize()
    {
        using var layoutEngine = new NativeOverlayTextLayout();
        const string text = "中文A①🤖";

        var layout = layoutEngine.CreateLayout(text, 20f);

        Assert.Equal(text, layout.Text);
        Assert.Equal(text, string.Concat(layout.Runs.Select(x => x.Text)));
        Assert.NotEmpty(layout.Runs);
        Assert.True(layout.Size.Width > 0f);
        Assert.True(layout.Size.Height > 0f);
    }
}
