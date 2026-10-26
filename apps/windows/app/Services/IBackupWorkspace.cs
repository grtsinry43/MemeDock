using System;
using System.Threading;
using System.Threading.Tasks;

namespace MemeDock.Windows.Services;

/// <summary>
/// Backup capabilities the core actually exposes: staging a portable archive inside
/// the library's own data directory, and writing a checkpoint of the current database.
/// Restoring reads archives through <see cref="IArchiveWorkspace"/> instead: an archive
/// the library writes is not the same resource as one it reads.
/// </summary>
internal interface IBackupWorkspace
{
    /// <summary>
    /// The archive ceiling the core enforces while writing. Display only: the core
    /// performs the check, and this value exists so a refusal can name the real limit
    /// instead of a guessed one.
    /// </summary>
    long MaxArchiveBytes { get; }

    /// <summary>
    /// Writes a complete archive into the core's staging area and returns it. That file
    /// is not the user's backup yet: it has to be saved elsewhere, and the staging entry
    /// has to be released through the returned handle.
    /// </summary>
    Task<IStagedBackup> StageBackupAsync(IProgress<string>? stages, CancellationToken cancellation);

    /// <summary>
    /// Writes a database snapshot inside the library's data directory and returns its
    /// absolute path. That is a working snapshot the core keeps, not a portable archive.
    /// </summary>
    Task<string> CreateCheckpointAsync(CancellationToken cancellation);
}

/// <summary>
/// An archive the core has written but not yet handed to the user. Disposing releases
/// the staging entry exactly once, whether or not the copy to the user's location
/// succeeded.
/// </summary>
internal interface IStagedBackup : IAsyncDisposable
{
    /// <summary>The staged archive inside the library's data directory.</summary>
    string Path { get; }

    /// <summary>The file name the core suggests, including its extensions.</summary>
    string FileName { get; }

    /// <summary>The size the core reported for this archive.</summary>
    long ByteSize { get; }
}
