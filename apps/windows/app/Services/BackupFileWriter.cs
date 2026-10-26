using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;

namespace MemeDock.Windows.Services;

/// <summary>
/// Raised when the destination already holds a file and replacing it was not confirmed.
/// </summary>
internal sealed class BackupTargetExistsException : IOException
{
    public BackupTargetExistsException(string path)
        : base($"A file already exists at '{path}'.")
    {
        TargetPath = path;
    }

    public string TargetPath { get; }
}

/// <summary>
/// Copies a staged archive to the location the user chose. The bytes are written to a
/// temporary sibling first and moved into place with one rename, so a failed copy never
/// leaves a truncated file under the backup's name and never destroys a file that was
/// already there.
/// </summary>
internal static class BackupFileWriter
{
    private const int BufferSize = 1 << 20;

    /// <param name="source">The archive staged by the core.</param>
    /// <param name="destination">Where the user wants the backup.</param>
    /// <param name="expectedBytes">The size the core reported for the archive.</param>
    /// <param name="replaceExisting">
    /// True only when the user confirmed replacing the file at the destination.
    /// </param>
    /// <param name="cancellation">Cancels the copy between chunks.</param>
    public static async Task WriteAsync(
        string source,
        string destination,
        long expectedBytes,
        bool replaceExisting,
        CancellationToken cancellation)
    {
        if (!replaceExisting && File.Exists(destination))
        {
            throw new BackupTargetExistsException(destination);
        }

        var directory = Path.GetDirectoryName(Path.GetFullPath(destination))
            ?? throw new IOException("The destination has no directory.");
        var temporary = Path.Combine(
            directory,
            $".{Path.GetFileName(destination)}.{Guid.NewGuid():N}.partial");
        try
        {
            await using (var input = new FileStream(
                source, FileMode.Open, FileAccess.Read, FileShare.Read, BufferSize,
                FileOptions.Asynchronous | FileOptions.SequentialScan))
            await using (var output = new FileStream(
                temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None, BufferSize,
                FileOptions.Asynchronous))
            {
                await input.CopyToAsync(output, BufferSize, cancellation).ConfigureAwait(false);
                await output.FlushAsync(cancellation).ConfigureAwait(false);
                if (output.Length != expectedBytes)
                {
                    // A short copy must never be presented as a finished backup.
                    throw new IOException(
                        $"The core reported {expectedBytes} bytes but {output.Length} were copied.");
                }
            }

            // Renaming inside the destination directory keeps the switch atomic, so the
            // destination is either the previous file or the complete new backup.
            try
            {
                File.Move(temporary, destination, replaceExisting);
            }
            catch (IOException) when (!replaceExisting && File.Exists(destination))
            {
                throw new BackupTargetExistsException(destination);
            }
        }
        finally
        {
            TryDelete(temporary);
        }
    }

    private static void TryDelete(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
        }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
    }
}
