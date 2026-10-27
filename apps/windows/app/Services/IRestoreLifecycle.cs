using System.Threading;
using System.Threading.Tasks;

namespace MemeDock.Windows.Services;

/// <summary>
/// What a restore needs from whoever owns the library session. A merge only changes
/// content, so the same session keeps working. A replace ends with the core closing the
/// library it restored into, so the owner has to open a new session and rebind every
/// consumer instead of refreshing a few properties.
/// </summary>
internal interface IRestoreLifecycle
{
    /// <summary>
    /// Whether the page can still run commands against the library. It is false once a
    /// replace committed but the rebound session could not be built, because a half-bound
    /// page must not keep operating on a library whose consumers are unknown.
    /// </summary>
    bool IsReady { get; }

    /// <summary>
    /// Blocks new conflicting work and returns false when something incompatible is
    /// already running. Every successful call must be matched by
    /// <see cref="EndRestoreAsync"/>. Remote sync is paused first so it cannot write to
    /// the library while the archive is applied; the caller's own paused choice is put
    /// back by <see cref="EndRestoreAsync"/>.
    /// </summary>
    Task<bool> TryBeginRestoreAsync();

    /// <summary>
    /// Resumes what <see cref="TryBeginRestoreAsync"/> paused, unless the session was
    /// replaced meanwhile (a new coordinator is already running then).
    /// </summary>
    Task EndRestoreAsync();

    /// <summary>
    /// Opens the library again when the core closed it. Returns true when the session was
    /// replaced and all consumers were rebound; false when the original session still owns
    /// the library (nothing was closed, so there is nothing to reopen). Throws when the
    /// library cannot be opened again or the page cannot be rebound.
    /// </summary>
    /// <remarks>
    /// Neither the result nor the exception says whether the contents were swapped, and a
    /// throw does not by itself prove the page is unusable. <see cref="IsReady"/> is the
    /// state the rebuild actually reached, so callers report from it instead of inferring
    /// from how this call ended.
    /// </remarks>
    Task<bool> ReopenAsync(CancellationToken cancellation);

    /// <summary>Reloads identity, statistics, catalogs and the visible page after a merge.</summary>
    Task RefreshAsync();
}
