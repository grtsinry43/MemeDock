using System.Diagnostics;
using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

/// <summary>
/// Narrow surface over the native sync contract so the state machine can be
/// exercised without a library, a network or a Windows-only runtime.
/// </summary>
public interface ISyncAdapter
{
    Task<SyncStatus> StatusAsync(CancellationToken cancellation);
    Task<SyncStatus> ConnectAsync(string origin, string token, CancellationToken cancellation);
    Task<SyncStatus> SynchronizeAsync(string token, CancellationToken cancellation);
    Task<SyncStatus> SetPausedAsync(bool paused, CancellationToken cancellation);
    ISyncSubscription Subscribe();
}

/// <summary>A single-consumer stream of native sync status snapshots.</summary>
public interface ISyncSubscription : IDisposable
{
    Task<SyncStatus?> NextAsync();
    void Close();
}

internal sealed class LibrarySyncAdapter(LibrarySession session) : ISyncAdapter
{
    public Task<SyncStatus> StatusAsync(CancellationToken cancellation)
        => session.SyncStatusAsync(cancellation);

    public Task<SyncStatus> ConnectAsync(string origin, string token, CancellationToken cancellation)
        => session.ConnectSyncAsync(origin, token, cancellation);

    public Task<SyncStatus> SynchronizeAsync(string token, CancellationToken cancellation)
        => session.SynchronizeAsync(token, cancellation);

    public Task<SyncStatus> SetPausedAsync(bool paused, CancellationToken cancellation)
        => session.SetSyncPausedAsync(paused, cancellation);

    public ISyncSubscription Subscribe() => new LibrarySyncSubscription(session.SubscribeSync());
}

internal sealed class LibrarySyncSubscription(SyncSubscriptionHandle handle) : ISyncSubscription
{
    public Task<SyncStatus?> NextAsync() => Task.Run(() => handle.Next());

    public void Close()
    {
        try { handle.Unsubscribe(); }
        catch (Exception error) { Trace.WriteLine($"Sync subscription could not be closed: {error}"); }
    }

    public void Dispose()
    {
        Close();
        handle.Dispose();
    }
}
