using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

internal sealed class QuickPickerViewModel(LibrarySession session, ICatalogStrings strings, int maximum = int.MaxValue) : ObservableObject, IAsyncDisposable
{
    public ObservableCollection<StickerCard> Cards { get; } = [];
    public ThumbnailLoader Thumbnails { get; } = new(session);
    public LibraryFilter Filter { get; private set; } = new(LibraryScope.Recent);
    public string SearchText { get; private set; } = "";
    public string? Error { get; private set; }
    public bool Loading { get; private set; }
    public bool Visible { get; private set; }
    public bool HasMore => _cursor is not null;
    private QueryCursorHandle? _cursor;
    private CancellationTokenSource? _cancellation;
    private readonly SemaphoreSlim _gate = new(1);
    private Task _work = Task.CompletedTask;
    private long _version;
    private bool _stopping;
    public Task PresentAsync()
    {
        Visible = true;
        Filter = new(LibraryScope.Recent);
        SearchText = "";
        return ReloadAsync();
    }
    public Task SelectAsync(LibraryFilter filter) { Filter = filter; return ReloadAsync(); }
    public Task SearchAsync(string text) { SearchText = text; return ReloadAsync(debounce: true); }
    public Task ReloadAsync(bool debounce = false)
    {
        if (!Visible || _stopping) return Task.CompletedTask;
        var version = ++_version;
        var retired = _cancellation;
        _cancellation?.Cancel();
        var cancellation = new CancellationTokenSource();
        _cancellation = cancellation;
        var previous = _work;
        return _work = LoadAsync(previous, version, Filter, SearchText, cancellation, debounce, more: false, retired);
    }
    public Task NextAsync()
    {
        if (!Visible || _stopping || Loading || !HasMore || _cancellation is null) return Task.CompletedTask;
        var previous = _work;
        return _work = LoadAsync(previous, _version, Filter, SearchText, _cancellation, false, more: true);
    }
    private async Task LoadAsync(Task previous, long version, LibraryFilter filter, string search,
        CancellationTokenSource cancellation, bool debounce, bool more, CancellationTokenSource? retired = null)
    {
        var acquired = false;
        try
        {
            await previous;
            retired?.Dispose();
            if (debounce) await Task.Delay(150, cancellation.Token);
            await _gate.WaitAsync(cancellation.Token);
            acquired = true;
            cancellation.Token.ThrowIfCancellationRequested();
            if (_stopping || !Visible || version != _version) return;
            Loading = true;
            Error = null;
            OnPropertyChanged(string.Empty);
            if (!more) { _cursor?.Dispose(); _cursor = null; }
            var page = await session.QueryAsync(filter, search, StickerSort.Recent, more ? _cursor : null, cancellation.Token);
            if (cancellation.IsCancellationRequested || version != _version || !Visible || _stopping)
            { page.Next?.Dispose(); return; }
            _cursor?.Dispose();
            _cursor = page.Next;
            if (!more)
            {
                foreach (var card in Cards) Thumbnails.Recycle(card);
                Cards.Clear();
            }
            var resources = page.Resources.ToDictionary(resource => resource.Asset.Hash);
            var ids = Cards.Select(card => card.Id).ToHashSet();
            foreach (var item in page.Stickers.Take(Math.Max(0, maximum - Cards.Count)))
                if (ids.Add(item.Id)) Cards.Add(new(item, resources.GetValueOrDefault(item.Id)));
            if (Cards.Count >= maximum) { _cursor?.Dispose(); _cursor = null; }
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { if (Visible && !_stopping && version == _version) Error = strings[strings.ErrorKey(error)]; }
        finally
        {
            if (acquired) _gate.Release();
            if (version == _version) { Loading = false; OnPropertyChanged(string.Empty); }
        }
    }
    public void Suspend()
    {
        Visible = false;
        ++_version;
        _cancellation?.Cancel();
        foreach (var card in Cards) Thumbnails.Recycle(card);
        Cards.Clear();
        OnPropertyChanged(string.Empty);
    }
    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        Suspend();
        await _work;
        _cursor?.Dispose();
        _cancellation?.Dispose();
        await Thumbnails.DisposeAsync();
        _gate.Dispose();
    }
}
