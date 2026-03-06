using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services.NativeOverlay;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class NativeOverlayMetricsTests
{
    [Fact]
    public void FromValues_UsesNativeOverlayBaseline()
    {
        var metrics = NativeOverlayMetricsCalculator.FromValues(1f, 1.0, DanmakuOverlayAreaMode.Full);

        Assert.Equal(20f, metrics.FontSizePx);
        Assert.Equal(84f, metrics.ImageSizePx);
        Assert.Equal(96f, metrics.LaneHeightPx);
        Assert.Equal(40f, metrics.GapPx);
        Assert.Equal(1.0, metrics.AreaRatio, 3);
        Assert.Equal(6, metrics.MaxSpawn);
    }

    [Fact]
    public void ComputeLaneCount_AndDensityFollowSharedSettings()
    {
        var metrics = NativeOverlayMetricsCalculator.FromValues(1f, 0.5, DanmakuOverlayAreaMode.Quarter);

        var laneCount = NativeOverlayLaneScheduler.ComputeLaneCount(540, metrics.AreaRatio, metrics.LaneHeightPx, 10, 10);

        Assert.Equal(3, metrics.MaxSpawn);
        Assert.Equal(1, laneCount);
    }

    [Fact]
    public void FindAvailableLane_UsesLaneCursorOrder()
    {
        var laneTail = new[] { 300d, 120d, 40d };

        var lane = NativeOverlayLaneScheduler.FindAvailableLane(laneTail, 100d, 1, out var nextLaneCursor);

        Assert.Equal(2, lane);
        Assert.Equal(0, nextLaneCursor);
    }
}
