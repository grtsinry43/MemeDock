using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using MemeDock.Windows.Localization;
using Microsoft.UI;
using Microsoft.Windows.Storage.Pickers;

namespace MemeDock.Windows.Services;

/// <summary>
/// The system save dialog. The app is unpackaged, so the Windows App SDK picker has to
/// be tied to a window: it takes the window's native identity (<see cref="WindowId"/>,
/// which is the HWND) in its constructor instead of the older IInitializeWithWindow
/// call. That association is what makes the dialog modal to this window and keeps it
/// from opening behind it.
/// </summary>
internal sealed class BackupDestinationPicker : IBackupDestinationPicker
{
    private const string BackupExtension = ".zip";

    private readonly WindowId _windowId;
    private readonly ICatalogStrings _strings;

    public BackupDestinationPicker(WindowId windowId, ICatalogStrings strings)
    {
        _windowId = windowId;
        _strings = strings;
    }

    public async Task<BackupDestination?> PickSavePathAsync(string suggestedFileName, CancellationToken cancellation)
    {
        cancellation.ThrowIfCancellationRequested();
        var picker = new FileSavePicker(_windowId)
        {
            SuggestedFileName = suggestedFileName,
            DefaultFileExtension = BackupExtension,
        };
        // The dialog itself asks before handing back an existing file, so an overwrite
        // is always the user's decision; the writer still refuses to replace anything
        // unless it was told this confirmation happened.
        picker.ShowOverwritePrompt = true;
        picker.FileTypeChoices.Add(_strings["desktop_backup_file_type"], new List<string> { BackupExtension });

        var result = await picker.PickSaveFileAsync();
        cancellation.ThrowIfCancellationRequested();
        if (result is null) return null;
        // Only report a replacement for a file that is already there when the dialog
        // returns; one that appears later is a conflict the writer reports instead.
        return new BackupDestination(result.Path, File.Exists(result.Path));
    }
}
