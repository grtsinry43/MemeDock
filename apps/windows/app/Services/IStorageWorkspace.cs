using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

/// <summary>
/// The storage operations the settings page needs from an open library. The
/// session is the production implementation; tests supply a fake so failure and
/// concurrency rules can be exercised without touching a real user directory.
/// </summary>
internal interface IStorageWorkspace
{
    /// <summary>The directory the core keeps the database and originals in.</summary>
    string DataDirectory { get; }

    /// <summary>The directory the core keeps derived files (thumbnails) in.</summary>
    string CacheDirectory { get; }

    /// <summary>The directory the core writes temporary share copies to.</summary>
    string ShareDirectory { get; }

    /// <summary>Reads one statistics snapshot. Never returns a partial result.</summary>
    Task<SpaceStatistics> StatisticsAsync(CancellationToken cancellation);

    /// <summary>
    /// Removes only the artifacts the core itself has retired (expired share
    /// copies, purged stickers' copies, and old orphan files in the share
    /// directory). The result is the number of entries removed, not a byte count.
    /// </summary>
    Task<ulong> CleanExportArtifactsAsync(CancellationToken cancellation);
}
