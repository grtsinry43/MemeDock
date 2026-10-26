using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Runtime.ExceptionServices;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

public sealed class LibraryViewModel(LocalizationService strings) : ObservableObject, IAsyncDisposable
{
    public ObservableCollection<StickerCard> Cards { get; } = [];
    public ObservableCollection<CollectionMetadata> Collections { get; } = [];
    public ObservableCollection<TagMetadata> Tags { get; } = [];
    public LibraryFilter Filter { get; private set; } = LibraryFilter.All;
    public StickerSort Sort { get; private set; } = StickerSort.Recent;
    public string SearchText { get; private set; } = "";
    public bool IsReady { get; private set; }
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
    public StickerDetailViewModel? Details { get; private set; }
    public BatchOrganizeViewModel? Batch { get; private set; }
    public CatalogViewModel? Catalog { get; private set; }
    public StickerOutputViewModel? Output { get; private set; }
    public SyncCoordinator? Sync { get; private set; }
    internal ThumbnailLoader? Thumbnails { get; private set; }

    private LibrarySession? _session;
    private QueryCursorHandle? _cursor;
    private SubscriptionHandle? _subscription;
    private readonly CancellationTokenSource _lifetime = new();
    private CancellationTokenSource? _queryCancellation;
    private readonly SemaphoreSlim _queries = new(1);
    private readonly BrowseRequestState _requests = new();
    private Task _observer = Task.CompletedTask;
    private Task _reload = Task.CompletedTask;
    private Task _trashBatch = Task.CompletedTask;
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
        Output?.Relocalize();
        foreach (var property in new[] { nameof(Heading), nameof(EmptyHeading), nameof(CountText), nameof(SelectionText),
            nameof(ErrorText), nameof(CatalogErrorText), nameof(ImportText), nameof(IsTrash) }) OnPropertyChanged(property);
    }

    public async Task InitializeAsync(string root)
    {
        try
        {
            _session ??= await LibrarySession.OpenAsync(root);
            if (_stopping) return;
            Thumbnails = new ThumbnailLoader(_session);
            Details = new StickerDetailViewModel(_session, strings);
            Batch = new BatchOrganizeViewModel(_session, strings);
            Catalog = new CatalogViewModel(_session, strings, RefreshCatalogsAsync);
            Output = new StickerOutputViewModel(_session, strings);
            Sync ??= new SyncCoordinator(
                new LibrarySyncAdapter(_session), new WindowsCredentialTokenStore(), _session.LibraryId);
            _subscription = _session.Subscribe();
            IsReady = true;
            OnPropertyChanged(nameof(IsReady));
            _observer = ObserveAsync();
            await Sync.StartAsync();
            await RefreshCatalogsAsync();
            await RefreshAsync();
        }
        catch (Exception error) { Report(error); }
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
                    if (notification is Notification.CollectionsChanged or Notification.TagsChanged or Notification.ReloadRequired)
                        await RefreshCatalogsAsync();
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
                if (IsImporting || IsTrashing || Batch?.IsRunning == true) { _dirty = true; continue; }
                await RefreshAsync(preserveLoaded: true);
            }
        }
        catch (OperationCanceledException) { }
    }

    public async Task ImportAsync(IReadOnlyList<string> paths)
    {
        if (_session is null || _stopping || IsImporting || paths.Count == 0) return;
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
        var failure = await ShutdownRun.SequenceAsync(() => _observer, () => _reload, () => _trashBatch);
        var held = false;
        try
        {
            // In-flight queries finish (their token is already cancelled) before
            // the handles and the session they use are released.
            await _queries.WaitAsync();
            held = true;
            // Assigned through a local: ??= would skip the whole cleanup sequence
            // when a background task already failed.
            var cleanup = await ShutdownRun.SequenceAsync(
                () => Sync is null ? Task.CompletedTask : Sync.DisposeAsync().AsTask(),
                () => Output is null ? Task.CompletedTask : Output.DisposeAsync().AsTask(),
                () => Details is null ? Task.CompletedTask : Details.DisposeAsync().AsTask(),
                () => Batch is null ? Task.CompletedTask : Batch.DisposeAsync().AsTask(),
                () => Catalog is null ? Task.CompletedTask : Catalog.DisposeAsync().AsTask(),
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
