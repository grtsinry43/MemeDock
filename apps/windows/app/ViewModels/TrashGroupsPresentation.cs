namespace MemeDock.Windows.ViewModels;

/// <summary>
/// Resource-key decisions for the deleted catalog, kept apart from WinUI and the
/// resource loader so the same rules can be exercised in a linked test build.
/// </summary>
internal static class TrashGroupsPresentation
{
    /// <summary>
    /// A failed refresh is a list problem, not a failed restore or purge, so it
    /// reuses the existing "collections and tags failed to load" copy instead of
    /// borrowing an operation error key that would read as an action failure.
    /// </summary>
    public const string RefreshFailureKey = "organize_load_failed";

    /// <summary>
    /// The error the panel shows. A business failure wins over a refresh failure:
    /// the action is the primary result and a stale list is secondary, so both
    /// are recorded but they never overwrite each other.
    /// </summary>
    public static string? VisibleErrorKey(string? operationKey, string? refreshKey)
        => operationKey ?? (refreshKey is null ? null : RefreshFailureKey);

    /// <summary>
    /// Only a successful core operation may announce success; a refresh result
    /// never turns into a success message.
    /// </summary>
    public static bool ShowsMessage(bool operationSucceeded, string? messageKey)
        => operationSucceeded && messageKey is not null;

    /// <summary>
    /// The extra line in the permanent-delete dialog. Only a collection promises
    /// that its stickers survive. A tag makes no such promise and has no
    /// localized tag-specific sentence to reuse, so it keeps the generic purge
    /// hint alone rather than borrowing the collection wording.
    /// </summary>
    public static string? ExtraPurgeHintKey(bool isCollection)
        => isCollection ? "desktop_collection_delete_hint" : null;
}
