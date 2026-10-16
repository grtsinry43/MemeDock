using System.Collections.ObjectModel;
using System.Diagnostics;
using MemeDock.Bridge;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.Windows.Storage.Pickers;

namespace MemeDock.Windows;

public sealed partial class MainWindow : Window
{
    public ObservableCollection<StickerCard> Cards { get; } = [];
    private readonly CancellationTokenSource _lifetime = new();
    private readonly SemaphoreSlim _thumbnailSlots = new(4);
    private readonly Dictionary<StickerCard, ThumbnailWork> _thumbnails = [];
    private LibrarySession? _session;
    private QueryCursorHandle? _cursor;
    private CancellationTokenSource? _queryCancellation;
    private CancellationTokenSource? _searchCancellation;
    private Task _initialization = Task.CompletedTask;
    private Task _loading = Task.CompletedTask;
    private Task _importing = Task.CompletedTask;
    private ScrollViewer? _scroll;
    private bool _initialized;
    private bool _hasMore;
    private bool _loadingPage;
    private bool _stopping;
    private bool _canClose;
    private int _generation;
    private string _query = "";

    private sealed class ThumbnailWork(CancellationTokenSource cancellation)
    {
        public CancellationTokenSource Cancellation { get; } = cancellation;
        public Task Completion { get; set; } = Task.CompletedTask;
    }

    public MainWindow()
    {
        InitializeComponent();
        AppWindow.Resize(new global::Windows.Graphics.SizeInt32(1120, 800));
        AppWindow.Closing += OnClosing;
    }

    private async void OnLoaded(object sender, RoutedEventArgs args)
    {
        if (_initialized) return;
        _initialized = true;
        _initialization = InitializeAsync();
        await _initialization;
    }

    private async Task InitializeAsync()
    {
        var elapsed = Stopwatch.StartNew();
        try
        {
            var root = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "MemeDock");
            _session = await LibrarySession.OpenAsync(root);
            if (_stopping) return;
            ImportButton.IsEnabled = Search.IsEnabled = true;
            await RefreshAsync();
            Trace.WriteLine($"MemeDock startup to first library page: {elapsed.ElapsedMilliseconds} ms");
        }
        catch (Exception error)
        {
            Status.Text = $"无法打开表情库：{error.Message}";
            Busy.IsActive = false;
        }
    }

    private async void OnSearchChanged(object sender, TextChangedEventArgs args)
    {
        if (_session is null || _stopping) return;
        _searchCancellation?.Cancel();
        _searchCancellation?.Dispose();
        _searchCancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        var cancellation = _searchCancellation.Token;
        try
        {
            await Task.Delay(250, cancellation);
            if (!cancellation.IsCancellationRequested) await RefreshAsync();
        }
        catch (OperationCanceledException) { }
    }

    private async Task RefreshAsync()
    {
        var generation = ++_generation;
        _queryCancellation?.Cancel();
        await _loading;
        if (_stopping || generation != _generation) return;
        _queryCancellation?.Dispose();
        _queryCancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        foreach (var work in _thumbnails.Values.ToArray()) work.Cancellation.Cancel();
        _cursor?.Dispose();
        _cursor = null;
        Cards.Clear();
        _query = Search.Text.Trim();
        _hasMore = true;
        _loading = LoadNextAsync(generation, _queryCancellation.Token);
        await _loading;
    }

    private async Task LoadNextAsync(int generation, CancellationToken cancellation)
    {
        if (_session is null || _loadingPage || !_hasMore || _stopping) return;
        _loadingPage = true;
        Busy.IsActive = true;
        EmptyState.Visibility = Visibility.Collapsed;
        try
        {
            var page = await _session.QueryAsync(_query, _cursor, cancellation);
            if (cancellation.IsCancellationRequested || generation != _generation)
            {
                page.Next?.Dispose();
                return;
            }
            _cursor?.Dispose();
            // Transfer the page's only owned handle to this window. Disposing
            // the page here would destroy the cursor needed for the next page.
            _cursor = page.Next;
            _hasMore = _cursor is not null;
            var resources = page.Resources.ToDictionary(resource => resource.Asset.Hash);
            for (var index = 0; index < page.Stickers.Length; index++)
            {
                var sticker = page.Stickers[index];
                var thumbnail = resources.TryGetValue(sticker.Id, out var resource) ? resource.ThumbnailPath : null;
                Cards.Add(new StickerCard(sticker.Id, sticker.Title, thumbnail));
            }
            Status.Text = _hasMore ? $"已加载 {Cards.Count} 张，继续滚动查看更多" : $"共 {Cards.Count} 张表情";
            EmptyState.Text = _query.Length == 0 ? "还没有表情，导入几张喜欢的图片吧。" : "没有找到匹配的表情，换个关键词试试。";
            EmptyState.Visibility = Cards.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        }
        catch (Exception error)
        {
            if (!cancellation.IsCancellationRequested) Status.Text = $"加载失败：{error.Message}";
        }
        finally
        {
            _loadingPage = false;
            Busy.IsActive = false;
        }
    }

    private void OnGalleryLoaded(object sender, RoutedEventArgs args)
    {
        _scroll = FindScrollViewer(Gallery);
        if (_scroll is not null) _scroll.ViewChanged += OnViewChanged;
    }

    private void OnViewChanged(object? sender, ScrollViewerViewChangedEventArgs args)
    {
        if (_stopping || _loadingPage || !_hasMore || _queryCancellation is null || _scroll is null) return;
        if (_scroll.ScrollableHeight - _scroll.VerticalOffset < 700)
            _loading = LoadNextAsync(_generation, _queryCancellation.Token);
    }

    private static ScrollViewer? FindScrollViewer(DependencyObject parent)
    {
        if (parent is ScrollViewer scroll) return scroll;
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(parent); index++)
        {
            var found = FindScrollViewer(VisualTreeHelper.GetChild(parent, index));
            if (found is not null) return found;
        }
        return null;
    }

    private void OnContainerContentChanging(ListViewBase sender, ContainerContentChangingEventArgs args)
    {
        if (args.Item is not StickerCard card) return;
        card.IsRealized = !args.InRecycleQueue;
        if (args.InRecycleQueue)
        {
            if (_thumbnails.TryGetValue(card, out var previous)) previous.Cancellation.Cancel();
            card.Thumbnail = null;
            return;
        }
        if (args.Phase == 0) StartThumbnail(card);
    }

    private void StartThumbnail(StickerCard card)
    {
        if (_stopping || _session is null || _thumbnails.ContainsKey(card)) return;
        var cancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        var work = new ThumbnailWork(cancellation);
        _thumbnails.Add(card, work);
        work.Completion = LoadThumbnailAsync(card, work);
    }

    private async Task LoadThumbnailAsync(StickerCard card, ThumbnailWork work)
    {
        var acquired = false;
        try
        {
            await _thumbnailSlots.WaitAsync(work.Cancellation.Token);
            acquired = true;
            var path = card.CachedThumbnailPath ?? await _session!.ThumbnailAsync(card.Id, work.Cancellation.Token);
            work.Cancellation.Token.ThrowIfCancellationRequested();
            var normalized = path.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase) ? @"\\" + path[8..]
                : path.StartsWith(@"\\?\", StringComparison.Ordinal) ? path[4..] : path;
            card.Thumbnail = new BitmapImage
            {
                DecodePixelWidth = 256,
                DecodePixelType = DecodePixelType.Physical,
                UriSource = new Uri(normalized),
                AutoPlay = false
            };
        }
        catch (Exception error)
        {
            if (!work.Cancellation.IsCancellationRequested)
                Trace.WriteLine($"Thumbnail {card.Id}: {error}");
        }
        finally
        {
            if (acquired) _thumbnailSlots.Release();
            var cancelled = work.Cancellation.IsCancellationRequested;
            _thumbnails.Remove(card);
            work.Cancellation.Dispose();
            if (cancelled && card.IsRealized && !_stopping) StartThumbnail(card);
        }
    }

    private async void OnImport(object sender, RoutedEventArgs args)
    {
        if (_session is null || _stopping || !_importing.IsCompleted) return;
        _importing = ImportAsync();
        await _importing;
    }

    private async Task ImportAsync()
    {
        ImportButton.IsEnabled = false;
        try
        {
            var picker = new FileOpenPicker(AppWindow.Id);
            foreach (var extension in new[] { ".png", ".jpg", ".jpeg", ".gif", ".webp" }) picker.FileTypeFilter.Add(extension);
            var files = await picker.PickMultipleFilesAsync();
            var completed = 0;
            foreach (var file in files)
            {
                _lifetime.Token.ThrowIfCancellationRequested();
                Busy.IsActive = true;
                Status.Text = $"正在导入 {++completed}/{files.Count}：{Path.GetFileName(file.Path)}";
                var outcome = await _session!.ImportAsync(file.Path, _lifetime.Token);
                if (outcome.Status == ImportStatus.RestoreRequired)
                    throw new IOException("该图片已在回收站中，需要先恢复后才能重新导入。");
            }
            if (!_stopping && files.Count > 0) await RefreshAsync();
        }
        catch (Exception error)
        {
            if (!_stopping) Status.Text = $"导入未完成：{error.Message}";
        }
        finally
        {
            ImportButton.IsEnabled = !_stopping && _session is not null;
            Busy.IsActive = false;
        }
    }

    private async void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_canClose) return;
        args.Cancel = true;
        if (_stopping) return;
        _stopping = true;
        Root.IsHitTestVisible = false;
        Status.Text = "正在关闭表情库…";
        _lifetime.Cancel();
        _queryCancellation?.Cancel();
        try
        {
            await _initialization;
            await Task.WhenAll(_loading, _importing);
            await Task.WhenAll(_thumbnails.Values.Select(work => work.Completion).ToArray());
            _cursor?.Dispose();
            if (_session is not null) await _session.DisposeAsync();
        }
        catch (Exception error) { Trace.WriteLine($"Library shutdown: {error}"); }
        finally
        {
            if (_scroll is not null) _scroll.ViewChanged -= OnViewChanged;
            _queryCancellation?.Dispose();
            _searchCancellation?.Dispose();
            _lifetime.Dispose();
            _thumbnailSlots.Dispose();
            _canClose = true;
            Close();
        }
    }
}
