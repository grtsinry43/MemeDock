using System.Globalization;

namespace MemeDock.Windows.Services;

/// <summary>
/// Formats a byte count the way the Android client does: base 1024, at most one
/// fractional digit, and the same unit names. The value is always a count of
/// bytes the core reported; this type never estimates or rounds beyond display.
/// </summary>
internal static class ByteSize
{
    private static readonly string[] Units = ["B", "KB", "MB", "GB", "TB", "PB"];

    /// <summary>
    /// The bridge reports byte totals as signed integers. A negative value would
    /// mean the core broke its contract, so it is rejected instead of displayed.
    /// </summary>
    public static string Format(long bytes)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(bytes);

        var unit = 0;
        var divisor = 1L;
        while (unit < Units.Length - 1 && bytes >= divisor * 1024)
        {
            divisor *= 1024;
            unit++;
        }
        // One decimal can round up into the next unit (1048575 B -> "1024.0 KB");
        // promote so a formatted value never reads as 1024 of a smaller unit.
        while (unit < Units.Length - 1
               && Math.Round((double)bytes / divisor, 1, MidpointRounding.AwayFromZero) >= 1024)
        {
            divisor *= 1024;
            unit++;
        }
        if (unit == 0) return bytes.ToString(CultureInfo.InvariantCulture) + " B";
        var value = Math.Round((double)bytes / divisor, 1, MidpointRounding.AwayFromZero);
        return value.ToString("0.#", CultureInfo.InvariantCulture) + " " + Units[unit];
    }
}
