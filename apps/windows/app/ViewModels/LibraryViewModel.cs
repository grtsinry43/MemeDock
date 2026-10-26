using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Runtime.ExceptionServices;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

public sealed class LibraryViewModel(LocalizationService strings, IOutputDefaults outputDefaults)
    : ObservableObject, IAsyncDisposable, IRestoreLifecycle
{
    public ObservableCollection<StickerCard> Cards { get; } = [];
    public ObservableCollection<CollectionMetadata> Collections { get; } = [];
    public ObservableCollection<TagMetadata> Tags { get; } = [];
    public LibraryFilter Filter { get; private set; } = LibraryFilter.All;
    public StickerSort Sort { get; private set; } = StickerSort.Recent;
    public string SearchText { get; private set; } = "";
    public bool IsReady { get; private set; }
    public LibraryIdentity? Identity { get; private set; }
    /// <summary>
    /// True while a restore owns the library. New conflicting work is refused instead of
    /// being allowed to race the restore, and the window disables actions that need the
    /// session a replace is about to close.
    /// </summary>
    public bool IsRestoring { get; private set; }
    public bool IsLoading { get; private set; }
    public bool HasMore { get; private set; }
    public string? ErrorKey { get; private set; }
    public string? CatalogErrorKey { get; private set; }
    public bool IsImporting { get; private set; }
    public bool IsTrashing { get; private set; }
    public bool IsTrash => Filter.IsTrash;
    public string? ImportMessageKey { get; private set; }
    public int ImportCompleted { get; private set; }
    public int ImportTotal { get; private set; }
    private int _importCreated;
    private int _importExisting;
    public int SelectedCount { get; private set; }
    public event EventHandler? CatalogChanged;
    public event EventHandler<string>? ThumbnailInvalidated;
    public event EventHandler<string?>? ContentChanged;
    /// <summary>
    /// Raised after a replace opened a new session and rebound every consumer. The window
    /// has to wire itself to the new child view models; nothing from the old session is
    /// reused.
    /// </summary>
    public event EventHandler? Rebuilt;
    public StickerDetailViewModel? Details { get; private set; }
    public BatchOrganizeViewModel? Batch { get; private set; }
    public CatalogViewModel? Catalog { get; private set; }
    public TrashGroupsViewModel? TrashGroups { get; private set; }
    public StickerOutputViewModel? Output { get; private set; }
    // The backup and storage view models stay internal with the services they wrap; the window
    // passes them on inside this assembly, so they are not part of the page's public surface.
    internal StorageViewModel? Storage { get; private set; }
    internal BackupViewModel? Backup { get; private set; }
    public SyncCoordinator? Sync { get; private set; }
    internal ThumbnailLoader? Thumbnails { get; private set; }

    private LibrarySession? _session;
    private string? _root;
    private IBackupDestinationPicker? _backupDestinationPicker;
    private IArchiveSourcePicker? _archiveSourcePicker;
    private QueryCursorHandle? _cursor;
    private SubscriptionHandle? _subscription;
    private readonly CancellationTokenSource _lifetime = new();
    private CancellationTokenSource? _queryCancellation;
    private readonly SemaphoreSlim _queries = new(1);
    private readonly BrowseRequestState _requests = new();
    private Task _observer = Task.CompletedTask;
    private Task _reload = Task.CompletedTask;
    private Task _trashBatch = Task.CompletedTask;
    private SyncCoordinator? _pausedSync;
    private bool _syncWasPaused;
    private bool _dirty;
    private bool _stopping;
    private bool _disposed;

    public string Heading => Filter.Scope switch
    {
        LibraryScope.Starred => strings["favorites_only"],
        LibraryScope.Recent => strings["filter_recent"],
        LibraryScope.Trash => strings["trash"],
        LibraryScope.Collection or LibraryScope.Tag => Filter.Name ?? "",
        _ => strings["filter_all"]
    };
    public string EmptyHeading => strings[SearchText.Length > 0 ? "library_no_matches" : Filter.Scope switch
    {
        LibraryScope.Starred => "starred_empty",
        LibraryScope.Trash => "trash_empty",
        LibraryScope.Collection => "collection_empty",
        LibraryScope.Tag => "tag_empty",
        _ => "library_empty"
    }];
    public string CountText => strings.Format("collection_count", Cards.Count);
    public string SelectionText => strings.Format("selection_count", SelectedCount);
    public string ErrorText => ErrorKey is null ? "" : strings[ErrorKey];
    public string CatalogErrorText => CatalogErrorKey is null ? "" : strings[CatalogErrorKey];
    public string ImportText => ImportMessageKey switch
    {
        "import_running_progress" => strings.Format(ImportMessageKey, ImportCompleted, ImportTotal),
        "import_done" => strings.Format(ImportMessageKey, _importCreated),
        "import_done_reused" => strings.Format(ImportMessageKey, _importCreated, _importExisting),
        null => "",
        _ => strings[ImportMessageKey]
    };

    public void Relocalize()
    {
        Details?.Relocalize();
        Batch?.Relocalize();
        Catalog?.Relocalize();
        TrashGroups?.Relocalize();
        Output?.Relocalize();
        foreach (var property in new[] { nameof(Heading), nameof(EmptyHeading), nameof(CountText), nameof(SelectionText),
            nameof(ErrorText), nameof(CatalogErrorText), nameof(ImportText), nameof(IsTrash) }) OnPropertyChanged(property);
    }

    // The pickers are internal contracts, so the entry point that takes them is internal too;
    // the only caller is MainWindow in this assembly.
    internal async Task InitializeAsync(string root, IBackupDestinationPicker backupDestinationPicker,
        IArchiveSourcePicker archiveSourcePicker)
    {
        _root = root;
        _backupDestinationPicker = backupDestinationPicker;
        _archiveSourcePicker = archiveSourcePicker;
        try
        {
            _session ??= await LibrarySession.OpenAsync(root);
            Identity = _session.Identity;
            if (_stopping) return;
            CreateChildren();
            _subscription = _session.Subscribe();
            IsReady = true;
            OnPropertyChanged(nameof(IsReady));
            _observer = ObserveAsync();
            await Sync!.StartAsync();
            await RefreshCatalogsAsync();
            if (TrashGroups is { } trashGroups) await trashGroups.RefreshAsync();
            await RefreshAsync();
        }
        catch (Exception error) { Report(error); }
    }

    /// <summary>
    /// Binds every child view model to the current session. A rebuild runs this again
    /// instead of trying to refresh objects that kept a reference to a closed library.
    /// </summary>
    private void CreateChildren()
    {
        var session = _session!;
        Thumbnails = new ThumbnailLoader(session);
        Details = new StickerDetailViewModel(session, strings);
        Batch = new BatchOrganizeViewModel(session, strings);
        Catalog = new CatalogViewModel(session, strings, RefreshCatalogsAsync);
        TrashGroups ??= new TrashGroupsViewModel(session, strings, RefreshCatalogsAsync);
        Output = new StickerOutputViewModel(session, strings, outputDefaults);
        Storage ??= new StorageViewModel(session, strings);
        Backup ??= new BackupViewModel(session, session, _backupDestinationPicker!, _archiveSourcePicker!,
            this, strings);
        Sync ??= new SyncCoordinator(
            new LibrarySyncAdapter(session), new WindowsCredentialTokenStore(), session.LibraryId);
    }

    // ---- Restore lifecycle ----

    async Task<bool> IRestoreLifecycle.TryBeginRestoreAsync()
    {
        if (_session is null || _stopping || IsRestoring) return false;
        if (IsImporting || IsTrashing) return false;
        if (Batch is { IsRunning: true } || Catalog is { IsBusy: true } || TrashGroups is { IsBusy: true }) return false;
        if (Output is { IsBusy: true } || Details is { IsSaving: true } || Storage is { IsCleaning: true }) return false;

        IsRestoring = true;
        OnPropertyChanged(nameof(IsRestoring));
        _requests.Invalidate();
        _queryCancellation?.Cancel();

        // Remote apply writes to this same library, so it is paused rather than killed:
        // stopping the coordinator is final and a merge keeps the session, which would
        // silently leave the user without sync. A replace disposes this coordinator during
        // the rebuild; replacing it then means there is nothing to resume here.
        if (Sync is { } sync && sync.State.Loaded)
        {
            _pausedSync = sync;
            _syncWasPaused = sync.State.Status.Paused;
            if (!_syncWasPaused)
            {
                try
                {
                    await sync.SetPausedAsync(true);
                }
                catch (Exception error)
                {
                    // Sync being unable to pause must not turn a local restore into a
                    // refusal; the restore is still serialized by the core.
                    Trace.WriteLine($"Pausing sync before a restore failed: {error.GetType().Name}");
                }
            }
        }

        return true;
    }

    async Task IRestoreLifecycle.EndRestoreAsync()
    {
        var paused = _pausedSync;
        _pausedSync = null;
        if (paused is not null && !_syncWasPaused && ReferenceEquals(paused, Sync))
        {
            try
            {
                await paused.SetPausedAsync(false);
            }
            catch (Exception error)
            {
                // The library is usable even if the remote side stays paused; the sync
                // page shows the real state instead of hiding it.
                Trace.WriteLine($"Resuming sync after a restore failed: {error.GetType().Name}");
            }
        }

        if (!IsRestoring) return;
        IsRestoring = false;
        OnPropertyChanged(nameof(IsRestoring));
    }

    async Task<bool> IRestoreLifecycle.ReopenAsync(CancellationToken cancellation)
    {
        if (_root is null) throw new InvalidOperationException("the library was never opened");
        LibrarySession fresh;
        try
        {
            fresh = await LibrarySession.OpenAsync(_root);
        }
        catch (BridgeException error) when (error is BridgeException.Failure
            { code: ErrorCode.AlreadyOpen or ErrorCode.Busy })
        {
            // The core still owns the previous handle, so nothing closed this library and
            // the current session remains the live one.
            return false;
        }
        catch
        {
            // The core closed the old session during the replace and the new one cannot
            // be opened. Nothing in this page can work against a closed handle, so it
            // reports that instead of letting every later action fail on its own.
            if (IsReady)
            {
                IsReady = false;
                OnPropertyChanged(nameof(IsReady));
            }

            throw;
        }

        try
        {
            await AdoptAsync(fresh);
        }
        catch
        {
            // AdoptAsync takes ownership of the session when it assigns it; until then the
            // fresh handle would be orphaned, so it is closed here.
            if (!ReferenceEquals(_session, fresh))
            {
                try { await fresh.DisposeAsync(); } catch (BridgeException) { }
            }

            if (IsReady)
            {
                IsReady = false;
                OnPropertyChanged(nameof(IsReady));
            }

            throw;
        }

        return true;
    }

    Task IRestoreLifecycle.RefreshAsync() => RefreshAfterRestoreAsync();

    /// <summary>
    /// Reloads everything a merge changed. The session survives a merge, so the child
    /// view models are kept and simply told to re-read the library.
    /// </summary>
    private async Task RefreshAfterRestoreAsync()
    {
        if (_session is null) return;
        Identity = _session.Identity;
        OnPropertyChanged(nameof(Identity));
        await RefreshCatalogsAsync();
        if (TrashGroups is { } trashGroups) await trashGroups.RefreshAsync();
        if (Storage is { } storage) await storage.RefreshAsync();
        await RefreshAsync();
        // The catalog labels carry counts, so they are refreshed with the contents.
        Relocalize();
    }

    /// <summary>
    /// Takes over a freshly opened session after a replace closed the previous one. Every
    /// consumer is stopped, released and rebuilt: the old view models, cursors and
    /// subscription all point at a handle the core already closed.
    /// </summary>
    private async Task AdoptAsync(LibrarySession fresh)
    {
        var previous = _session;

        // Invalidate first: a browse or observer continuation that wakes up from here on
        // belongs to the old session and must not publish into the new page.
        _requests.Invalidate();
        _queryCancellation?.Cancel();
        // Let a running query observe the cancellation before its handle is released. The
        // wait has to observe shutdown: DisposeAsync holds this same gate while it waits
        // for this rebuild to finish, so waiting forever here would keep the window open
        // for good. Cancelling now is safe because nothing was torn down yet.
        await _queries.WaitAsync(_lifetime.Token);
        _queries.Release();

        // Until every consumer points at the new session the page must not accept work.
        if (IsReady)
        {
            IsReady = false;
            OnPropertyChanged(nameof(IsReady));
        }

        TrashGroups?.RequestStop();
        Storage?.RequestStop();
        await ShutdownRun.SequenceAsync(
            () => Sync is null ? Task.CompletedTask : Sync.DisposeAsync().AsTask(),
            () => Output is null ? Task.CompletedTask : Output.DisposeAsync().AsTask(),
            () => Details is null ? Task.CompletedTask : Details.DisposeAsync().AsTask(),
            () => Batch is null ? Task.CompletedTask : Batch.DisposeAsync().AsTask(),
            () => Catalog is null ? Task.CompletedTask : Catalog.DisposeAsync().AsTask(),
            () => TrashGroups is null ? Task.CompletedTask : TrashGroups.DisposeAsync().AsTask(),
            () => Storage is null ? Task.CompletedTask : Storage.DisposeAsync().AsTask(),
            () => Thumbnails is null ? Task.CompletedTask : Thumbnails.DisposeAsync().AsTask(),
            ReleaseHandles,
            () => _observer);

        Sync = null;
        Output = null;
        Details = null;
        Batch = null;
        Catalog = null;
        TrashGroups = null;
        Storage = null;
        Thumbnails = null;
        _session = null;
        if (previous is not null)
        {
            // The core closed this library as part of the replace, so closing it again
            // only drops the handle; a second close is not an error worth surfacing.
            try { await previous.DisposeAsync(); } catch (BridgeException) { }
        }

        _session = fresh;
        Identity = fresh.Identity;
        ResetBrowseState();
        CreateChildren();
        // The restore itself runs from the backup view model, which is the one consumer
        // that cannot be rebuilt from under its own call; it keeps its wording and only
        // gets the new session.
        Backup?.Adopt(fresh, fresh);
        _subscription = fresh.Subscribe();
        IsReady = true;
        OnPropertyChanged(nameof(IsReady));
        _observer = ObserveAsync();
        await Sync!.StartAsync();
        await RefreshCatalogsAsync();
        if (TrashGroups is { } trashGroups) await trashGroups.RefreshAsync();
        if (Storage is { } storage) await storage.RefreshAsync();
        await RefreshAsync();
        Relocalize();
        // The replace already committed; a UI handler that fails while re-binding must not
        // be reported as a failed restore, so the notification is isolated from the adopt.
        try { Rebuilt?.Invoke(this, EventArgs.Empty); }
        catch (Exception error) { Trace.WriteLine($"Rebuilt handler failed: {error}"); }
    }

    private void ResetBrowseState()
    {
        _cursor?.Dispose();
        _cursor = null;
        Cards.Clear();
        Collections.Clear();
        Tags.Clear();
        Filter = LibraryFilter.All;
        SearchText = "";
        Sort = StickerSort.Recent;
        HasMore = false;
        ErrorKey = null;
        CatalogErrorKey = null;
        IsLoading = false;
        _dirty = false;
        SetSelectedCount(0);
    }

    public async Task NavigateAsync(LibraryFilter filter)
    {
        Filter = filter;
        SetSelectedCount(0);
        if (!filter.AllowsCollectionOrder && Sort == StickerSort.CollectionOrder) Sort = StickerSort.Recent;
        Relocalize();
        await RefreshAsync();
    }

    public async Task SearchAsync(string text)
    {
        SearchText = text.Trim();
        Relocalize();
        await RefreshAsync();
    }

    public async Task SortAsync(StickerSort sort)
    {
        Sort = sort;
        await RefreshAsync();
    }

    public void SetSelectedCount(int count)
    {
        SelectedCount = count;
        OnPropertyChanged(nameof(SelectedCount));
        OnPropertyChanged(nameof(SelectionText));
    }

    public async Task RefreshAsync(bool preserveLoaded = false)
    {
        if (_session is null || _stopping) return;
        var generation = _requests.Begin();
        _queryCancellation?.Cancel();
        var filter = Filter;
        var search = SearchText;
        var sort = Sort;
        var target = preserveLoaded ? Math.Max(1, Cards.Count) : 1;
        try
        {
            await _queries.WaitAsync(_lifetime.Token);
            try
            {
                if (!_requests.IsCurrent(generation) || _stopping) return;
                _queryCancellation?.Dispose();
                _queryCancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
                var cancellation = _queryCancellation.Token;
                _cursor?.Dispose();
                _cursor = null;
                HasMore = false;
                IsLoading = true;
                ErrorKey = null;
                NotifyState();
                // Keep the old viewport usable until the replacement query succeeds.
                var items = new List<(StickerMetadata Metadata, StickerResource? Resource)>();
                QueryCursorHandle? next = null;
                try
                {
                    do
                    {
                        var page = await _session.QueryAsync(filter, search, sort, next, cancellation);
                        next?.Dispose();
                        next = page.Next;
                        cancellation.ThrowIfCancellationRequested();
                        if (!_requests.IsCurrent(generation)) return;
                        var resources = page.Resources.ToDictionary(resource => resource.Asset.Hash);
                        items.AddRange(page.Stickers.Select(metadata =>
                            (metadata, resources.GetValueOrDefault(metadata.Id))));
                    } while (items.Count < target && next is not null);
                    _cursor?.Dispose();
                    _cursor = next;
                    next = null;
                    HasMore = _cursor is not null;
                    Reconcile(items);
                }
                finally { next?.Dispose(); }
            }
            finally
            {
                if (_requests.IsCurrent(generation)) { IsLoading = false; NotifyState(); }
                _queries.Release();
            }
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { if (_requests.IsCurrent(generation) && !_stopping) Report(error); }
    }

    public async Task LoadNextAsync()
    {
        if (_session is null || _stopping || IsLoading || !HasMore || _cursor is null) return;
        var generation = _requests.Current;
        await _queries.WaitAsync();
        try
        {
            if (_stopping || !_requests.IsCurrent(generation) || _cursor is null || _queryCancellation is null) return;
            IsLoading = true;
            ErrorKey = null;
            NotifyState();
            var page = await _session.QueryAsync(Filter, SearchText, Sort, _cursor, _queryCancellation.Token);
            if (_stopping || !_requests.IsCurrent(generation) || _queryCancellation.IsCancellationRequested)
            {
                page.Next?.Dispose();
                return;
            }
            _cursor.Dispose();
            _cursor = page.Next;
            HasMore = _cursor is not null;
            var resources = page.Resources.ToDictionary(resource => resource.Asset.Hash);
            var existing = Cards.Select(card => card.Id).ToHashSet();
            foreach (var metadata in page.Stickers)
                if (existing.Add(metadata.Id)) Cards.Add(new StickerCard(metadata, resources.GetValueOrDefault(metadata.Id)));
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { if (!_stopping && _requests.IsCurrent(generation)) Report(error); }
        finally { IsLoading = false; NotifyState(); _queries.Release(); }
    }

    private void Reconcile(List<(StickerMetadata Metadata, StickerResource? Resource)> items)
    {
        var existing = Cards.ToDictionary(card => card.Id);
        for (var index = 0; index < items.Count; index++)
        {
            var (metadata, resource) = items[index];
            if (!existing.TryGetValue(metadata.Id, out var card)) card = new StickerCard(metadata, resource);
            else card.Update(metadata, resource);
            if (index < Cards.Count && Cards[index] == card) continue;
            var oldIndex = Cards.IndexOf(card);
            if (oldIndex >= 0) Cards.Move(oldIndex, index);
            else Cards.Insert(index, card);
        }
        while (Cards.Count > items.Count) Cards.RemoveAt(Cards.Count - 1);
    }

    private async Task RefreshCatalogsAsync()
    {
        if (_session is null || _stopping) return;
        try
        {
            var collections = await _session.CollectionsAsync(_lifetime.Token);
            var tags = await _session.TagsAsync(_lifetime.Token);
            if (_stopping) return;
            Collections.Clear();
            foreach (var collection in collections) Collections.Add(collection);
            Tags.Clear();
            foreach (var tag in tags) Tags.Add(tag);
            CatalogErrorKey = null;
            if (Filter.Scope is LibraryScope.Collection or LibraryScope.Tag)
            {
                var name = Filter.Scope == LibraryScope.Collection
                    ? collections.FirstOrDefault(item => item.Id == Filter.Id)?.Name
                    : tags.FirstOrDefault(item => item.Id == Filter.Id)?.Name;
                if (name is null) { await NavigateAsync(LibraryFilter.All); }
                else Filter = Filter with { Name = name };
            }
            CatalogChanged?.Invoke(this, EventArgs.Empty);
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { Trace.WriteLine(error); CatalogErrorKey = strings.ErrorKey(error); }
        OnPropertyChanged(nameof(CatalogErrorText));
        Relocalize();
    }

    public async Task RetryAsync()
    {
        if (!IsReady) return;
        if (CatalogErrorKey is not null) await RefreshCatalogsAsync();
        await RefreshAsync(preserveLoaded: true);
    }

    private async Task ObserveAsync()
    {
        try
        {
            while (!_stopping && _subscription is not null)
            {
                var subscription = _subscription;
                var notification = await Task.Run(() => subscription.Next());
                if (_stopping || notification is Notification.Closed) break;
                if (notification is Notification.ThumbnailChanged thumbnail)
                    ThumbnailInvalidated?.Invoke(this, thumbnail.StickerId);
                else
                {
                    ContentChanged?.Invoke(this, notification is Notification.StickerChanged sticker ? sticker.StickerId : null);
                    // A restore owns the library until it finishes: queries issued from here
                    // would read a half-written library (or a session the core just closed).
                    // Everything is refreshed from the restore's own exit path instead.
                    if (!IsRestoring
                        && notification is (Notification.CollectionsChanged or Notification.TagsChanged or Notification.ReloadRequired))
                    {
                        await RefreshCatalogsAsync();
                        if (TrashGroups is { } trashGroups) await trashGroups.RefreshAsync();
                    }
                    _dirty = true;
                    Sync?.NotifyContentChanged();
                    if (_reload.IsCompleted) _reload = ReloadWhenQuietAsync();
                }
            }
        }
        catch (Exception error) { if (!_stopping) Report(error); }
    }

    private async Task ReloadWhenQuietAsync()
    {
        try
        {
            while (_dirty && !_stopping)
            {
                _dirty = false;
                await Task.Delay(200, _lifetime.Token);
                if (!IsReady || IsRestoring || IsImporting || IsTrashing || Batch?.IsRunning == true)
                {
                    _dirty = true;
                    continue;
                }
                await RefreshAsync(preserveLoaded: true);
            }
        }
        catch (OperationCanceledException) { }
    }

    public async Task ImportAsync(IReadOnlyList<string> paths)
    {
        // IsReady is false while a replace rebuilds the page and after a failed adopt, so an
        // import cannot slip into a half-bound session even if it bypasses the disabled UI.
        if (!IsReady || _session is null || _stopping || IsImporting || paths.Count == 0) return;
        if (paths.Count > 200)
        {
            ImportMessageKey = "failure_batch";
            NotifyState();
            return;
        }
        IsImporting = true;
        ImportTotal = paths.Count;
        ImportCompleted = 0;
        _importCreated = _importExisting = 0;
        var collection = Filter.Scope == LibraryScope.Collection ? Filter.Id : null;
        try
        {
            for (var index = 0; index < paths.Count; index++)
            {
                ImportMessageKey = "import_running_progress";
                ImportCompleted = index + 1;
                NotifyState();
                var outcome = await _session.ImportAsync(paths[index], collection, _lifetime.Token);
                if (outcome.Status == ImportStatus.RestoreRequired)
                {
                    ImportMessageKey = "import_restore_required";
                    return;
                }
                if (outcome.Status == ImportStatus.Created) _importCreated++;
                else _importExisting++;
            }
            ImportMessageKey = _importExisting == 0 ? "import_done"
                : _importCreated == 0 ? "import_done_existing" : "import_done_reused";
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { Trace.WriteLine(error); ImportMessageKey = strings.ErrorKey(error); }
        finally
        {
            IsImporting = false;
            NotifyState();
            if (!_stopping) await RefreshAsync(preserveLoaded: true);
        }
    }

    internal Task<TrashOutcome> RestoreAsync(IReadOnlyList<StickerCard> cards) =>
        TrackTrashAsync(() => ApplyTrashAsync(cards, TrashOperations.RestoreAsync));

    internal Task<TrashOutcome> PurgeAsync(IReadOnlyList<StickerCard> cards) =>
        TrackTrashAsync(() => ApplyTrashAsync(cards, TrashOperations.PurgeAsync));

    /// <summary>
    /// Keeps the running batch reachable so <see cref="DisposeAsync"/> can wait
    /// for it before releasing the session. The inner guard runs first, which
    /// keeps a second click from replacing the tracked task.
    /// </summary>
    private async Task<TrashOutcome> TrackTrashAsync(Func<Task<TrashOutcome>> work)
    {
        if (_session is null || _stopping || IsTrashing) return TrashOutcome.Empty;
        var task = work();
        _trashBatch = task;
        return await task;
    }

    /// <summary>
    /// Applies a trash action one entity at a time, then reloads the current
    /// listing so restored stickers leave the trash and purged ones disappear
    /// from both listings. A failed entry keeps its place and its file.
    /// </summary>
    private async Task<TrashOutcome> ApplyTrashAsync(IReadOnlyList<StickerCard> cards,
        Func<LibrarySession, IReadOnlyList<TrashEntry>, Func<Exception, string>, CancellationToken, Task<TrashOutcome>> apply)
    {
        if (_session is null || _stopping || IsTrashing || cards.Count == 0) return TrashOutcome.Empty;
        var entries = cards.Select(card => new TrashEntry(card.Id, card.Generation, card.Revision)).ToArray();
        IsTrashing = true;
        ErrorKey = null;
        NotifyState();
        TrashOutcome outcome;
        try
        {
            outcome = await apply(_session, entries, strings.ErrorKey, _lifetime.Token);
        }
        catch (OperationCanceledException) { outcome = TrashOutcome.Empty; }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            outcome = new TrashOutcome(0, cards.Count, strings.ErrorKey(error));
        }
        finally
        {
            // The flag gates navigation and the selection bar, so it must fall
            // even when the native call throws something unexpected.
            IsTrashing = false;
        }
        if (!_stopping) await RefreshAsync(preserveLoaded: true);
        if (outcome.AnyFailed) ErrorKey = outcome.ErrorKey;
        NotifyState();
        return outcome;
    }

    private void Report(Exception error)
    {
        Trace.WriteLine(error);
        ErrorKey = strings.ErrorKey(error);
        NotifyState();
    }

    private void NotifyState()
    {
        foreach (var property in new[] { nameof(IsLoading), nameof(IsImporting), nameof(IsTrashing), nameof(HasMore),
            nameof(ErrorText), nameof(ImportText), nameof(CountText), nameof(EmptyHeading) }) OnPropertyChanged(property);
    }

    public void RequestStop()
    {
        _stopping = true;
        // The deleted catalog owns its own query lifetime, so it has to be
        // cancelled here: DisposeAsync only reaches it after the other children,
        // and a closing window must not keep a catalog query alive meanwhile.
        TrashGroups?.RequestStop();
        Storage?.RequestStop();
        // A backup in flight keeps its staged archive and the session it needs until
        // it finishes, so it is told to stop here and awaited in DisposeAsync.
        Backup?.RequestStop();
        _requests.Invalidate();
        _lifetime.Cancel();
        _queryCancellation?.Cancel();
        _subscription?.Unsubscribe();
        Sync?.RequestStop();
    }

    public async ValueTask DisposeAsync()
    {
        if (_disposed) return;
        _disposed = true;
        RequestStop();
        // Shutdown has to reach the session even when a background task or a
        // child view model throws: the window closes regardless, and skipping
        // the session would leave the native library (and its files) open.
        var held = false;
        var failure = await ShutdownRun.SequenceAsync(
            () => _observer,
            () => _reload,
            () => _trashBatch,
            async () =>
            {
                // In-flight queries finish (their token is already cancelled)
                // before the handles and the session they use are released. The
                // wait is a step like any other: if it fails, the cleanup below
                // still runs instead of leaving the library open.
                await _queries.WaitAsync();
                held = true;
            });
        try
        {
            // Assigned through a local: ??= would skip the whole cleanup sequence
            // when a background task already failed.
            var cleanup = await ShutdownRun.SequenceAsync(
                () => Sync is null ? Task.CompletedTask : Sync.DisposeAsync().AsTask(),
                () => Output is null ? Task.CompletedTask : Output.DisposeAsync().AsTask(),
                () => Details is null ? Task.CompletedTask : Details.DisposeAsync().AsTask(),
                () => Batch is null ? Task.CompletedTask : Batch.DisposeAsync().AsTask(),
                () => Catalog is null ? Task.CompletedTask : Catalog.DisposeAsync().AsTask(),
                () => TrashGroups is null ? Task.CompletedTask : TrashGroups.DisposeAsync().AsTask(),
                () => Storage is null ? Task.CompletedTask : Storage.DisposeAsync().AsTask(),
                // Must finish before the session closes: the staged archive is held
                // by the library and its cleanup goes through the library handle.
                () => Backup is null ? Task.CompletedTask : Backup.DisposeAsync().AsTask(),
                () => Thumbnails is null ? Task.CompletedTask : Thumbnails.DisposeAsync().AsTask(),
                ReleaseHandles,
                () => _session is null ? Task.CompletedTask : _session.DisposeAsync().AsTask());
            failure ??= cleanup;
            _session = null;
        }
        finally
        {
            if (held) _queries.Release();
            _queries.Dispose();
            _queryCancellation?.Dispose();
            _lifetime.Dispose();
        }
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }

    private Task ReleaseHandles()
    {
        _cursor?.Dispose();
        _cursor = null;
        _subscription?.Dispose();
        _subscription = null;
        return Task.CompletedTask;
    }
}
