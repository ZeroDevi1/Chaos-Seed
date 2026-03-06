using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.NativeOverlay;

internal readonly record struct NativeOverlayMetrics(
    float FontSizePx,
    float ImageSizePx,
    float LaneHeightPx,
    float GapPx,
    double AreaRatio,
    int MaxSpawn
);

internal static class NativeOverlayMetricsCalculator
{
    public const float BaseFontSizePx = 20f;
    public const float BaseImageSizePx = 84f;
    public const float MinFontScale = 0.5f;
    public const float MaxFontScale = 2.0f;
    public const float MinGapPx = 20f;
    public const float GapScale = 40f;
    public const float VerticalPaddingPx = 12f;
    public const float TextHeightPadPx = 10f;
    public const float MinLaneHeightPx = 28f;

    public static NativeOverlayMetrics FromSettings(AppSettings settings)
    {
        var fontScale = settings is null
            ? 1f
            : Math.Clamp((float)settings.DanmakuOverlayFontScale, MinFontScale, MaxFontScale);
        var density = settings is null ? 1.0 : Clamp01(settings.DanmakuOverlayDensity);
        var area = settings?.DanmakuOverlayArea ?? DanmakuOverlayAreaMode.Full;
        return FromValues(fontScale, density, area);
    }

    public static NativeOverlayMetrics FromOverlayWindowSettings(AppSettings settings)
    {
        var fontScale = settings is null
            ? 1f
            : Math.Clamp((float)settings.DanmakuOverlayWindowFontScale, MinFontScale, MaxFontScale);
        var density = settings is null ? 1.0 : Clamp01(settings.DanmakuOverlayWindowDensity);
        var area = settings?.DanmakuOverlayWindowArea ?? DanmakuOverlayAreaMode.Full;
        return FromValues(fontScale, density, area);
    }

    public static NativeOverlayMetrics FromValues(float fontScale, double density, DanmakuOverlayAreaMode area)
    {
        fontScale = Math.Clamp(fontScale, MinFontScale, MaxFontScale);
        density = Clamp01(density);

        var fontSizePx = BaseFontSizePx * fontScale;
        var imageSizePx = BaseImageSizePx * fontScale;
        var textHeightPx = fontSizePx + TextHeightPadPx;
        var laneHeightPx = Math.Max(MinLaneHeightPx, Math.Max(textHeightPx, imageSizePx) + VerticalPaddingPx);
        var gapPx = Math.Max(MinGapPx, GapScale * fontScale);
        var maxSpawn = Math.Clamp((int)Math.Round(density * 6.0), 0, 6);

        return new NativeOverlayMetrics(
            FontSizePx: fontSizePx,
            ImageSizePx: imageSizePx,
            LaneHeightPx: laneHeightPx,
            GapPx: gapPx,
            AreaRatio: GetAreaRatio(area),
            MaxSpawn: maxSpawn
        );
    }

    public static double GetAreaRatio(DanmakuOverlayAreaMode area)
    {
        return area switch
        {
            DanmakuOverlayAreaMode.Quarter => 0.25,
            DanmakuOverlayAreaMode.Half => 0.5,
            DanmakuOverlayAreaMode.ThreeQuarter => 0.75,
            _ => 1.0,
        };
    }

    private static double Clamp01(double value)
    {
        if (double.IsNaN(value) || double.IsInfinity(value))
        {
            return 1.0;
        }

        return Math.Clamp(value, 0.0, 1.0);
    }
}
