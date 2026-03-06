using ChaosSeed.WinUI3.Services;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class DanmakuOverlaySpawnGuardTests
{
    [Fact]
    public void FasterSprite_RequiresExtraSpacingToAvoidCatchUp()
    {
        var previous = new DanmakuLaneSpriteState(X: 500, Width: 120, SpeedPxPerSec: 120);

        var canSpawn = DanmakuOverlaySpawnGuard.CanSpawnAfter(previous, spawnX: 970, minGapPx: 40, newSpeedPxPerSec: 260);

        Assert.False(canSpawn);
    }

    [Fact]
    public void SlowerSprite_CanReuseLaneWhenGapIsEnough()
    {
        var previous = new DanmakuLaneSpriteState(X: 500, Width: 120, SpeedPxPerSec: 220);

        var canSpawn = DanmakuOverlaySpawnGuard.CanSpawnAfter(previous, spawnX: 970, minGapPx: 40, newSpeedPxPerSec: 180);

        Assert.True(canSpawn);
    }
}

