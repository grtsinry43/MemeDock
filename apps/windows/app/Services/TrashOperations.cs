using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

// The core restores and purges one entity at a time, keyed by the revision seen
// when the sticker was listed. Carrying that revision lets a concurrent edit or
// restore fail for that entry alone instead of being silently dismissed.
internal readonly record struct TrashEntry(string Id, long Generation, long Revision);

internal readonly record struct TrashOutcome(int Succeeded, int Failed, string? ErrorKey)
{
    public static TrashOutcome Empty => new(0, 0, null);
    public bool AnySucceeded => Succeeded > 0;
    public bool AnyFailed => Failed > 0;
}

internal static class TrashOperations
{
    public static Task<TrashOutcome> RestoreAsync(LibrarySession session, IReadOnlyList<TrashEntry> entries,
        Func<Exception, string> errorKey, CancellationToken cancellation) =>
        ApplyAsync(entries, entry => session.RestoreStickerAsync(
            new EntityReference(entry.Id, entry.Generation), entry.Revision, cancellation), errorKey, cancellation);

    public static Task<TrashOutcome> PurgeAsync(LibrarySession session, IReadOnlyList<TrashEntry> entries,
        Func<Exception, string> errorKey, CancellationToken cancellation) =>
        ApplyAsync(entries, entry => session.PurgeStickerAsync(
            new EntityReference(entry.Id, entry.Generation), entry.Revision, cancellation), errorKey, cancellation);

    private static async Task<TrashOutcome> ApplyAsync(IReadOnlyList<TrashEntry> entries,
        Func<TrashEntry, Task> apply, Func<Exception, string> errorKey, CancellationToken cancellation)
    {
        if (entries.Count == 0) return TrashOutcome.Empty;
        var succeeded = 0;
        var failed = 0;
        string? firstError = null;
        foreach (var entry in entries)
        {
            if (cancellation.IsCancellationRequested) break;
            try
            {
                await apply(entry).ConfigureAwait(false);
                succeeded++;
            }
            catch (OperationCanceledException) { break; }
            catch (Exception error)
            {
                // One stale or missing sticker must not abandon the rest of the
                // selection, so the remaining entries are still attempted.
                failed++;
                firstError ??= errorKey(error);
            }
        }
        return new TrashOutcome(succeeded, failed, firstError);
    }
}
