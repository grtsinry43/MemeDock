using MemeDock.Bridge;
using MemeDock.Windows.Models;

namespace MemeDock.Windows;

internal sealed class LibrarySession : IAsyncDisposable
{
    private readonly LibraryHandle _library;
    private readonly ulong _maxInputBytes;
    public string LibraryId { get; }

    private LibrarySession(LibraryHandle library, ulong maxInputBytes)
    {
        (_library, _maxInputBytes) = (library, maxInputBytes);
        LibraryId = library.Identity().LibraryId;
    }

    public static Task<LibrarySession> OpenAsync(string root) => Task.Run(async () =>
    {
        var limits = MemeDockNative.DefaultResourceConfiguration();
        var configuration = new LibraryConfiguration(
            Path.Combine(root, "data"), Path.Combine(root, "cache"),
            Path.Combine(root, "share"), limits);
        var library = await MemeDockNative.OpenLibrary(configuration).ConfigureAwait(false);
        return new LibrarySession(library, limits.MaxFileBytes);
    });

    public Task<StickerPage> QueryAsync(LibraryFilter filter, string text, StickerSort sort,
        QueryCursorHandle? cursor, CancellationToken cancellation)
        => Task.Run(async () =>
        {
            cancellation.ThrowIfCancellationRequested();
            var query = filter.Query(MemeDockNative.NewRequestId(), text, sort, cursor);
            using var request = _library.ListStickers(query);
            using var registration = cancellation.Register(() => request.Cancel());
            return await request.AwaitResult().ConfigureAwait(false);
        });

    public Task<string> ThumbnailAsync(string id, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.RequestThumbnail(id, Priority.Visible);
        using var registration = cancellation.Register(() => request.Cancel());
        return (await request.AwaitResult().ConfigureAwait(false)).Path;
    });

    public SubscriptionHandle Subscribe() => _library.Subscribe();

    public Task<CollectionMetadata[]> CollectionsAsync(CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.Collections(false);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<TagMetadata[]> TagsAsync(CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.Tags(false);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<ImportOutcome> ImportAsync(string source, string? collectionId, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var create = _library.CreateImportInput();
        using var input = await create.AwaitResult().ConfigureAwait(false);
        var submitted = false;
        try
        {
            await using (var reader = new FileStream(source, FileMode.Open, FileAccess.Read, FileShare.Read,
                65536, FileOptions.Asynchronous | FileOptions.SequentialScan))
            await using (var writer = new FileStream(input.Path(), FileMode.Truncate, FileAccess.Write,
                FileShare.None, 65536, FileOptions.Asynchronous | FileOptions.SequentialScan))
            {
                var buffer = new byte[65536];
                ulong total = 0;
                int read;
                while ((read = await reader.ReadAsync(buffer, cancellation).ConfigureAwait(false)) != 0)
                {
                    total += (ulong)read;
                    if (total > _maxInputBytes)
                        throw new BridgeException.Failure(ErrorCode.ResourceLimit, "Import exceeds max file bytes.");
                    await writer.WriteAsync(buffer.AsMemory(0, read), cancellation).ConfigureAwait(false);
                }
                await writer.FlushAsync(cancellation).ConfigureAwait(false);
            }
            cancellation.ThrowIfCancellationRequested();
            using var request = _library.ImportStaged(input, new ImportOptions(Path.GetFileName(source), null, collectionId));
            submitted = true;
            using var registration = cancellation.Register(() => request.Cancel());
            return await request.AwaitResult().ConfigureAwait(false);
        }
        catch (Exception original) when (!submitted)
        {
            try
            {
                using var discard = _library.DiscardImportInput(input);
                await discard.AwaitResult().ConfigureAwait(false);
            }
            catch (Exception cleanup)
            {
                throw new AggregateException("导入失败，临时文件将在下次打开表情库时恢复检查。", original, cleanup);
            }
            throw;
        }
    });

    public Task<StickerDetail> DetailAsync(string id, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.StickerDetail(id);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<StickerMetadata> PatchAsync(EntityReference target, StickerEdit edit, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.PatchSticker(target, edit);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<bool> OrganizeAsync(EntityReference target, bool changeCollection, EntityReference? collection, EntityReference[]? tags, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.SetStickerOrganization(target, changeCollection, collection, tags);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<CollectionMetadata> CreateCollectionAsync(string name, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.CreateCollection(name, null);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<CollectionMetadata> RenameCollectionAsync(EntityReference target, string name, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.RenameCollection(target, name);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<CollectionMetadata> DeleteCollectionAsync(EntityReference target, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.DeleteCollection(target);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<CollectionMetadata> RestoreCollectionAsync(EntityReference target, long deletedRevision, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.RestoreCollection(target, deletedRevision);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<CollectionMetadata> MoveCollectionAsync(EntityReference target, string? before, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.MoveCollection(target, before);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<TagMetadata> CreateTagAsync(string name, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.CreateTag(name);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<TagMetadata> RenameTagAsync(EntityReference target, string name, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.RenameTag(target, name);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<TagMetadata> DeleteTagAsync(EntityReference target, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.DeleteTag(target);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<TagMetadata> RestoreTagAsync(EntityReference target, long deletedRevision, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.RestoreTag(target, deletedRevision);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<StickerMetadata> DeleteStickerAsync(EntityReference target, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.DeleteSticker(target);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<StickerMetadata> RestoreStickerAsync(EntityReference target, long deletedRevision, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.RestoreSticker(target, deletedRevision);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });
    public Task MoveCollectionItemAsync(EntityReference collection, EntityReference sticker, string? before,
        CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.MoveCollectionItem(collection, sticker, before);
        using var registration = cancellation.Register(() => request.Cancel());
        await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<BatchTask> StartBatchAsync(BatchTarget[] targets, BatchAction action, CancellationToken cancellation)
        => Task.Run(() =>
        {
            cancellation.ThrowIfCancellationRequested();
            return _library.Batch(targets, action);
        });

    public Task<ArtifactLeaseHandle> ExportAsync(string id, ExportOptions options, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.Export(id, options);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<ExportArtifact> PrepareHandoffAsync(ArtifactLeaseHandle lease, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.PrepareHandoff(lease);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task<string> ProtectClipboardAsync(ArtifactLeaseHandle lease, CancellationToken cancellation) => Task.Run(async () =>
    {
        cancellation.ThrowIfCancellationRequested();
        using var request = _library.ProtectClipboard(lease);
        using var registration = cancellation.Register(() => request.Cancel());
        return await request.AwaitResult().ConfigureAwait(false);
    });

    public Task ReconcileClipboardAsync(string? reference) => Task.Run(async () =>
    {
        using var request = _library.ReconcileClipboard(reference);
        await request.AwaitResult().ConfigureAwait(false);
    });

    public Task AbortClipboardAsync(string reference) => Task.Run(async () =>
    {
        using var request = _library.AbortClipboard(reference);
        await request.AwaitResult().ConfigureAwait(false);
    });

    public Task RecordUseAsync(string id, UsageAction action) => Task.Run(async () =>
    {
        using var request = _library.RecordUse(id, action);
        await request.AwaitResult().ConfigureAwait(false);
    });

    public async ValueTask DisposeAsync()
    {
        try { await Task.Run(() => _library.Shutdown()).ConfigureAwait(false); }
        finally { _library.Dispose(); }
    }
}
