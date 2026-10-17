using MemeDock.Bridge;

namespace MemeDock.Windows.Models;

public enum LibraryScope { All, Starred, Recent, Collection, Tag }

public sealed record LibraryFilter(LibraryScope Scope, string? Id = null, string? Name = null)
{
    public static LibraryFilter All { get; } = new(LibraryScope.All);
    public bool AllowsCollectionOrder => Scope == LibraryScope.Collection;

    public StickerQuery Query(string requestId, string text, StickerSort sort, QueryCursorHandle? cursor = null)
    {
        if (Scope is LibraryScope.Collection or LibraryScope.Tag && string.IsNullOrWhiteSpace(Id))
            throw new ArgumentException("A collection or tag scope requires an ID.");
        return new StickerQuery(requestId, text.Trim(), Scope == LibraryScope.Collection ? Id : null,
            Scope == LibraryScope.Tag ? [Id!] : [], Scope == LibraryScope.Starred ? true : null,
            false, Scope == LibraryScope.Recent ? StickerSort.LastUsed
                : sort == StickerSort.CollectionOrder && !AllowsCollectionOrder ? StickerSort.Recent : sort,
            80, cursor);
    }
}

// Request generations also guard native operations whose cancellation arrives after completion.
public sealed class BrowseRequestState
{
    private long _generation;
    public long Current => _generation;
    public long Begin() => ++_generation;
    public bool IsCurrent(long generation) => generation == _generation;
    public void Invalidate() => ++_generation;
}
