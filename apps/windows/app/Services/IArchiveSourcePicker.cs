using System.Threading;
using System.Threading.Tasks;

namespace MemeDock.Windows.Services;

/// <summary>
/// Asks the user for an existing backup file. The returned path is only a location: the
/// caller still has to let the core validate the file, because an extension proves
/// nothing about the contents.
/// </summary>
internal interface IArchiveSourcePicker
{
    /// <summary>Returns the chosen path, or <c>null</c> when the user dismisses the dialog.</summary>
    Task<string?> PickArchivePathAsync(CancellationToken cancellation);
}
