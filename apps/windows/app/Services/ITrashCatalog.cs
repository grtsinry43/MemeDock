using MemeDock.Bridge;

namespace MemeDock.Windows;

/// <summary>
/// The deleted-catalog calls <see cref="ViewModels.TrashGroupsViewModel"/> needs.
/// <see cref="LibrarySession"/> implements it; the seam exists so the wake-up,
/// coalescing and shutdown rules can run against a fake without the native
/// library, the same way <see cref="Services.ISyncAdapter"/> does for sync.
/// </summary>
internal interface ITrashCatalog
{
    Task<CollectionMetadata[]> CollectionsAsync(CancellationToken cancellation, bool deleted = false);
    Task<TagMetadata[]> TagsAsync(CancellationToken cancellation, bool deleted = false);
    Task<CollectionMetadata> RestoreCollectionAsync(EntityReference target, long deletedRevision, CancellationToken cancellation);
    Task<TagMetadata> RestoreTagAsync(EntityReference target, long deletedRevision, CancellationToken cancellation);
    Task PurgeCollectionAsync(EntityReference target, long deletedRevision, CancellationToken cancellation);
    Task PurgeTagAsync(EntityReference target, long deletedRevision, CancellationToken cancellation);
}
