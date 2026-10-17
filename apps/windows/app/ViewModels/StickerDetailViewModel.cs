using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;

namespace MemeDock.Windows.ViewModels;

public sealed class StickerDetailViewModel : ObservableObject, IAsyncDisposable
{
    private readonly LibrarySession _session;
    private readonly LocalizationService _strings;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly SemaphoreSlim _writes = new(1);
    private readonly BrowseRequestState _loads = new();
    private CancellationTokenSource? _loadCancellation;
    private Task _loading = Task.CompletedTask;
    private Task _refreshing = Task.CompletedTask;
    private StickerEditState? _edit;
    private bool _stopping;

    public StickerDetail? Detail { get; private set; }
    public StickerMetadata? LastDeleted { get; private set; }
    public string? CurrentId => Detail?.Sticker.Id;
    public bool IsLoading { get; private set; }
    public bool IsSaving { get; private set; }
    public bool IsDirty => _edit?.IsDirty == true;
    public string? ErrorKey { get; private set; }
    public string ErrorText => ErrorKey is null ? "" : _strings[ErrorKey];
    public string Title { get => _edit?.Title ?? ""; set { if (_edit is not null && !IsLoading) { _edit.Title = value; Notify(); } } }
    public string Note { get => _edit?.Note ?? ""; set { if (_edit is not null && !IsLoading) { _edit.Note = value; Notify(); } } }
    public event EventHandler? Deleted;

    internal StickerDetailViewModel(LibrarySession session, LocalizationService strings) => (_session, _strings) = (session, strings);
    public void Relocalize() => Notify();
    private void Notify() => OnPropertyChanged(string.Empty);

    public Task<bool> SelectAsync(string? id)
    {
        var work = SelectCoreAsync(id);
        _loading = work;
        return work;
    }

    private async Task<bool> SelectCoreAsync(string? id)
    {
        if (_stopping) return false;
        if (id == CurrentId && !IsLoading) return true;
        var generation = _loads.Begin();
        _loadCancellation?.Cancel();
        IsLoading = true;
        Notify();
        try
        {
            if (!await CommitAsync()) return !_loads.IsCurrent(generation);
            if (_stopping || !_loads.IsCurrent(generation)) return true;
            if (id is null)
            {
                Detail = null;
                _edit = null;
                ErrorKey = null;
                return true;
            }
            _loadCancellation?.Dispose();
            _loadCancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
            var detail = await _session.DetailAsync(id, _loadCancellation.Token);
            if (_stopping || !_loads.IsCurrent(generation)) return true;
            Detail = detail;
            _edit = new StickerEditState(detail.Sticker);
            ErrorKey = null;
            return true;
        }
        catch (OperationCanceledException) { return true; }
        catch (Exception error)
        {
            if (_loads.IsCurrent(generation) && !_stopping) Report(error);
            return false;
        }
        finally { if (_loads.IsCurrent(generation)) { IsLoading = false; Notify(); } }
    }

    public async Task<bool> CommitAsync()
    {
        if (_stopping) return false;
        var state = _edit;
        if (state is null || !state.IsDirty) return true;
        try
        {
            await _writes.WaitAsync(_lifetime.Token);
            try
            {
                if (!state.IsDirty) return true;
                var sent = state.Capture();
                IsSaving = true;
                ErrorKey = null;
                Notify();
                var saved = await _session.PatchAsync(sent.Target, sent.Edit, _lifetime.Token);
                if (!state.Accept(saved, sent)) throw new BridgeException.Failure(ErrorCode.Conflict, "Draft generation changed.");
                if (ReferenceEquals(state, _edit) && Detail is not null) Detail = Detail with { Sticker = saved };
                return !state.IsDirty;
            }
            finally { IsSaving = false; Notify(); _writes.Release(); }
        }
        catch (OperationCanceledException) { return false; }
        catch (Exception error) { if (!_stopping) Report(error); return false; }
    }

    public void Discard()
    {
        if (IsSaving) return;
        _edit?.Discard();
        ErrorKey = null;
        Notify();
    }

    public Task RefreshAsync()
    {
        if (!_refreshing.IsCompleted) return _refreshing;
        return _refreshing = RefreshCoreAsync();
    }

    private async Task RefreshCoreAsync()
    {
        if (_stopping || IsLoading || IsSaving || IsDirty || Detail is null) return;
        var id = Detail.Sticker.Id;
        var generation = _loads.Current;
        try
        {
            var fresh = await _session.DetailAsync(id, _lifetime.Token);
            if (!_loads.IsCurrent(generation) || IsDirty || IsSaving || _stopping
                || fresh.Sticker.Lifecycle.Revision < (Detail?.Sticker.Lifecycle.Revision ?? 0)) return;
            Detail = fresh;
            _edit = new StickerEditState(fresh.Sticker);
            Notify();
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { if (!_stopping && _loads.IsCurrent(generation)) Report(error); }
    }

    public Task<bool> ToggleStarAsync() => ChangeAsync(async (detail, cancellation) =>
    {
        await _session.PatchAsync(Reference(detail.Sticker), new StickerEdit(null, null, !detail.Sticker.Starred), cancellation);
    });

    public Task<bool> AssignCollectionAsync(CollectionMetadata? collection) => ChangeAsync(async (detail, cancellation) =>
    {
        await _session.OrganizeAsync(Reference(detail.Sticker), true,
            collection is null ? null : new EntityReference(collection.Id, collection.Lifecycle.Generation), null, cancellation);
    });

    public Task<bool> AssignTagsAsync(TagMetadata[] tags) => ChangeAsync(async (detail, cancellation) =>
    {
        await _session.OrganizeAsync(Reference(detail.Sticker), false, null,
            tags.Select(tag => new EntityReference(tag.Id, tag.Lifecycle.Generation)).ToArray(), cancellation);
    });

    private async Task<bool> ChangeAsync(Func<StickerDetail, CancellationToken, Task> change)
    {
        var state = _edit;
        if (state is null || IsLoading || _stopping || !await CommitAsync()) return false;
        try
        {
            await _writes.WaitAsync(_lifetime.Token);
            try
            {
                if (!ReferenceEquals(state, _edit) || Detail is null) return false;
                var target = Detail;
                IsSaving = true;
                ErrorKey = null;
                Notify();
                await change(target, _lifetime.Token);
                var fresh = await _session.DetailAsync(target.Sticker.Id, _lifetime.Token);
                if (ReferenceEquals(state, _edit))
                {
                    var sent = new PendingStickerEdit(state.Target, new StickerEdit(null, null, null));
                    if (!state.Accept(fresh.Sticker, sent)) throw new BridgeException.Failure(ErrorCode.Conflict, "Detail generation changed.");
                    Detail = fresh;
                }
                return true;
            }
            finally { IsSaving = false; Notify(); _writes.Release(); }
        }
        catch (OperationCanceledException) { return false; }
        catch (Exception error) { if (!_stopping) Report(error); return false; }
    }

    public async Task<bool> DeleteAsync()
    {
        var state = _edit;
        if (state is null || IsLoading || _stopping || !await CommitAsync()) return false;
        try
        {
            await _writes.WaitAsync(_lifetime.Token);
            try
            {
                if (!ReferenceEquals(state, _edit)) return false;
                IsSaving = true;
                Notify();
                LastDeleted = await _session.DeleteStickerAsync(state.Target, _lifetime.Token);
                _loads.Invalidate();
                Detail = null;
                _edit = null;
                Deleted?.Invoke(this, EventArgs.Empty);
                return true;
            }
            finally { IsSaving = false; Notify(); _writes.Release(); }
        }
        catch (OperationCanceledException) { return false; }
        catch (Exception error) { if (!_stopping) Report(error); return false; }
    }

    public async Task<bool> UndoDeleteAsync()
    {
        if (LastDeleted is not { } deleted || IsSaving || _stopping) return false;
        try
        {
            await _writes.WaitAsync(_lifetime.Token);
            try
            {
                if (!ReferenceEquals(LastDeleted, deleted)) return false;
                IsSaving = true;
                await _session.RestoreStickerAsync(Reference(deleted), deleted.Lifecycle.Revision, _lifetime.Token);
                LastDeleted = null;
                return true;
            }
            finally { IsSaving = false; Notify(); _writes.Release(); }
        }
        catch (OperationCanceledException) { return false; }
        catch (Exception error) { if (!_stopping) Report(error); return false; }
    }

    internal Task<string> ThumbnailAsync(string id, CancellationToken cancellation) => _session.ThumbnailAsync(id, cancellation);
    private static EntityReference Reference(StickerMetadata value) => new(value.Id, value.Lifecycle.Generation);
    private void Report(Exception error) { Trace.WriteLine(error); ErrorKey = _strings.ErrorKey(error); Notify(); }

    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        _loads.Invalidate();
        _lifetime.Cancel();
        _loadCancellation?.Cancel();
        await Task.WhenAll(_loading, _refreshing);
        await _writes.WaitAsync();
        _writes.Release();
        _loadCancellation?.Dispose();
        _lifetime.Dispose();
        // Queued UI commands may still unwind after cancellation; do not dispose their semaphore.
    }
}
