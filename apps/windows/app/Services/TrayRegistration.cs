namespace MemeDock.Windows.Services;

internal enum TrayOperation : uint { Add = 0, Modify = 1, Delete = 2, SetVersion = 4 }

// Shell version negotiation must never revoke a successfully registered resident icon.
// Kept separate from HWND callbacks so failure/recovery can be tested without driving the UI.
internal sealed class TrayRegistration(Func<TrayOperation, bool, bool> notify) : IDisposable
{
    public bool Available { get; private set; }
    public bool UsesGuid { get; private set; } = true;
    public bool Version4 { get; private set; }
    private bool _disposed;

    public bool Install()
    {
        if (_disposed) return false;
        if (Available) return true;
        foreach (var useGuid in new[] { true, false })
        {
            // A repeated TaskbarCreated broadcast may arrive without the old icon
            // being removed. Modify first makes recovery idempotent.
            if (!notify(TrayOperation.Modify, useGuid) && !notify(TrayOperation.Add, useGuid)) continue;
            UsesGuid = useGuid;
            Available = true;
            Version4 = notify(TrayOperation.SetVersion, useGuid);
            return true;
        }
        return false;
    }
    public void TaskbarRestarted()
    {
        Available = false;
        Version4 = false;
    }
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (Available) notify(TrayOperation.Delete, UsesGuid);
        Available = false;
    }
}
