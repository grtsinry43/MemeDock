namespace MemeDock.Windows.Models;

// Opened only when its turn in the batch arrives; the session owns and disposes
// the returned stream. No image buffers cross the native bridge.
internal sealed record StickerImportSource(string Name, Func<CancellationToken, Task<Stream>> OpenRead)
{
    internal static StickerImportSource FromPath(string path) => new(Path.GetFileName(path), cancellation =>
    {
        cancellation.ThrowIfCancellationRequested();
        return Task.FromResult<Stream>(new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read,
            65536, FileOptions.Asynchronous | FileOptions.SequentialScan));
    });
}
