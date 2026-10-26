using System.Diagnostics;

namespace MemeDock.Windows.ViewModels;

/// <summary>
/// Runs catalog refreshes one at a time and merges overlapping requests into a
/// single pending pass. A request that arrives while another pass is running, or
/// while <see cref="Suspend"/> marks a business operation, is latched instead of
/// dropped, so a notification cannot be lost by arriving a moment too early.
/// Every request adds at most one pass, which keeps coalescing bounded: a burst
/// of N notifications during one pass still produces a single follow-up pass.
/// Entry points are called from the UI thread, like the rest of the view model.
/// </summary>
internal sealed class RefreshQueue
{
    private readonly Func<Task<bool>> _pass;
    private readonly object _sync = new();
    private Task _drain = Task.CompletedTask;
    private bool _pending;
    private bool _suspended;
    private bool _stopped;
    private bool _draining;

    /// <param name="pass">One reflection of the catalog.</param>
    public RefreshQueue(Func<Task<bool>> pass) => _pass = pass;

    /// <summary>The pass that is running or last ran, so shutdown can await it.</summary>
    public Task Completion
    {
        get { lock (_sync) return _drain; }
    }

    /// <summary>
    /// Requests one pass. A request that arrives while a pass is running is
    /// merged into that drain, so the returned task settles only after the pass
    /// it caused. A request that arrives while passes are suspended settles
    /// immediately with the drain that last ran: the pass covering it starts
    /// later from <see cref="Resume"/>, which this task does not wait for. A
    /// request that arrives after <see cref="Stop"/> settles immediately and
    /// adds no pass.
    /// </summary>
    public Task Request()
    {
        lock (_sync)
        {
            if (_stopped) return Task.CompletedTask;
            _pending = true;
            if (_suspended || _draining) return _drain;
            _draining = true;
        }
        return Start();
    }

    /// <summary>
    /// Holds passes back while a business operation owns the catalog, so the
    /// operation is not racing a query that would publish a stale snapshot.
    /// </summary>
    public void Suspend()
    {
        lock (_sync) _suspended = true;
    }

    /// <summary>
    /// Ends a suspended span. An operation always changes the catalog, so this
    /// runs at least one pass even when no notification arrived meanwhile.
    /// </summary>
    public Task Resume()
    {
        lock (_sync)
        {
            _suspended = false;
            if (_stopped) return Task.CompletedTask;
            _pending = true;
            if (_draining) return _drain;
            _draining = true;
        }
        return Start();
    }

    /// <summary>
    /// Stops accepting passes. Work already running is left to its own
    /// cancellation token; anything still pending is discarded because the panel
    /// is going away.
    /// </summary>
    public void Stop()
    {
        lock (_sync)
        {
            _stopped = true;
            _pending = false;
        }
    }

    private Task Start()
    {
        var drain = DrainAsync();
        lock (_sync) _drain = drain;
        return drain;
    }

    private async Task DrainAsync()
    {
        while (true)
        {
            lock (_sync)
            {
                if (_stopped || !_pending)
                {
                    _draining = false;
                    return;
                }
                _pending = false;
            }
            try
            {
                await _pass();
            }
            catch (Exception error)
            {
                // The pass reports its own failures. This only keeps an unexpected
                // throw from ending the loop and losing requests that follow it.
                Trace.WriteLine(error);
            }
        }
    }
}
