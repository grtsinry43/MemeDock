using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;

namespace MemeDock.Windows.ViewModels;

public enum CatalogKind { Collection, Tag }
public sealed record CatalogTarget(CatalogKind Kind, EntityReference Reference, string Name, long Revision)
{
    public static CatalogTarget From(CollectionMetadata item) => new(CatalogKind.Collection,
        new(item.Id, item.Lifecycle.Generation), item.Name, item.Lifecycle.Revision);
    public static CatalogTarget From(TagMetadata item) => new(CatalogKind.Tag,
        new(item.Id, item.Lifecycle.Generation), item.Name, item.Lifecycle.Revision);
}

public sealed class CatalogViewModel : ObservableObject, IAsyncDisposable
{
    private readonly LibrarySession _session;
    private readonly LocalizationService _strings;
    private readonly Func<Task> _reload;
    private readonly CancellationTokenSource _lifetime = new();
    private Task _pending = Task.CompletedTask;
    private bool _stopping;
    public bool IsBusy { get; private set; }
    public string? ErrorKey { get; private set; }
    public string? MessageKey { get; private set; }
    public string ErrorText => ErrorKey is null ? "" : _strings[ErrorKey];
    public string MessageText => MessageKey is null ? "" : _strings[MessageKey];
    public CatalogTarget? LastDeleted { get; private set; }

    internal CatalogViewModel(LibrarySession session, LocalizationService strings, Func<Task> reload)
        => (_session, _strings, _reload) = (session, strings, reload);

    public Task<bool> CreateAsync(CatalogKind kind, string name) => RunAsync(async () =>
    {
        if (kind == CatalogKind.Collection) await _session.CreateCollectionAsync(name.Trim(), _lifetime.Token);
        else await _session.CreateTagAsync(name.Trim(), _lifetime.Token);
        MessageKey = "message_success";
    });

    public Task<bool> RenameAsync(CatalogTarget target, string name) => RunAsync(async () =>
    {
        if (target.Kind == CatalogKind.Collection) await _session.RenameCollectionAsync(target.Reference, name.Trim(), _lifetime.Token);
        else await _session.RenameTagAsync(target.Reference, name.Trim(), _lifetime.Token);
        MessageKey = "message_success";
    });

    public Task<bool> DeleteAsync(CatalogTarget target) => RunAsync(async () =>
    {
        LastDeleted = target.Kind == CatalogKind.Collection
            ? CatalogTarget.From(await _session.DeleteCollectionAsync(target.Reference, _lifetime.Token))
            : CatalogTarget.From(await _session.DeleteTagAsync(target.Reference, _lifetime.Token));
        MessageKey = target.Kind == CatalogKind.Collection ? "collection_deleted" : "tag_deleted";
    });

    public Task<bool> UndoAsync()
    {
        var target = LastDeleted;
        if (target is null) return Task.FromResult(false);
        return RunAsync(async () =>
        {
            if (target.Kind == CatalogKind.Collection) await _session.RestoreCollectionAsync(target.Reference, target.Revision, _lifetime.Token);
            else await _session.RestoreTagAsync(target.Reference, target.Revision, _lifetime.Token);
            LastDeleted = null;
            MessageKey = "restored";
        });
    }

    public Task<bool> MoveAsync(CollectionMetadata target, string? before) => RunAsync(async () =>
    {
        await _session.MoveCollectionAsync(new(target.Id, target.Lifecycle.Generation), before, _lifetime.Token);
        MessageKey = null;
    });

    public Task<bool> MoveStickerAsync(CollectionMetadata collection, StickerCard target, string? before) => RunAsync(async () =>
    {
        await _session.MoveCollectionItemAsync(new(collection.Id, collection.Lifecycle.Generation),
            new(target.Id, target.Generation), before, _lifetime.Token);
        MessageKey = null;
    });

    private Task<bool> RunAsync(Func<Task> mutation)
    {
        if (_stopping || IsBusy) return Task.FromResult(false);
        var work = RunCoreAsync(mutation);
        _pending = work;
        return work;
    }

    private async Task<bool> RunCoreAsync(Func<Task> mutation)
    {
        IsBusy = true;
        ErrorKey = null;
        Notify();
        try
        {
            await mutation();
            await _reload();
            return true;
        }
        catch (OperationCanceledException) { return false; }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            if (!_stopping)
            {
                ErrorKey = _strings.ErrorKey(error);
                // A drag may already have rearranged the observable list. Restore native order on failure.
                await _reload();
            }
            return false;
        }
        finally { IsBusy = false; Notify(); }
    }

    public void Relocalize() => Notify();
    private void Notify() => OnPropertyChanged(string.Empty);
    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        _lifetime.Cancel();
        await _pending;
        _lifetime.Dispose();
    }
}
