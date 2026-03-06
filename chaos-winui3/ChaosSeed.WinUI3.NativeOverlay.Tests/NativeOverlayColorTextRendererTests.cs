using ChaosSeed.WinUI3.Services.NativeOverlay;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class NativeOverlayColorTextRendererTests
{
    [Theory]
    [InlineData("abc", false)]
    [InlineData("中文①", false)]
    [InlineData("😀", true)]
    [InlineData("A🤖B", true)]
    [InlineData("👨‍👩‍👧‍👦", true)]
    public void ShouldUseColorFontRendering_DetectsEmojiClusters(string text, bool expected)
    {
        Assert.Equal(expected, NativeOverlayColorTextRenderer.ShouldUseColorFontRendering(text));
    }
}
