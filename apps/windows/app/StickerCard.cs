using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using Microsoft.UI.Xaml.Media.Imaging;

namespace MemeDock.Windows;

public sealed class StickerCard : ObservableObject
{
    private StickerMetadata _metadata;
    public string Id => _metadata.Id;
    public string Title => _metadata.Title;
    public bool Starred => _metadata.Starred;
    public long Generation => _metadata.Lifecycle.Generation;
    public long Revision => _metadata.Lifecycle.Revision;
    public bool Animated { get; private set; }
    public string? CachedThumbnailPath { get; internal set; }
    internal bool IsRealized { get; set; }
    private BitmapImage? _thumbnail;
    private bool _thumbnailFailed;

    public StickerCard(StickerMetadata metadata, StickerResource? resource)
    {
        _metadata = metadata;
        Update(metadata, resource);
    }

    public BitmapImage? Thumbnail
    {
        get => _thumbnail;
        set => SetProperty(ref _thumbnail, value);
    }

    public bool ThumbnailFailed { get => _thumbnailFailed; internal set => SetProperty(ref _thumbnailFailed, value); }

    public void Update(StickerMetadata metadata, StickerResource? resource)
    {
        _metadata = metadata;
        CachedThumbnailPath = resource?.ThumbnailPath;
        Animated = resource?.Asset.Animated ?? false;
        OnPropertyChanged(nameof(Title));
        OnPropertyChanged(nameof(Starred));
        OnPropertyChanged(nameof(Animated));
    }
}
