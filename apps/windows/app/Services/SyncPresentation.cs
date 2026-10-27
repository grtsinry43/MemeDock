using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

/// <summary>What the sync entry says, independent of where it is rendered.</summary>
public enum SyncPhase
{
    /// <summary>The native status has not been read yet.</summary>
    Unavailable,

    /// <summary>A request is in flight: it says nothing about the next round.</summary>
    Running,

    /// <summary>A usable credential is missing or was rejected.</summary>
    Authorization,

    /// <summary>The user paused syncing; the network is not the problem.</summary>
    Paused,

    /// <summary>Some changes cannot be synced and need a decision in the web console.</summary>
    Blocked,

    /// <summary>The last attempt failed and will be retried.</summary>
    Failed,

    /// <summary>Idle with local changes still waiting for the next round.</summary>
    Pending,

    /// <summary>Connected and idle, but no round has completed yet.</summary>
    Unsynced,

    /// <summary>Idle, nothing pending, and a round has completed.</summary>
    UpToDate,
}

/// <summary>
/// Turns sync state into resource keys without touching WinUI, so the main
/// window, the settings page and the tests share one decision table.
/// </summary>
/// <remarks>
/// The native status only ever proves "a round completed", "changes are still
/// queued", "some changes are blocked" or "the last attempt failed". None of
/// those is the same as "everything on the server is here", so the ready text
/// is limited to an idle, successfully synced binding.
/// </remarks>
public static class SyncPresentation
{
    private static readonly string[] TerminalStatusErrors =
        ["unauthorized", "invalid_input", "unsupported_schema", "corrupt_data"];

    public static bool NeedsAuthorization(SyncState state)
        => !state.HasCredential
           || state.Error is SyncCredentialException
           || IsUnauthorized(state.Error)
           || state.Status.Error == "unauthorized";

    public static bool IsFailed(SyncState state)
        => state.Error is not null || !string.IsNullOrEmpty(state.Status.Error);

    public static bool IsTerminal(SyncState state)
        => state.Error is SyncCredentialException
           || state.Error is BridgeException.Failure { code: ErrorCode.Unauthorized or ErrorCode.InvalidInput
               or ErrorCode.UnsupportedSchema or ErrorCode.CorruptData }
           || state.Status.Error is { Length: > 0 } status && TerminalStatusErrors.Contains(status);

    public static bool CanSynchronize(SyncState state)
        => state.Loaded && state.HasCredential && state.Status.Connected && !state.Status.Paused
           && !IsTerminal(state);

    /// <summary>
    /// A window entry appears only once the library has a sync binding or
    /// something to report; an unconfigured library stays out of the way.
    /// </summary>
    public static bool IsVisible(SyncState state)
        => state.Loaded
           && (state.HasCredential || state.Status.Connected || state.Status.Pending > 0
               || state.Status.Blocked > 0 || IsFailed(state));

    /// <summary>
    /// Ordered by what the user needs to act on. Paused comes before failure so
    /// the user's own choice is not reported as a network problem, and blocked
    /// comes before failure because blocked changes outlive a retryable error.
    /// Running wins outright: it only claims a request was sent.
    /// </summary>
    public static SyncPhase Phase(SyncState state)
    {
        if (!state.Loaded) return SyncPhase.Unavailable;
        if (state.Running) return SyncPhase.Running;
        if (NeedsAuthorization(state)) return SyncPhase.Authorization;
        if (state.Status.Paused) return SyncPhase.Paused;
        if (state.Status.Blocked > 0) return SyncPhase.Blocked;
        if (IsFailed(state)) return SyncPhase.Failed;
        if (state.Status.Pending > 0) return SyncPhase.Pending;
        return state.Status.LastSuccessMs is > 0 ? SyncPhase.UpToDate : SyncPhase.Unsynced;
    }

    public static string StatusKey(SyncState state) => Phase(state) switch
    {
        SyncPhase.Unavailable => "sync_authorize",
        SyncPhase.Running => "sync_running",
        // A stored token that no longer works needs a different instruction from
        // a library that was never connected.
        SyncPhase.Authorization => state.HasCredential ? "sync_reauthorize" : "sync_authorize",
        SyncPhase.Paused => "sync_paused",
        SyncPhase.Blocked => "sync_blocked",
        SyncPhase.Failed => "sync_failed",
        SyncPhase.Pending => "sync_pending",
        // Connected with nothing queued but no completed round: the settings
        // page shows "last synced: never", so the status says the same instead
        // of claiming the library is up to date.
        SyncPhase.Unsynced => "sync_never",
        _ => "sync_ready",
    };

    /// <summary>Extra explanation for the phases that need one; null otherwise.</summary>
    public static string? HintKey(SyncState state) => Phase(state) switch
    {
        SyncPhase.Failed => ErrorKey(state) ?? "sync_failed",
        SyncPhase.Blocked => "sync_blocked_hint",
        SyncPhase.Authorization when state.HasCredential => "sync_failure_unauthorized",
        _ => null,
    };

    /// <summary>
    /// Sync-specific copy for network, authorization and credential failures.
    /// Status errors this build does not know keep a visible explanation instead
    /// of failing silently; other failures keep the generic <c>failure_*</c>
    /// texts.
    /// </summary>
    public static string? ErrorKey(SyncState state)
    {
        if (state.Error is SyncCredentialException) return "sync_failure_credential";
        if (state.Error is BridgeException.Failure failure)
        {
            if (failure.code == ErrorCode.Unauthorized) return "sync_failure_unauthorized";
            if (failure.code is ErrorCode.Network or ErrorCode.Timeout or ErrorCode.RateLimited)
                return "sync_failure_network";
        }
        // Pausing stops the retry loop, so a retryable status error left over
        // from the last attempt is history until syncing resumes; presenting it
        // would report the user's own pause as a network problem.
        if (state.Status.Paused && state.Error is null && IsRetryableStatus(state.Status.Error)) return null;
        return state.Status.Error switch
        {
            "unauthorized" => "sync_failure_unauthorized",
            "network" or "timeout" or "rate_limited" => "sync_failure_network",
            _ => null
        };
    }

    private static bool IsRetryableStatus(string? error)
        => error is "network" or "timeout" or "rate_limited";

    private static bool IsUnauthorized(Exception? error)
        => error is BridgeException.Failure { code: ErrorCode.Unauthorized };
}
