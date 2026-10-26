using System.Threading;
using System.Threading.Tasks;

namespace MemeDock.Windows.Services;

/// <summary>
/// Where the user wants a finished backup saved.
/// </summary>
/// <param name="Path">The absolute path the save dialog returned.</param>
/// <param name="ReplaceExisting">
/// True only when the dialog asked the user about the file already at that path before
/// returning it. The picker reports this instead of the writer guessing from
/// <see cref="System.IO.File.Exists(string)"/>: only the dialog knows whether a
/// replacement was confirmed.
/// </param>
internal readonly record struct BackupDestination(string Path, bool ReplaceExisting);

/// <summary>
/// Asks the user where a finished backup should be saved. Returns the chosen
/// destination, or null when the user dismissed the picker without choosing anything.
/// </summary>
internal interface IBackupDestinationPicker
{
    Task<BackupDestination?> PickSavePathAsync(string suggestedFileName, CancellationToken cancellation);
}
