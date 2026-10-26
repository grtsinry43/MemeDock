using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI;
using Microsoft.Windows.Storage.Pickers;

namespace MemeDock.Windows.Services;

/// <summary>
/// The system open dialog, tied to this window the same way the save dialog is: the
/// unpackaged app passes the window's native identity (<see cref="WindowId"/>, which is
/// the HWND) to the Windows App SDK picker so the dialog is modal to this window.
/// </summary>
internal sealed class ArchiveSourcePicker : IArchiveSourcePicker
{
    private const string BackupExtension = ".zip";

    private readonly WindowId _windowId;

    public ArchiveSourcePicker(WindowId windowId) => _windowId = windowId;

    public async Task<string?> PickArchivePathAsync(CancellationToken cancellation)
    {
        cancellation.ThrowIfCancellationRequested();
        var picker = new FileOpenPicker(_windowId);
        // The filter only narrows what the dialog shows. The core still decides whether
        // the bytes are a backup, so a renamed or mislabelled file cannot slip through.
        picker.FileTypeFilter.Add(BackupExtension);

        var result = await picker.PickSingleFileAsync();
        cancellation.ThrowIfCancellationRequested();
        return result?.Path;
    }
}
