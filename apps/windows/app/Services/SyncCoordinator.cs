using System.Diagnostics;
using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

/// <summary>Immutable snapshot of everything the sync UI needs to render.</summary>
public sealed record SyncState(
    SyncStatus Status,
    bool Loaded,
    bool Running,
    bool HasCredential,
    bool Changing,
    Exception? Error)
{
    public static SyncState Initial { get; } = new(
        new SyncStatus(null, null, false, false, 0, 0, 0, null, null, 0),
        Loaded: false, Running: false, HasCredential: false, Changing: false, Error: null);
}

/// <summary>
/// Owns the native sync session for one open library: credential lookup, the
/// status subscription, automatic retries and the command gate. UI code only
/// submits commands and renders <see cref="State"/>.
/// </summary>
public sealed class SyncCoordinator : IAsyncDisposable
{
    private static readonly TimeSpan DefaultRetryFloor = TimeSpan.FromMilliseconds(500);
    private static readonly TimeSpan MaxRetry = TimeSpan.FromSeconds(60);
    private static readonly TimeSpan ChangeDebounce = TimeSpan.FromMilliseconds(500);

    private readonly ISyncAdapter _adapter;
    private readonly ISyncTokenStore _tokens;
    private readonly string _libraryId;
    private readonly TimeSpan _retryFloor;
    private readonly object _stateLock = new();
    private readonly object _commandLock = new();
    private readonly WakeSignal _wake = new();
    private readonly SemaphoreSlim _gate = new(1, 1);
    private readonly CancellationTokenSource _lifetime = new();
    private SyncState _state = SyncState.Initial;
    private ISyncSubscription? _subscription;
    private Task _observer = Task.CompletedTask;
    private Task _loop = Task.CompletedTask;
    private Task _debounce = Task.CompletedTask;
    private Task _command = Task.CompletedTask;
    private int _debouncing;
    private bool _commandQueued;
    private int _started;
    private int _stopping;
    private int _disposed;

    public SyncCoordinator(ISyncAdapter adapter, ISyncTokenStore tokens, string libraryId,
        TimeSpan? retryFloor = null)
    {
        _adapter = adapter;
        _tokens = tokens;
        _libraryId = libraryId;
        _retryFloor = retryFloor ?? DefaultRetryFloor;
    }

    public event EventHandler? StateChanged;

    public SyncState State
    {
        get { lock (_stateLock) return _state; }
    }

    /// <summary>
    /// Reads the current binding and credential, then starts observing native
    /// status changes. The automatic loop is optional so tests can drive every
    /// transition deterministically.
    /// </summary>
    public async Task StartAsync(bool runLoop = true)
    {
        if (Interlocked.Exchange(ref _started, 1) == 1) return;
        if (Volatile.Read(ref _stopping) == 1) return;
        try
        {
            var status = await _adapter.StatusAsync(_lifetime.Token).ConfigureAwait(false);
            var token = await LoadTokenAsync(status.Origin).ConfigureAwait(false);
            Mutate(current => current with
            {
                Status = status, Loaded = true, HasCredential = token is not null, Error = null
            });
        }
        catch (OperationCanceledException) when (_lifetime.IsCancellationRequested) { return; }
        catch (Exception error)
        {
            Mutate(current => current with { Loaded = true, Error = error });
        }
        if (Volatile.Read(ref _stopping) == 1) return;
        try
        {
            _subscription = _adapter.Subscribe();
            _observer = ObserveAsync(_subscription);
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Sync status subscription failed: {error}");
            Mutate(current => current with { Error = current.Error ?? error });
        }
        if (runLoop) _loop = LoopAsync(_lifetime.Token);
    }

    public Task ConnectAsync(string origin, string token)
        => RunCommandAsync(async () =>
        {
            var requested = (origin ?? "").Trim();
            if (requested.Length == 0 || string.IsNullOrEmpty(token)) return;
            SyncStatus status;
            try
            {
                status = await _adapter.ConnectAsync(requested, token, _lifetime.Token).ConfigureAwait(false);
            }
            catch (Exception) when (!_lifetime.IsCancellationRequested)
            {
                await RefreshStatusAsync().ConfigureAwait(false);
                throw;
            }
            Mutate(current => current with { Status = status, Loaded = true, Error = null });
            var stored = status.Origin ?? requested;
            try
            {
                await _tokens.SaveAsync(_libraryId, stored, token, _lifetime.Token).ConfigureAwait(false);
            }
            catch (OperationCanceledException) when (_lifetime.IsCancellationRequested) { throw; }
            catch (Exception error)
            {
                // Keep the binding from auto-syncing with a token we could not persist.
                var paused = await PauseQuietlyAsync().ConfigureAwait(false);
                Mutate(current => current with
                {
                    Status = paused ?? current.Status,
                    HasCredential = false,
                    Error = new SyncCredentialException("Could not save the sync token", error)
                });
                return;
            }
            Mutate(current => current with { HasCredential = true });
        });

    public Task SynchronizeNowAsync()
        => RunCommandAsync(() => SynchronizeLockedAsync(_lifetime.Token), wake: false);

    public Task SetPausedAsync(bool paused)
        => RunCommandAsync(async () =>
        {
            var status = await _adapter.SetPausedAsync(paused, _lifetime.Token).ConfigureAwait(false);
            Mutate(current => current with { Status = status, Error = null });
        });

    public Task DisconnectAsync()
        => RunCommandAsync(async () =>
        {
            var status = await _adapter.SetPausedAsync(true, _lifetime.Token).ConfigureAwait(false);
            var origin = status.Origin ?? State.Status.Origin;
            if (!string.IsNullOrWhiteSpace(origin))
            {
                try
                {
                    await _tokens.ClearAsync(_libraryId, origin, CancellationToken.None).ConfigureAwait(false);
                }
                catch (Exception error)
                {
                    Mutate(current => current with
                    {
                        Status = status, Error = new SyncCredentialException("Could not clear the sync token", error)
                    });
                    return;
                }
            }
            Mutate(current => current with { Status = status, HasCredential = false, Error = null });
        });

    /// <summary>Coalesces local library changes into one wake-up.</summary>
    public void NotifyContentChanged()
    {
        if (Volatile.Read(ref _stopping) == 1) return;
        if (Interlocked.Exchange(ref _debouncing, 1) == 1) return;
        _debounce = DebouncedWakeAsync();
    }

    /// <summary>Cancels native work without waiting; safe to call from a closing window.</summary>
    public void RequestStop()
    {
        if (Interlocked.Exchange(ref _stopping, 1) == 1) return;
        _lifetime.Cancel();
        _wake.Set();
        try { _subscription?.Close(); }
        catch (Exception error) { Trace.WriteLine(error); }
    }

    public async ValueTask DisposeAsync()
    {
        if (Interlocked.Exchange(ref _disposed, 1) == 1) return;
        RequestStop();
        Task command;
        lock (_commandLock) command = _command;
        try { await Task.WhenAll(_loop, _observer, _debounce, command).ConfigureAwait(false); }
        catch (Exception error) { Trace.WriteLine($"Sync shutdown: {error}"); }
        try { _subscription?.Dispose(); }
        catch (Exception error) { Trace.WriteLine(error); }
        _subscription = null;
        _gate.Dispose();
        _lifetime.Dispose();
    }

    /// <summary>
    /// Runs one command at a time. A request that arrives while another command
    /// is in flight is coalesced into it, so the caller always awaits real work
    /// and <see cref="DisposeAsync"/> keeps a handle on the running command.
    /// </summary>
    /// <param name="wake">
    /// Wakes the automatic loop once the command has released the gate, so a
    /// connection or a resume starts synchronizing without waiting out the
    /// current sleep. A manual sync passes <c>false</c>: it has just run, and
    /// waking would only make the loop repeat it immediately.
    /// </param>
    private Task RunCommandAsync(Func<Task> command, bool wake = true)
    {
        lock (_commandLock)
        {
            if (_commandQueued) return _command;
            _commandQueued = true;
            return _command = ExecuteAsync(command, wake);
        }
    }

    private async Task ExecuteAsync(Func<Task> command, bool wake)
    {
        try
        {
            if (Volatile.Read(ref _stopping) == 1) return;
            await _gate.WaitAsync(_lifetime.Token).ConfigureAwait(false);
            try
            {
                if (Volatile.Read(ref _stopping) == 1) return;
                Mutate(current => current with { Changing = true });
                await command().ConfigureAwait(false);
            }
            finally
            {
                Mutate(current => current with { Changing = false });
                _gate.Release();
            }
        }
        catch (OperationCanceledException) when (_lifetime.IsCancellationRequested) { }
        catch (Exception error) { Fail(error); }
        finally
        {
            // The semaphore is released before the slot is freed, so the next
            // admitted command can only start once this one has given it up.
            lock (_commandLock) _commandQueued = false;
            if (wake) _wake.Set();
        }
    }

    private async Task LoopAsync(CancellationToken cancellation)
    {
        var first = true;
        while (!cancellation.IsCancellationRequested)
        {
            if (!first)
            {
                try { await _wake.WaitAsync(LoopDelay(), cancellation).ConfigureAwait(false); }
                catch (OperationCanceledException) { break; }
                catch (ObjectDisposedException) { break; }
            }
            first = false;
            if (cancellation.IsCancellationRequested) break;
            try
            {
                await _gate.WaitAsync(cancellation).ConfigureAwait(false);
                try
                {
                    if (Volatile.Read(ref _stopping) == 1) break;
                    await SynchronizeLockedAsync(cancellation).ConfigureAwait(false);
                }
                finally { _gate.Release(); }
            }
            catch (Exception) when (cancellation.IsCancellationRequested) { break; }
            catch (OperationCanceledException) { break; }
            catch (Exception error) { Fail(error); }
        }
    }

    private async Task SynchronizeLockedAsync(CancellationToken cancellation)
    {
        var snapshot = State;
        if (!SyncPresentation.CanSynchronize(snapshot)) return;
        var origin = snapshot.Status.Origin;
        if (string.IsNullOrWhiteSpace(origin)) return;
        string? token;
        try { token = await LoadTokenAsync(origin).ConfigureAwait(false); }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested) { throw; }
        catch (Exception error)
        {
            Mutate(current => current with { HasCredential = false, Error = error });
            return;
        }
        if (token is null)
        {
            Mutate(current => current with { HasCredential = false });
            return;
        }
        Mutate(current => current with { Running = true, Error = null });
        try
        {
            var status = await _adapter.SynchronizeAsync(token, cancellation).ConfigureAwait(false);
            Mutate(current => current with { Status = status, HasCredential = true, Error = null });
        }
        catch (Exception) when (cancellation.IsCancellationRequested) { throw; }
        catch (OperationCanceledException) { throw; }
        catch (Exception error)
        {
            Fail(error);
            await RefreshStatusAsync().ConfigureAwait(false);
        }
        finally
        {
            Mutate(current => current with { Running = false });
        }
    }

    private async Task ObserveAsync(ISyncSubscription subscription)
    {
        try
        {
            while (Volatile.Read(ref _stopping) == 0)
            {
                var status = await subscription.NextAsync().ConfigureAwait(false);
                if (status is null || Volatile.Read(ref _stopping) == 1) break;
                var couldSynchronize = SyncPresentation.CanSynchronize(State);
                Mutate(current => current with { Status = status });
                // A pushed status never shortens the cadence the core asked for
                // (500 ms with pending work, 15 s idle, 30 s after a failure), so
                // a failed attempt is not retried straight away. It only lets the
                // loop act on a binding that became usable while it was asleep,
                // for example a pause cleared outside this client.
                if (!couldSynchronize && SyncPresentation.CanSynchronize(State))
                {
                    _wake.Set();
                }
            }
        }
        catch (Exception error)
        {
            if (Volatile.Read(ref _stopping) == 0)
            {
                Trace.WriteLine($"Sync status observation stopped: {error}");
                Fail(error);
            }
        }
    }

    private async Task DebouncedWakeAsync()
    {
        try
        {
            await Task.Delay(ChangeDebounce, _lifetime.Token).ConfigureAwait(false);
            if (Volatile.Read(ref _stopping) == 0) _wake.Set();
        }
        catch (OperationCanceledException) { }
        catch (ObjectDisposedException) { }
        finally { Volatile.Write(ref _debouncing, 0); }
    }

    private async Task<string?> LoadTokenAsync(string? origin)
    {
        if (string.IsNullOrWhiteSpace(origin)) return null;
        try
        {
            var token = await _tokens.LoadAsync(_libraryId, origin, _lifetime.Token).ConfigureAwait(false);
            return string.IsNullOrEmpty(token) ? null : token;
        }
        catch (OperationCanceledException) { throw; }
        catch (Exception error)
        {
            throw new SyncCredentialException("Could not read the sync token", error);
        }
    }

    private async Task<SyncStatus?> PauseQuietlyAsync()
    {
        try
        {
            return await _adapter.SetPausedAsync(true, CancellationToken.None).ConfigureAwait(false);
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Sync binding could not be paused: {error}");
            return null;
        }
    }

    private async Task RefreshStatusAsync()
    {
        try
        {
            var status = await _adapter.StatusAsync(CancellationToken.None).ConfigureAwait(false);
            Mutate(current => current with { Status = status });
        }
        catch (Exception error) { Trace.WriteLine($"Sync status could not be refreshed: {error}"); }
    }

    /// <summary>
    /// Connected bindings follow the core cadence (500 ms with pending work,
    /// 30 s after a failure, 15 s idle, which also discovers remote changes).
    /// A paused, disconnected or unauthorized binding has nothing to do until a
    /// command or a status push calls <see cref="WakeSignal.Set"/>, so the loop
    /// sleeps instead of polling.
    /// </summary>
    private TimeSpan LoopDelay()
    {
        // One snapshot for both decisions, so a concurrent status change cannot
        // pair "can synchronize" with another status' retry interval.
        var snapshot = State;
        if (!SyncPresentation.CanSynchronize(snapshot)) return Timeout.InfiniteTimeSpan;
        var floor = (long)_retryFloor.TotalMilliseconds;
        return TimeSpan.FromMilliseconds(
            Math.Clamp((long)snapshot.Status.RetryAfterMs, floor, (long)MaxRetry.TotalMilliseconds));
    }

    private void Mutate(Func<SyncState, SyncState> change)
    {
        lock (_stateLock) _state = change(_state);
        try { StateChanged?.Invoke(this, EventArgs.Empty); }
        catch (Exception error) { Trace.WriteLine($"Sync state listener failed: {error}"); }
    }

    private void Fail(Exception error)
    {
        Trace.WriteLine(error);
        Mutate(current => current with { Error = error });
    }

    /// <summary>Conflated signal: a wake requested during a run is never lost.</summary>
    private sealed class WakeSignal
    {
        private readonly object _lock = new();
        private TaskCompletionSource<bool> _pending = NewSource();
        private bool _signaled;

        public void Set()
        {
            lock (_lock)
            {
                _signaled = true;
                _pending.TrySetResult(true);
            }
        }

        public async Task WaitAsync(TimeSpan timeout, CancellationToken cancellation)
        {
            TaskCompletionSource<bool> waiter;
            lock (_lock)
            {
                _signaled = false;
                waiter = _pending;
            }
            using var expiry = CancellationTokenSource.CreateLinkedTokenSource(cancellation);
            var delay = Task.Delay(timeout, expiry.Token);
            var completed = await Task.WhenAny(waiter.Task, delay).ConfigureAwait(false);
            expiry.Cancel();
            if (completed == waiter.Task)
            {
                lock (_lock)
                {
                    if (ReferenceEquals(_pending, waiter))
                    {
                        _pending = NewSource();
                        if (_signaled) _pending.TrySetResult(true);
                    }
                }
            }
            cancellation.ThrowIfCancellationRequested();
        }

        private static TaskCompletionSource<bool> NewSource()
            => new(TaskCreationOptions.RunContinuationsAsynchronously);
    }
}
