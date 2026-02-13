namespace ChaosSeed.WinUI3.Services.NativeOverlay;

internal static class NativeOverlayHitTest
{
    public readonly record struct RectI(int X, int Y, int W, int H)
    {
        public bool Contains(int x, int y) => x >= X && x < X + W && y >= Y && y < Y + H;
    }

    public sealed record Config(
        int TopBarHeightPx,
        int CloseBtnSizePx,
        int CloseBtnPadPx,
        int ResizeGripPx
    );

    public enum Region
    {
        PassThrough = 0,
        Client = 1,
        Caption = 2,
        CloseButton = 3,
        ResizeLeft = 10,
        ResizeRight = 11,
        ResizeTop = 12,
        ResizeTopLeft = 13,
        ResizeTopRight = 14,
        ResizeBottom = 15,
        ResizeBottomLeft = 16,
        ResizeBottomRight = 17,
    }

    public static RectI CloseButtonRect(int wPx, Config cfg)
    {
        var size = Math.Clamp(cfg.CloseBtnSizePx, 16, 64);
        var pad = Math.Clamp(cfg.CloseBtnPadPx, 0, 32);
        return new RectI(Math.Max(0, wPx - pad - size), pad, size, size);
    }

    public static Region HitTest(
        int wPx,
        int hPx,
        int dx,
        int dy,
        bool locked,
        Config cfg
    )
    {
        if (wPx <= 1 || hPx <= 1)
        {
            return Region.PassThrough;
        }

        // Top bar is always interactive (so user can focus/move/close overlay even when locked).
        if (dy >= 0 && dy < cfg.TopBarHeightPx)
        {
            if (CloseButtonRect(wPx, cfg).Contains(dx, dy))
            {
                return Region.CloseButton;
            }
            return Region.Caption;
        }

        // Edges are resizable in both modes; the difference is whether the center is pass-through (locked)
        // or interactive (edit).
        var grip = Math.Clamp(cfg.ResizeGripPx, 4, 64);
        var left = dx >= 0 && dx < grip;
        var right = dx >= (wPx - grip) && dx < wPx;
        var top = dy >= 0 && dy < grip;
        var bottom = dy >= (hPx - grip) && dy < hPx;

        if (left && top) return Region.ResizeTopLeft;
        if (right && top) return Region.ResizeTopRight;
        if (left && bottom) return Region.ResizeBottomLeft;
        if (right && bottom) return Region.ResizeBottomRight;
        if (left) return Region.ResizeLeft;
        if (right) return Region.ResizeRight;
        if (top) return Region.ResizeTop;
        if (bottom) return Region.ResizeBottom;

        return locked ? Region.PassThrough : Region.Client;
    }
}
