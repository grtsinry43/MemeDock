using System;
using System.Threading;
using System.Threading.Tasks;
using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

/// <summary>
/// Archive restore capabilities the core exposes. Inspection copies the chosen file into
/// the library's staging area and lets the core validate it; the returned preview owns
/// that staged copy until it is discarded or a restore consumes it.
/// </summary>
internal interface IArchiveWorkspace
{
    /// <summary>
    /// The archive ceiling the core enforces. Display only: the core performs the check
    /// while inspecting, and this value exists so a refusal can name the real limit.
    /// </summary>
    long MaxArchiveBytes { get; }

    /// <summary>
    /// Copies the file at <paramref name="sourcePath"/> into the library, then asks the
    /// core to validate it. Nothing stays staged when this throws.
    /// </summary>
    Task<IArchivePreview> InspectArchiveAsync(string sourcePath, CancellationToken cancellation);
}

/// <summary>
/// An archive the core has validated and summarised. Nothing here is inferred from the
/// file name: the counts come from the archive metadata the core parsed. Disposing
/// discards the staged copy exactly once, and a restore consumes it whether it succeeds
/// or fails.
/// </summary>
internal interface IArchivePreview : IAsyncDisposable
{
    /// <summary>The counts the core reported for the archive's own contents.</summary>
    ArchiveSummary Summary { get; }

    /// <summary>
    /// Applies the archive to the current library. Merge keeps what exists and adds what
    /// is missing; Replace swaps the whole library for the archive under a new identity.
    /// The core consumes this preview either way.
    /// </summary>
    Task<ArchiveSummary> RestoreArchiveAsync(ArchiveRestoreMode mode, CancellationToken cancellation);
}
