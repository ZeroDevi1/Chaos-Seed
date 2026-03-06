namespace ChaosSeed.WinUI3.Services.NativeOverlay;

internal static class NativeOverlayLaneScheduler
{
    public static int ComputeLaneCount(
        double stageHeightPx,
        double areaRatio,
        double laneHeightPx,
        double topPadPx,
        double bottomPadPx
    )
    {
        if (laneHeightPx <= 0)
        {
            return 1;
        }

        var clampedAreaRatio = Math.Clamp(areaRatio, 0.0, 1.0);
        var availableHeight = stageHeightPx * clampedAreaRatio - topPadPx - bottomPadPx;
        return Math.Max(1, (int)Math.Floor(availableHeight / laneHeightPx));
    }

    public static int FindAvailableLane(
        IReadOnlyList<double> laneTail,
        double maxTail,
        int laneCursor,
        out int nextLaneCursor
    )
    {
        nextLaneCursor = laneCursor;
        if (laneTail.Count == 0)
        {
            return -1;
        }

        for (var i = 0; i < laneTail.Count; i++)
        {
            var lane = (laneCursor + i) % laneTail.Count;
            if (laneTail[lane] < maxTail)
            {
                nextLaneCursor = (lane + 1) % laneTail.Count;
                return lane;
            }
        }

        return -1;
    }
}
