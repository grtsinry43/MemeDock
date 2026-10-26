using MemeDock.Bridge;

namespace MemeDock.Windows.Services;

/// <summary>
/// Turns sync state into resource keys without touching WinUI, so both the view
/// and the tests use the same decision table.
/// </summary>
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

    public static string StatusKey(SyncState state)
    {
        if (state.Running) return "sync_running";
        if (NeedsAuthorization(state)) return "sync_authorize";
        if (state.Status.Paused) return "sync_paused";
        if (IsFailed(state)) return "sync_failed";
        return "sync_ready";
    }

    /// <summary>
    /// Sync-specific copy for network, authorization and credential failures.
    /// Other failures keep the generic <c>failure_*</c> texts.
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
        return state.Status.Error switch
        {
            "unauthorized" => "sync_failure_unauthorized",
            "network" or "timeout" or "rate_limited" => "sync_failure_network",
            _ => null
        };
    }

    private static bool IsUnauthorized(Exception? error)
        => error is BridgeException.Failure { code: ErrorCode.Unauthorized };
}
