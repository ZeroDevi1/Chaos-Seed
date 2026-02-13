using ChaosSeed.WinUI3.Services.NativeOverlay;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class NativeOverlayHitTestTests
{
    private static readonly NativeOverlayHitTest.Config Cfg = new(
        TopBarHeightPx: 32,
        CloseBtnSizePx: 18,
        CloseBtnPadPx: 8,
        ResizeGripPx: 8
    );

    [Fact]
    public void Locked_ClientArea_IsPassThrough()
    {
        var r = NativeOverlayHitTest.HitTest(800, 600, dx: 200, dy: 200, locked: true, Cfg);
        Assert.Equal(NativeOverlayHitTest.Region.PassThrough, r);
    }

    [Fact]
    public void Locked_Edge_IsResizable()
    {
        var r = NativeOverlayHitTest.HitTest(800, 600, dx: 2, dy: 200, locked: true, Cfg);
        Assert.Equal(NativeOverlayHitTest.Region.ResizeLeft, r);
    }

    [Fact]
    public void Locked_TopBar_IsCaption()
    {
        var r = NativeOverlayHitTest.HitTest(800, 600, dx: 10, dy: 10, locked: true, Cfg);
        Assert.Equal(NativeOverlayHitTest.Region.Caption, r);
    }

    [Fact]
    public void Locked_CloseButton_IsCloseButton()
    {
        var close = NativeOverlayHitTest.CloseButtonRect(800, Cfg);
        var r = NativeOverlayHitTest.HitTest(800, 600, dx: close.X + 1, dy: close.Y + 1, locked: true, Cfg);
        Assert.Equal(NativeOverlayHitTest.Region.CloseButton, r);
    }

    [Fact]
    public void EditMode_Edge_IsResizable()
    {
        var r = NativeOverlayHitTest.HitTest(800, 600, dx: 2, dy: 100, locked: false, Cfg);
        Assert.Equal(NativeOverlayHitTest.Region.ResizeLeft, r);
    }

    [Fact]
    public void EditMode_Center_IsClient()
    {
        var r = NativeOverlayHitTest.HitTest(800, 600, dx: 200, dy: 200, locked: false, Cfg);
        Assert.Equal(NativeOverlayHitTest.Region.Client, r);
    }
}
