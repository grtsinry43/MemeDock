using System.Diagnostics;
using MemeDock.Bridge;
using Microsoft.UI.Dispatching;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Services;

// All clipboard access is dispatched to the window's UI thread; core writes are serialized.
internal sealed class ClipboardGuard : IAsyncDisposable
{
    private const string Marker = "application/x-memedock-clipboard-reference";
    private readonly LibrarySession _session;
    private readonly DispatcherQueue _dispatcher = DispatcherQueue.GetForCurrentThread();
    private readonly SemaphoreSlim _gate = new(1);
    private PreparedSticker? _entry;
    private string? _entryReference;
    private bool _needsFlush;
    private int _flushAttempts;
    private bool _foreground;
    private bool _stopping;
    private bool _dirty;
    private bool _publishing;
    private long _version;
    private Task _observing = Task.CompletedTask;

    public ClipboardGuard(LibrarySession session)
    {
        _session = session;
        Clipboard.ContentChanged += OnContentChanged;
    }

    public void SetForeground(bool foreground)
    {
        _foreground = foreground;
        if (foreground) QueueObservation();
    }

    // SetContentWithOptions is the publication boundary. A persistence retry must
    // neither turn an accepted copy into a failed copy nor abort its durable pin.
    public async Task InstallAsync(PreparedSticker prepared, DataPackage package, CancellationToken cancellation)
    {
        await _gate.WaitAsync(cancellation);
        try
        {
            cancellation.ThrowIfCancellationRequested();
            if (_stopping) throw new OperationCanceledException(cancellation);
            _publishing = true;
            var reference = await _session.ProtectClipboardAsync(prepared.Lease, cancellation);
            try
            {
                cancellation.ThrowIfCancellationRequested();
                if (_stopping) throw new OperationCanceledException(cancellation);
                package.SetData(Marker, _session.LibraryId + ":" + reference);
                var accepted = Clipboard.SetContentWithOptions(package, new ClipboardContentOptions
                {
                    IsAllowedInHistory = false, IsRoamable = false
                });
                if (!accepted) throw new BridgeException.Failure(ErrorCode.Busy, "Windows clipboard is busy.");
            }
            catch
            {
                // A cancelled/failed publication must not replace the previous protection.
                try { await _session.AbortClipboardAsync(reference); }
                catch (Exception error) { Trace.WriteLine($"Abort clipboard protection: {error}"); }
                throw;
            }
            var previous = _entry;
            _entry = prepared;
            _entryReference = reference;
            _needsFlush = true;
            _flushAttempts = 0;
            previous?.Dispose();
            TryFlush();
        }
        finally { _publishing = false; _gate.Release(); QueueObservation(); }
    }

    private void TryFlush()
    {
        if (!_needsFlush || _flushAttempts >= 3) return;
        _flushAttempts++;
        try { Clipboard.Flush(); _needsFlush = false; }
        catch (Exception error)
        {
            Trace.WriteLine($"Clipboard already published; persistence pending (HRESULT 0x{error.HResult:X8}): {error}");
        }
    }

    private void OnContentChanged(object? sender, object args) => _dispatcher.TryEnqueue(() =>
    {
        _version++;
        QueueObservation();
    });

    private void QueueObservation()
    {
        if (_stopping || !_foreground) return;
        _dirty = true;
        if (_observing.IsCompleted) _observing = ObserveAsync();
    }

    private async Task ObserveAsync()
    {
        while (_dirty && !_stopping && _foreground)
        {
            _dirty = false;
            await _gate.WaitAsync();
            try
            {
                if (_stopping || !_foreground || _publishing) continue;
                var version = _version;
                var view = Clipboard.GetContent();
                string? reference = null;
                if (view.Contains(Marker))
                {
                    if (await view.GetDataAsync(Marker) is not string marker) continue;
                    var prefix = _session.LibraryId + ":";
                    if (marker.StartsWith(prefix, StringComparison.Ordinal))
                    {
                        reference = marker[prefix.Length..];
                        if (!Guid.TryParse(reference, out _)) continue;
                    }
                }
                if (_stopping || !_foreground || version != _version) { _dirty = true; continue; }
                // One observed retry; reserve the final attempt for a verified shutdown observation.
                if (_needsFlush && _flushAttempts < 2 && reference == _entryReference && _entry is not null) TryFlush();
                // Only a successful foreground observation can release persistent pins.
                await _session.ReconcileClipboardAsync(reference);
                if (reference is null)
                {
                    _entry?.Dispose();
                    _entry = null;
                    _entryReference = null;
                    _needsFlush = false;
                }
            }
            catch (Exception error) { Trace.WriteLine($"Observe clipboard (protection retained): {error}"); }
            finally { _gate.Release(); }
        }
    }

    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        Clipboard.ContentChanged -= OnContentChanged;
        await _observing;
        await _gate.WaitAsync();
        if (_needsFlush && _foreground && _entryReference is { } reference)
        {
            try
            {
                var view = Clipboard.GetContent();
                if (view.Contains(Marker) && await view.GetDataAsync(Marker) is string marker
                    && marker == _session.LibraryId + ":" + reference) TryFlush();
            }
            catch (Exception error) { Trace.WriteLine($"Observe clipboard before exit (protection retained): {error}"); }
        }
        _entry?.Dispose();
        _entry = null;
        // Durable clipboard protection deliberately survives application shutdown.
        _gate.Release();
    }
}
