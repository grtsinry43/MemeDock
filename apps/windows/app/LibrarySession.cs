using MemeDock.Bridge;

namespace MemeDock.Windows;

internal sealed class LibrarySession : IAsyncDisposable
{
    private readonly LibraryHandle _library;
    private readonly ulong _maxInputBytes;

    private LibrarySession(LibraryHandle library, ulong maxInputBytes)
        => (_library, _maxInputBytes) = (library, maxInputBytes);

    public static Task<LibrarySession> OpenAsync(string root) => Task.Run(async () =>
    {
        var limits = MemeDockNative.DefaultResourceConfiguration();
        var configuration = new LibraryConfiguration(
            Path.Combine(root, "data"), Path.Combine(root, "cache"),
            Path.Combine(root, "share"), limits);
        var library = await MemeDockNative.OpenLibrary(configuration).ConfigureAwait(false);
        return new LibrarySession(library, limits.MaxFileBytes);
    });

    public Task<StickerPage> QueryAsync(string text, QueryCursorHandle? cursor, CancellationToken cancellation)
        => Task.Run(async () =>
        {
            cancellation.ThrowIfCancellationRequested();
            var query = new StickerQuery(MemeDockNative.NewRequestId(), text, null, [], null,
                false, StickerSort.Recent, 80, cursor);
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

    public Task<ImportOutcome> ImportAsync(string source, CancellationToken cancellation) => Task.Run(async () =>
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
                        throw new IOException("图片超过当前表情库允许的文件大小。");
                    await writer.WriteAsync(buffer.AsMemory(0, read), cancellation).ConfigureAwait(false);
                }
                await writer.FlushAsync(cancellation).ConfigureAwait(false);
            }
            cancellation.ThrowIfCancellationRequested();
            using var request = _library.ImportStaged(input, new ImportOptions(Path.GetFileName(source), null, null));
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

    public async ValueTask DisposeAsync()
    {
        try { await Task.Run(() => _library.Shutdown()).ConfigureAwait(false); }
        finally { _library.Dispose(); }
    }
}
