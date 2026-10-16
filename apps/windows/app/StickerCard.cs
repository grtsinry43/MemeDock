using System.ComponentModel;
using Microsoft.UI.Xaml.Media.Imaging;

namespace MemeDock.Windows;

public sealed class StickerCard : INotifyPropertyChanged
{
    public string Id { get; }
    public string Title { get; }
    public string? CachedThumbnailPath { get; }
    internal bool IsRealized { get; set; }
    private BitmapImage? _thumbnail;

    public StickerCard(string id, string title, string? cachedThumbnailPath)
        => (Id, Title, CachedThumbnailPath) = (id, title, cachedThumbnailPath);

    public BitmapImage? Thumbnail
    {
        get => _thumbnail;
        set
        {
            _thumbnail = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Thumbnail)));
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;
}
