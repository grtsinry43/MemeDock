using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;

namespace MemeDock.Windows.ViewModels;

/// <summary>
/// Deleted collections and tags. The core keeps soft-deleted metadata after a
/// restart, so every query here comes from the library rather than from an
/// in-memory undo slot.
/// </summary>
/// <remarks>
/// Two results are tracked apart: the core operation the user asked for, and
/// the catalog read that follows it. A failed refresh never turns a committed
/// restore or purge into an action failure, and a successful operation is the
/// only thing that can publish a success message.
/// </remarks>
/// <remarks>
/// <see cref="INotifyPropertyChanged"/> is listed again so this class owns the
/// event it notifies through: the inherited one can only be raised as a single
/// multicast delegate, which stops at the first subscriber that throws. See
/// <see cref="Notify"/>.
/// </remarks>
public sealed class TrashGroupsViewModel : ObservableObject, INotifyPropertyChanged, IAsyncDisposable
{
    private readonly ITrashCatalog _catalog;
    private readonly ICatalogStrings _strings;
    private readonly Func<Task> _reloadCatalogs;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly RefreshQueue _refreshes;
    private Task _operation = Task.CompletedTask;
    private bool _stopping;
    private bool _disposed;
    private bool _busy;
    private string? _operationErrorKey;
    private string? _refreshErrorKey;
    private string? _messageKey;
    private int _messageSucceeded;
    private int _messageFailed;

    /// <summary>
    /// Declared here, not inherited: only the declaring class may read the
    /// subscriber list of a field-like event, and the inherited multicast
    /// invocation stops at the first subscriber that throws. Interface-typed
    /// subscribers reach this event because <see cref="INotifyPropertyChanged"/>
    /// is implemented again on this class.
    /// </summary>
    public new event PropertyChangedEventHandler? PropertyChanged;

    public ObservableCollection<CollectionMetadata> Collections { get; } = [];
    public ObservableCollection<TagMetadata> Tags { get; } = [];
    public bool IsBusy => _busy;
    /// <summary>
    /// The bar's error. A business failure wins over a refresh failure; the
    /// refresh failure stays recorded underneath instead of overwriting it.
    /// </summary>
    public string? ErrorKey => TrashGroupsPresentation.VisibleErrorKey(_operationErrorKey, _refreshErrorKey);
    public string? MessageKey => _messageKey;
    public string ErrorText => ErrorKey is null ? "" : _strings[ErrorKey];
    public string MessageText => MessageKey switch
    {
        "purge_result" => _strings.Format("purge_result", _messageSucceeded, _messageFailed),
        null => "",
        _ => _strings[MessageKey!]
    };
    public bool HasAny => Collections.Count > 0 || Tags.Count > 0;
    public bool HasVisibleContent => HasAny || ErrorKey is not null || MessageKey is not null;
    public int TotalCount => Collections.Count + Tags.Count;

    internal TrashGroupsViewModel(ITrashCatalog catalog, ICatalogStrings strings, Func<Task> reloadCatalogs)
    {
        _catalog = catalog;
        _strings = strings;
        _reloadCatalogs = reloadCatalogs;
        _refreshes = new RefreshQueue(RefreshCoreAsync);
    }

    /// <summary>
    /// Asks for a catalog read. A request is never dropped: one that arrives
    /// during an operation or a running read is merged into the next read.
    /// </summary>
    public Task RefreshAsync() => _refreshes.Request();

    public Task<bool> RestoreCollectionAsync(CollectionMetadata collection) => RunAsync(async () =>
    {
        await _catalog.RestoreCollectionAsync(
            new EntityReference(collection.Id, collection.Lifecycle.Generation),
            collection.Lifecycle.Revision, _lifetime.Token);
        await _reloadCatalogs();
        return "restored";
    });

    public Task<bool> RestoreTagAsync(TagMetadata tag) => RunAsync(async () =>
    {
        await _catalog.RestoreTagAsync(
            new EntityReference(tag.Id, tag.Lifecycle.Generation),
            tag.Lifecycle.Revision, _lifetime.Token);
        await _reloadCatalogs();
        return "restored";
    });

    public Task<bool> PurgeCollectionAsync(CollectionMetadata collection) => PurgeAsync(
        () => _catalog.PurgeCollectionAsync(
            new EntityReference(collection.Id, collection.Lifecycle.Generation),
            collection.Lifecycle.Revision, _lifetime.Token));

    public Task<bool> PurgeTagAsync(TagMetadata tag) => PurgeAsync(
        () => _catalog.PurgeTagAsync(
            new EntityReference(tag.Id, tag.Lifecycle.Generation),
            tag.Lifecycle.Revision, _lifetime.Token));

    private Task<bool> PurgeAsync(Func<Task> operation) => RunAsync(async () =>
    {
        await operation();
        return "purge_result";
    });

    /// <summary>
    /// Runs one business operation. Refreshes are held back while it owns the
    /// catalog, so a read taken before the change cannot publish a stale list,
    /// and <see cref="RefreshQueue.Resume"/> runs one read afterwards covering
    /// every notification that arrived in the meantime.
    /// </summary>
    private async Task<bool> RunAsync(Func<Task<string>> operation)
    {
        if (_stopping || _busy) return false;
        _busy = true;
        // A new operation must not inherit the previous action's banner.
        _operationErrorKey = null;
        _messageKey = null;
        NotifyBars();
        _refreshes.Suspend();
        var task = RunCoreAsync(operation);
        _operation = task;
        try
        {
            return await task;
        }
        finally
        {
            _busy = false;
            if (!_stopping) NotifyBars();
        }
    }

    private async Task<bool> RunCoreAsync(Func<Task<string>> operation)
    {
        var succeeded = false;
        string? message = null;
        try
        {
            try
            {
                message = await operation();
                succeeded = true;
            }
            catch (OperationCanceledException) when (_lifetime.IsCancellationRequested)
            {
                // Shutdown cancelled the call; a closed panel reports nothing.
            }
            catch (Exception error)
            {
                Trace.WriteLine(error);
                if (!_stopping) _operationErrorKey = _strings.ErrorKey(error);
            }
            // Only a core success may announce success, and only while the panel is
            // still open.
            if (!_stopping && TrashGroupsPresentation.ShowsMessage(succeeded, message))
            {
                _messageKey = message;
                _messageSucceeded = 1;
                _messageFailed = 0;
                NotifyBars();
            }
        }
        finally
        {
            // The pass held back by Suspend has to run on every path, including a
            // throw from the success message above: a queue left suspended would
            // never reflect the catalog again. A committed change also has to be
            // reflected even when no notification arrived. On shutdown the queue
            // is already stopped, so this is a no-op instead of a late read of a
            // released session.
            //
            // RefreshQueue traps every failure of a pass and only traces it, so
            // this await cannot fault and mask the operation's own outcome; the
            // guard keeps that assumption from turning into a silently swallowed
            // exception if the queue changes.
            try
            {
                await _refreshes.Resume();
            }
            catch (Exception error)
            {
                Trace.WriteLine(error);
            }
        }
        return succeeded;
    }

    /// <summary>
    /// One reflection of the deleted catalog. Passes are serialized by
    /// <see cref="RefreshQueue"/>, so a pass can never be overtaken by a newer
    /// one, and the published list is always the last read.
    /// </summary>
    private async Task<bool> RefreshCoreAsync()
    {
        if (_stopping) return false;
        try
        {
            var collections = await _catalog.CollectionsAsync(_lifetime.Token, deleted: true);
            var tags = await _catalog.TagsAsync(_lifetime.Token, deleted: true);
            // Re-check across the awaits: the window may have started closing.
            if (_stopping) return false;
            Collections.Clear();
            foreach (var collection in collections) Collections.Add(collection);
            Tags.Clear();
            foreach (var tag in tags) Tags.Add(tag);
            // A successful read is what clears a stale list error.
            _refreshErrorKey = null;
            NotifyBars();
            return true;
        }
        catch (OperationCanceledException) when (_lifetime.IsCancellationRequested)
        {
            return false;
        }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            if (!_stopping)
            {
                // The real failure is recorded; the bar shows the existing
                // "collections and tags failed to load" copy for it.
                _refreshErrorKey = _strings.ErrorKey(error);
                NotifyBars();
            }
            return false;
        }
    }

    private void NotifyBars()
    {
        Notify(nameof(IsBusy), nameof(ErrorText), nameof(MessageText), nameof(HasAny),
            nameof(HasVisibleContent), nameof(TotalCount));
    }

    /// <summary>
    /// Raises the given notifications while dispatching one subscriber at a
    /// time, so a view that throws neither stops the other views from updating
    /// nor reaches the operation or the refresh queue: the business change is
    /// already committed when a banner is published, so a notification failure
    /// is traced, never reported as an operation or refresh error.
    /// </summary>
    /// <remarks>
    /// The subscriber list is snapshotted per notification, which also keeps a
    /// handler that subscribes or unsubscribes while being notified from
    /// changing the dispatch.
    /// </remarks>
    private void Notify(params string?[] names)
    {
        if (PropertyChanged is not { } handlers) return;
        var subscribers = handlers.GetInvocationList();
        foreach (var name in names)
        {
            var args = new PropertyChangedEventArgs(name);
            foreach (var subscriber in subscribers)
            {
                try
                {
                    ((PropertyChangedEventHandler)subscriber)(this, args);
                }
                catch (Exception error)
                {
                    Trace.WriteLine(error);
                }
            }
        }
    }

    /// <summary>
    /// Anything the toolkit raises on this class (property setters, for example)
    /// goes through the same isolated dispatch instead of the base event.
    /// </summary>
    protected override void OnPropertyChanged(PropertyChangedEventArgs e) => Notify(e.PropertyName);

    public void DismissMessage()
    {
        if (_messageKey is null) return;
        _messageKey = null;
        NotifyBars();
    }

    public void DismissError()
    {
        if (_operationErrorKey is null && _refreshErrorKey is null) return;
        _operationErrorKey = null;
        _refreshErrorKey = null;
        NotifyBars();
    }

    public void Relocalize() => Notify(nameof(ErrorText), nameof(MessageText));

    /// <summary>
    /// Asks the panel to stop: drops queued reads and cancels the ones in
    /// flight. Separate from <see cref="DisposeAsync"/> so the window can cancel
    /// this work as soon as it starts closing, without waiting for the rest of
    /// the shutdown sequence.
    /// </summary>
    public void RequestStop()
    {
        if (_stopping) return;
        _stopping = true;
        // Stop first: no new pass may start while cancellation unwinds.
        _refreshes.Stop();
        _lifetime.Cancel();
    }

    /// <summary>
    /// Waits for the work <see cref="RequestStop"/> cancelled, then releases the
    /// token source. The operation and the queue's last pass are both tracked,
    /// so nothing is still using the catalog when the session is released.
    /// </summary>
    public async ValueTask DisposeAsync()
    {
        if (_disposed) return;
        _disposed = true;
        RequestStop();
        try
        {
            await Task.WhenAll(_operation, _refreshes.Completion);
        }
        finally
        {
            _lifetime.Dispose();
        }
    }
}
