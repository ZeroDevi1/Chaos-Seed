namespace ChaosSeed.WinUI3.Services;

internal readonly record struct DanmakuLaneSpriteState(double X, double Width, double SpeedPxPerSec);

internal static class DanmakuOverlaySpawnGuard
{
    private const double ExitMarginPx = 10.0;
    private const double SpeedEpsilon = 0.01;

    public static bool CanSpawnAfter(
        DanmakuLaneSpriteState? previous,
        double spawnX,
        double minGapPx,
        double newSpeedPxPerSec
    )
    {
        if (previous is null)
        {
            return true;
        }

        var prev = previous.Value;
        var initialGap = spawnX - (prev.X + prev.Width);
        if (initialGap < minGapPx)
        {
            return false;
        }

        if (newSpeedPxPerSec <= prev.SpeedPxPerSec + SpeedEpsilon)
        {
            return true;
        }

        if (prev.SpeedPxPerSec <= SpeedEpsilon)
        {
            return false;
        }

        var exitTimeSec = Math.Max(0.0, (prev.X + prev.Width + ExitMarginPx) / prev.SpeedPxPerSec);
        var catchUpGap = (newSpeedPxPerSec - prev.SpeedPxPerSec) * exitTimeSec;
        return initialGap >= minGapPx + catchUpGap;
    }
}
