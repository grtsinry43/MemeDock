namespace MemeDock.Windows.Localization;

/// <summary>
/// Resource lookup a view model needs. <see cref="LocalizationService"/> is the
/// production implementation; this seam keeps the WinUI resource loader out of
/// the linked test build so view-model rules can be exercised with plain text.
/// </summary>
public interface ICatalogStrings
{
    string this[string key] { get; }
    string Format(string key, params object[] arguments);
    string ErrorKey(Exception error);
}
