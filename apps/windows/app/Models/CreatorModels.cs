using System.Globalization;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;

namespace MemeDock.Windows.Models;

/// <summary>Normalized [0, 1] crop rectangle relative to the decoded video frame.</summary>
public readonly record struct CreatorCrop(double X, double Y, double Width, double Height)
{
    public static CreatorCrop Full => new(0, 0, 1, 1);

    public double Right => X + Width;
    public double Bottom => Y + Height;
    public double MidX => X + Width / 2;
    public double MidY => Y + Height / 2;

    public bool IsCropped =>
        Math.Abs(X) > 0.0005
        || Math.Abs(Y) > 0.0005
        || Math.Abs(Width - 1) > 0.0005
        || Math.Abs(Height - 1) > 0.0005;
}

public readonly record struct CreatorDimensions(int Width, int Height);

public enum CreatorCropHandle
{
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right
}

public sealed record CreatorThumbnailItem(ulong TimeMs, string Path);

/// <summary>
/// Pure geometry, timeline, formatting, and error-mapping rules for Creator.
/// Kept free of WinUI types so the view model and tests share one implementation.
/// </summary>
public static class CreatorMath
{
    public const double MinNormalizedCrop = 0.05;
    public const double MinClipSeconds = 0.05;
    public const double MaxClipSeconds = 30.0;

    public static bool IsCorner(this CreatorCropHandle handle) => handle is
        CreatorCropHandle.TopLeft or CreatorCropHandle.TopRight
        or CreatorCropHandle.BottomLeft or CreatorCropHandle.BottomRight;

    public static CreatorDimensions CroppedPixels(uint videoWidth, uint videoHeight, CreatorCrop crop)
    {
        var w = Math.Max(1, (int)Math.Round(videoWidth * crop.Width));
        var h = Math.Max(1, (int)Math.Round(videoHeight * crop.Height));
        return new CreatorDimensions(w, h);
    }

    public static CreatorDimensions EstimatedOutputSize(uint videoWidth, uint videoHeight, CreatorCrop crop, int maxEdge)
    {
        var cropped = CroppedPixels(videoWidth, videoHeight, crop);
        var longest = Math.Max(cropped.Width, cropped.Height);
        if (longest <= maxEdge) return cropped;
        var scale = (double)maxEdge / longest;
        return new CreatorDimensions(
            Math.Max(1, (int)Math.Round(cropped.Width * scale)),
            Math.Max(1, (int)Math.Round(cropped.Height * scale)));
    }

    public static CreatorCrop CenteredCrop(uint videoWidth, uint videoHeight, double? aspect)
    {
        if (aspect is null || videoWidth == 0 || videoHeight == 0) return CreatorCrop.Full;
        var target = aspect.Value;
        var sourceAspect = (double)videoWidth / videoHeight;
        if (sourceAspect > target)
        {
            var width = target / sourceAspect;
            return new CreatorCrop((1 - width) / 2, 0, width, 1);
        }
        var height = sourceAspect / target;
        return new CreatorCrop(0, (1 - height) / 2, 1, height);
    }

    public static CreatorCrop MoveCrop(CreatorCrop origin, double dx, double dy)
    {
        var x = Math.Clamp(origin.X + dx, 0, Math.Max(0, 1 - origin.Width));
        var y = Math.Clamp(origin.Y + dy, 0, Math.Max(0, 1 - origin.Height));
        return new CreatorCrop(x, y, origin.Width, origin.Height);
    }

    public static CreatorCrop ResizeCrop(
        CreatorCrop origin,
        CreatorCropHandle handle,
        double dx,
        double dy,
        double? aspectLock,
        double sourceWidth,
        double sourceHeight)
    {
        if (!handle.IsCorner() || aspectLock is null || sourceWidth <= 0 || sourceHeight <= 0)
            return ResizeFreeform(origin, handle, dx, dy);

        var sourceAspect = sourceWidth / sourceHeight;
        var normAspect = aspectLock.Value / sourceAspect;
        return ResizeLocked(origin, handle, dx, dy, normAspect);
    }

    private static CreatorCrop ResizeFreeform(CreatorCrop origin, CreatorCropHandle handle, double dx, double dy)
    {
        var minX = origin.X;
        var maxX = origin.Right;
        var minY = origin.Y;
        var maxY = origin.Bottom;

        switch (handle)
        {
            case CreatorCropHandle.TopLeft:
                minX = Math.Clamp(origin.X + dx, 0, maxX - MinNormalizedCrop);
                minY = Math.Clamp(origin.Y + dy, 0, maxY - MinNormalizedCrop);
                break;
            case CreatorCropHandle.TopRight:
                maxX = Math.Clamp(origin.Right + dx, minX + MinNormalizedCrop, 1);
                minY = Math.Clamp(origin.Y + dy, 0, maxY - MinNormalizedCrop);
                break;
            case CreatorCropHandle.BottomLeft:
                minX = Math.Clamp(origin.X + dx, 0, maxX - MinNormalizedCrop);
                maxY = Math.Clamp(origin.Bottom + dy, minY + MinNormalizedCrop, 1);
                break;
            case CreatorCropHandle.BottomRight:
                maxX = Math.Clamp(origin.Right + dx, minX + MinNormalizedCrop, 1);
                maxY = Math.Clamp(origin.Bottom + dy, minY + MinNormalizedCrop, 1);
                break;
            case CreatorCropHandle.Top:
                minY = Math.Clamp(origin.Y + dy, 0, maxY - MinNormalizedCrop);
                break;
            case CreatorCropHandle.Bottom:
                maxY = Math.Clamp(origin.Bottom + dy, minY + MinNormalizedCrop, 1);
                break;
            case CreatorCropHandle.Left:
                minX = Math.Clamp(origin.X + dx, 0, maxX - MinNormalizedCrop);
                break;
            case CreatorCropHandle.Right:
                maxX = Math.Clamp(origin.Right + dx, minX + MinNormalizedCrop, 1);
                break;
        }
        return new CreatorCrop(minX, minY, maxX - minX, maxY - minY);
    }

    private static CreatorCrop ResizeLocked(
        CreatorCrop origin,
        CreatorCropHandle handle,
        double dx,
        double dy,
        double normAspect)
    {
        double anchorX, anchorY, rawW, rawH, maxW, maxH;
        bool isLeft, isTop;

        switch (handle)
        {
            case CreatorCropHandle.TopLeft:
                anchorX = origin.Right; anchorY = origin.Bottom;
                rawW = origin.Width - dx; rawH = origin.Height - dy;
                maxW = anchorX; maxH = anchorY;
                isLeft = true; isTop = true;
                break;
            case CreatorCropHandle.TopRight:
                anchorX = origin.X; anchorY = origin.Bottom;
                rawW = origin.Width + dx; rawH = origin.Height - dy;
                maxW = 1 - anchorX; maxH = anchorY;
                isLeft = false; isTop = true;
                break;
            case CreatorCropHandle.BottomLeft:
                anchorX = origin.Right; anchorY = origin.Y;
                rawW = origin.Width - dx; rawH = origin.Height + dy;
                maxW = anchorX; maxH = 1 - anchorY;
                isLeft = true; isTop = false;
                break;
            default: // BottomRight
                anchorX = origin.X; anchorY = origin.Y;
                rawW = origin.Width + dx; rawH = origin.Height + dy;
                maxW = 1 - anchorX; maxH = 1 - anchorY;
                isLeft = false; isTop = false;
                break;
        }

        double width, height;
        if (Math.Abs(dx) >= Math.Abs(dy))
        {
            width = Math.Max(MinNormalizedCrop, rawW);
            height = width / normAspect;
        }
        else
        {
            height = Math.Max(MinNormalizedCrop, rawH);
            width = height * normAspect;
        }

        if (width > maxW) { width = maxW; height = width / normAspect; }
        if (height > maxH) { height = maxH; width = height * normAspect; }
        if (width < MinNormalizedCrop) { width = MinNormalizedCrop; height = width / normAspect; }
        if (height < MinNormalizedCrop) { height = MinNormalizedCrop; width = height * normAspect; }

        width = Math.Min(1, width);
        height = Math.Min(1, height);
        var x = isLeft ? anchorX - width : anchorX;
        var y = isTop ? anchorY - height : anchorY;
        return new CreatorCrop(
            Math.Clamp(x, 0, Math.Max(0, 1 - width)),
            Math.Clamp(y, 0, Math.Max(0, 1 - height)),
            width,
            height);
    }

    public static (double Start, double End) ClampStart(double value, double currentEnd, double duration)
    {
        if (!double.IsFinite(value) || duration <= 0) return (0, Math.Max(0, duration));
        var end = Math.Clamp(currentEnd, Math.Min(duration, MinClipSeconds), duration);
        var minGap = Math.Min(duration, MinClipSeconds);
        var start = Math.Clamp(value, 0, Math.Max(0, end - minGap));
        if (end - start > MaxClipSeconds) end = Math.Min(duration, start + MaxClipSeconds);
        return (start, end);
    }

    public static (double Start, double End) ClampEnd(double value, double currentStart, double duration)
    {
        if (!double.IsFinite(value) || duration <= 0) return (0, Math.Max(0, duration));
        var start = Math.Clamp(currentStart, 0, duration);
        var minGap = Math.Min(duration, MinClipSeconds);
        var end = Math.Clamp(value, Math.Min(duration, start + minGap), duration);
        if (end - start > MaxClipSeconds) start = Math.Max(0, end - MaxClipSeconds);
        return (start, end);
    }

    public static (double Start, double End) MoveRange(double newStart, double currentStart, double currentEnd, double duration)
    {
        if (!double.IsFinite(newStart) || duration <= 0) return (currentStart, currentEnd);
        var span = Math.Clamp(currentEnd - currentStart, Math.Min(duration, MinClipSeconds), Math.Min(duration, MaxClipSeconds));
        var start = Math.Clamp(newStart, 0, Math.Max(0, duration - span));
        var end = Math.Min(duration, start + span);
        return (start, end);
    }

    public static ulong[] PreviewTimestampsMs(double durationSeconds, int count = 8)
    {
        if (count <= 0 || durationSeconds <= 0) return [];
        if (count == 1) return [0];
        var result = new List<ulong>(count);
        var seen = new HashSet<ulong>();
        var last = Math.Max(0, durationSeconds - 0.001);
        for (var index = 0; index < count; index++)
        {
            var ms = (ulong)Math.Round(last * index / (count - 1) * 1000.0);
            if (seen.Add(ms)) result.Add(ms);
        }
        return result.ToArray();
    }

    public static string FormatTime(double seconds)
    {
        var clamped = Math.Max(0, double.IsFinite(seconds) ? seconds : 0);
        var mins = (int)(clamped / 60);
        var secs = clamped - mins * 60;
        return mins > 0
            ? string.Format(CultureInfo.InvariantCulture, "{0}:{1:00.00}", mins, secs)
            : string.Format(CultureInfo.InvariantCulture, "{0:0.00}s", clamped);
    }

    public static string MapStageKey(string? stage) => stage switch
    {
        "copying_video" or "inspecting_video" => "creator_reading",
        "preparing_previews" => "creator_preparing_previews",
        "validating_creation" or "publishing_creation" or "committing" => "creator_validating",
        _ => "creator_generating"
    };

    public static string MapErrorKey(Exception error, ICatalogStrings strings)
    {
        if (error is BridgeException.Failure failure)
        {
            var detail = failure.Message ?? "";
            if (failure.code == ErrorCode.UnsupportedColorProfile) return "creator_sdr_only";
            if (failure.code == ErrorCode.ResourceLimit)
            {
                if (detail.Contains("decoder", StringComparison.OrdinalIgnoreCase)) return "creator_input_resolution";
                if (detail.Contains("2 GiB", StringComparison.OrdinalIgnoreCase)) return "creator_input_size";
                return "creator_resource_limit";
            }
            if (failure.code == ErrorCode.InvalidImage) return "creator_invalid_video";
            if (failure.code == ErrorCode.UnsupportedFormat) return "creator_unsupported";
            if (failure.code == ErrorCode.InvalidInput) return "creator_invalid_selection";
        }
        return strings.ErrorKey(error);
    }

    public static bool IsCancelled(Exception error)
        => error is OperationCanceledException
           || error is BridgeException.Failure { code: ErrorCode.Cancelled };
}
