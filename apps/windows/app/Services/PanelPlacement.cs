namespace MemeDock.Windows.Services;

public readonly record struct ScreenRect(int X, int Y, int Width, int Height)
{
    public int Right => X + Width;
    public int Bottom => Y + Height;
}

public static class PanelPlacement
{
    // Coordinates are physical pixels throughout; callers convert their effective size once.
    public static ScreenRect Place(ScreenRect work, ScreenRect anchor, int width, int height, int gap)
    {
        width = Math.Clamp(width, 1, Math.Max(1, work.Width));
        height = Math.Clamp(height, 1, Math.Max(1, work.Height));
        var x = Math.Clamp(anchor.X, work.X, work.Right - width);
        var y = anchor.Bottom + gap;
        if (y + height > work.Bottom) y = anchor.Y - height - gap;
        return new(x, Math.Clamp(y, work.Y, work.Bottom - height), width, height);
    }
}
