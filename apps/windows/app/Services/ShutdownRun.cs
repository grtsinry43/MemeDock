namespace MemeDock.Windows.Services;

// Closing the window is the last chance to release the native library, so one
// failing step must not skip the steps after it. Cancellation is expected while
// shutting down (the lifetime token is already cancelled) and is not a failure.
// Failures are returned instead of thrown so the caller finishes its own cleanup
// and then decides where the error is logged and rethrown.
//
// The steps deliberately keep the caller's synchronization context: disposal
// touches UI-bound view models, so no ConfigureAwait(false) here.
internal static class ShutdownRun
{
    internal static async Task<Exception?> SequenceAsync(params Func<Task>[] steps)
    {
        List<Exception>? failures = null;
        foreach (var step in steps)
        {
            try { await step(); }
            catch (OperationCanceledException) { }
            catch (Exception error)
            {
                if (failures is null) failures = new();
                failures.Add(error);
            }
        }
        if (failures is null) return null;
        return failures.Count == 1 ? failures[0] : new AggregateException(failures);
    }
}
