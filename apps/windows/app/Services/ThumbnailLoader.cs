using System.Diagnostics;
using Microsoft.UI.Xaml.Media.Imaging;

namespace MemeDock.Windows.Services;

internal sealed class ThumbnailLoader(LibrarySession session) : IAsyncDisposable
{
    private readonly CancellationTokenSource _lifetime = new();
    private readonly SemaphoreSlim _slots = new(4);
    private readonly Dictionary<StickerCard, Work> _work = [];
    private bool _stopping;
    private sealed class Work(CancellationTokenSource cancellation)
    {
        public CancellationTokenSource Cancellation { get; } = cancellation;
        public Task Completion { get; set; } = Task.CompletedTask;
    }

    public void Realize(StickerCard card)
    {
        card.IsRealized = true;
        if (_stopping || card.Thumbnail is not null || _work.ContainsKey(card)) return;
        var work = new Work(CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token));
        _work.Add(card, work);
        work.Completion = LoadAsync(card, work);
    }

    public void Recycle(StickerCard card)
    {
        card.IsRealized = false;
        if (_work.TryGetValue(card, out var work)) work.Cancellation.Cancel();
        card.Thumbnail = null;
    }

    public void Invalidate(StickerCard card)
    {
        card.CachedThumbnailPath = null;
        if (card.IsRealized && card.Thumbnail is null) Realize(card);
    }

    private async Task LoadAsync(StickerCard card, Work work)
    {
        var acquired = false;
        try
        {
            await _slots.WaitAsync(work.Cancellation.Token);
            acquired = true;
            var path = card.CachedThumbnailPath ?? await session.ThumbnailAsync(card.Id, work.Cancellation.Token);
            work.Cancellation.Token.ThrowIfCancellationRequested();
            var normalized = path.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase) ? @"\\" + path[8..]
                : path.StartsWith(@"\\?\", StringComparison.Ordinal) ? path[4..] : path;
            var image = new BitmapImage { DecodePixelWidth = 256, DecodePixelType = DecodePixelType.Physical, AutoPlay = false };
            image.ImageFailed += (_, _) => card.ThumbnailFailed = true;
            image.ImageOpened += (_, _) => card.ThumbnailFailed = false;
            card.ThumbnailFailed = false;
            image.UriSource = new Uri(normalized);
            card.Thumbnail = image;
        }
        catch (Exception error)
        {
            if (!work.Cancellation.IsCancellationRequested)
            {
                card.ThumbnailFailed = true;
                Trace.WriteLine($"Thumbnail {card.Id}: {error}");
            }
        }
        finally
        {
            if (acquired) _slots.Release();
            var cancelled = work.Cancellation.IsCancellationRequested;
            _work.Remove(card);
            work.Cancellation.Dispose();
            if (cancelled && card.IsRealized && !_stopping) Realize(card);
        }
    }

    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        _lifetime.Cancel();
        await Task.WhenAll(_work.Values.Select(work => work.Completion).ToArray());
        _slots.Dispose();
        _lifetime.Dispose();
    }
}
