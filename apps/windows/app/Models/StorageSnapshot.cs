using MemeDock.Bridge;

namespace MemeDock.Windows.Models;

/// <summary>
/// The three storage figures the settings page shows, taken from one
/// <see cref="SpaceStatistics"/> read.
/// </summary>
/// <remarks>
/// <para>
/// <see cref="OriginalBytes"/> is <see cref="SpaceStatistics.ReadyOriginalBytes"/>,
/// not <see cref="SpaceStatistics.KnownOriginalBytes"/>: the known total includes
/// assets whose local blob is not present, so adding it to the cache figure would
/// count bytes that are not on this device.
/// </para>
/// <para>
/// <see cref="CacheBytes"/> is thumbnails plus temporary share copies. The two
/// core fields are already directory totals, so their sum is the same number the
/// cleanup button can actually reclaim part of.
/// </para>
/// </remarks>
internal sealed record StorageSnapshot(long OriginalCount, long OriginalBytes, long CacheBytes)
{
    /// <summary>
    /// Rejects values that cannot be shown truthfully. The core never produces a
    /// negative total, so reaching this path means the bridge contract changed and
    /// the page has to show an error instead of a fabricated zero.
    /// </summary>
    public static bool TryCreate(SpaceStatistics statistics, out StorageSnapshot snapshot)
    {
        snapshot = null!;
        if (statistics.KnownAssets < 0
            || statistics.ReadyOriginalBytes < 0
            || statistics.ThumbnailBytes < 0
            || statistics.TemporaryShareBytes < 0)
        {
            return false;
        }

        long cache;
        try
        {
            cache = checked(statistics.ThumbnailBytes + statistics.TemporaryShareBytes);
        }
        catch (OverflowException)
        {
            return false;
        }

        snapshot = new StorageSnapshot(statistics.KnownAssets, statistics.ReadyOriginalBytes, cache);
        return true;
    }
}
