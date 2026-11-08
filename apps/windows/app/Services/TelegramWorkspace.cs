using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

// The platform sequences requests; network access, conversion and import decisions stay in Rust.
internal interface ITelegramPack : IDisposable
{
    TelegramPackMetadata Metadata { get; }
    Task<string> PreviewAsync(string id, CancellationToken cancellation);
    Task<TelegramImportReport> ImportAsync(string[] ids, Action<TelegramImportReport> progress, CancellationToken cancellation);
}
internal interface ITelegramWorkspace
{
    Task<ITelegramPack> LoadAsync(string token, string query, CancellationToken cancellation);
}
internal sealed class TelegramWorkspace(LibrarySession session) : ITelegramWorkspace
{
    public Task<ITelegramPack> LoadAsync(string token, string query, CancellationToken cancellation) => session.LoadTelegramAsync(token, query, cancellation);
    internal sealed class Pack(LibraryHandle library, TelegramPackHandle handle) : ITelegramPack
    {
        public TelegramPackMetadata Metadata { get; } = handle.Metadata();
        public Task<string> PreviewAsync(string id, CancellationToken cancellation) => Task.Run(async () =>
        {
            cancellation.ThrowIfCancellationRequested();
            using var task = library.TelegramPreview(handle, id);
            using var registration = cancellation.Register(() => task.Cancel());
            return await task.AwaitResult().ConfigureAwait(false);
        }, cancellation);
        public async Task<TelegramImportReport> ImportAsync(string[] ids, Action<TelegramImportReport> progress, CancellationToken cancellation)
        {
            cancellation.ThrowIfCancellationRequested();
            using var task = await Task.Run(() => library.ImportTelegram(handle, ids), cancellation);
            using var registration = cancellation.Register(() => { try { task.Cancel(); } catch (BridgeException) { } });
            var result = task.AwaitResult();
            while (!result.IsCompleted)
            {
                var completed = await Task.WhenAny(result, Task.Delay(150));
                if (completed == result) break;
                progress(await Task.Run(task.Snapshot));
            }
            return await result;
        }
        public void Dispose() => handle.Dispose();
    }
}
