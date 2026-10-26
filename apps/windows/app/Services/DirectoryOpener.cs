using System.ComponentModel;
using System.Diagnostics;

namespace MemeDock.Windows.Services;

/// <summary>Outcome of asking the shell to show a folder.</summary>
internal enum DirectoryOpenResult
{
    Opened,
    Missing,
    Failed
}

/// <summary>
/// Opens a folder with its registered shell handler. This never creates a
/// directory: a path that is not there is reported as <see cref="DirectoryOpenResult.Missing"/>.
/// </summary>
internal static class DirectoryOpener
{
    public static DirectoryOpenResult Open(string directory)
    {
        if (!Directory.Exists(directory)) return DirectoryOpenResult.Missing;
        try
        {
            using var process = Process.Start(new ProcessStartInfo(directory) { UseShellExecute = true });
            return DirectoryOpenResult.Opened;
        }
        catch (Exception error) when (error is Win32Exception or InvalidOperationException
            or PlatformNotSupportedException or FileNotFoundException or DirectoryNotFoundException)
        {
            // The path is user-facing data, not a secret, but the exception can
            // still be noisy; the type is enough to diagnose a shell failure.
            Trace.WriteLine($"Folder could not be opened: {error.GetType().Name}");
            return DirectoryOpenResult.Failed;
        }
    }
}
