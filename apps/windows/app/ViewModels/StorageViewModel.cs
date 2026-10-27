using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

/// <summary>
/// State for the settings page's storage section: one snapshot of the library's
/// own directories, plus the only destructive action the page offers (cleaning
/// the share copies the core has already retired).
/// </summary>
/// <remarks>
/// <para>
/// Reads go through <see cref="RefreshQueue"/>, so overlapping requests coalesce
/// into one extra pass and an older snapshot can never overwrite a newer one.
/// </para>
/// <para>
/// A failed read clears <see cref="Snapshot"/> and records an error key. The page
/// must render the error, never a zero-byte value: "could not read" and "nothing
/// stored" are different statements.
/// </para>
/// </remarks>
internal sealed class StorageViewModel : ObservableObject, IAsyncDisposable
{
    private readonly IStorageWorkspace _workspace;
    private readonly ICatalogStrings _strings;
    private readonly Func<string, DirectoryOpenResult> _openDirectory;
    private readonly RefreshQueue _refreshes;
    private readonly CancellationTokenSource _lifetime = new();
    private Task<bool> _cleaningTask = Task.FromResult(false);
    private bool _cleaning;
    private bool _stopping;
    private bool _disposed;

    public StorageViewModel(
        IStorageWorkspace workspace,
        ICatalogStrings strings,
        Func<string, DirectoryOpenResult>? openDirectory = null)
    {
        _workspace = workspace;
        _strings = strings;
        _openDirectory = openDirectory ?? DirectoryOpener.Open;
        _refreshes = new RefreshQueue(RefreshOnceAsync);
    }

    /// <summary>The last successfully read figures, or null when none are trustworthy.</summary>
    public StorageSnapshot? Snapshot { get; private set; }

    public bool IsLoading { get; private set; }

    public bool IsCleaning { get; private set; }

    /// <summary>Error key for the statistics read, or null when it succeeded.</summary>
    public string? StatisticsErrorKey { get; private set; }

    /// <summary>Error key for the last cleanup attempt, or null.</summary>
    public string? CleanupErrorKey { get; private set; }

    /// <summary>Success key for the last cleanup attempt, or null.</summary>
    public string? CleanupResultKey { get; private set; }

    /// <summary>Entries the core reported as removed by the last cleanup.</summary>
    public ulong? CleanedItems { get; private set; }

    /// <summary>Error key for the last "open folder" attempt, or null.</summary>
    public string? DirectoryErrorKey { get; private set; }

    public string DataDirectory => _workspace.DataDirectory;

    public string CacheDirectory => _workspace.CacheDirectory;

    public string ShareDirectory => _workspace.ShareDirectory;

    public bool IsReady => !_stopping && !_disposed;

    /// <summary>Completes when no read is in flight; used by the session shutdown.</summary>
    public Task Completion => _refreshes.Completion;

    public Task RefreshAsync() => IsReady ? _refreshes.Request() : Task.CompletedTask;

    /// <summary>
    /// Asks the core to remove the artifacts it considers disposable, then reads
    /// the directories again. Returns false when the request did not start, was
    /// cancelled, or failed; the properties carry the reason.
    /// </summary>
    public Task<bool> CleanAsync()
    {
        if (!IsReady || _cleaning) return Task.FromResult(false);
        _cleaning = true;
        IsCleaning = true;
        CleanupErrorKey = null;
        CleanupResultKey = null;
        CleanedItems = null;
        OnPropertyChanged(nameof(IsCleaning));
        OnPropertyChanged(nameof(CleanupErrorKey));
        OnPropertyChanged(nameof(CleanupResultKey));
        OnPropertyChanged(nameof(CleanedItems));

        _cleaningTask = CleanCoreAsync();
        return _cleaningTask;
    }

    private async Task<bool> CleanCoreAsync()
    {
        // Do not walk the share directory while it is being deleted, and do not
        // let a pass start in the middle of the deletion.
        _refreshes.Suspend();
        try
        {
            await _refreshes.Completion;
            if (!IsReady) return false;
            var removed = await _workspace.CleanExportArtifactsAsync(_lifetime.Token);
            if (!IsReady) return false;

            // The core returns how many entries it removed (artifact records and
            // orphan files). It never reports bytes, so neither do we.
            CleanedItems = removed;
            CleanupResultKey = "desktop_cleanup_result";
            return true;
        }
        catch (OperationCanceledException)
        {
            return false;
        }
        catch (Exception error)
        {
            // Cleanup is not transactional: a failure can leave some files gone.
            // Keep the error visible and still refresh the figures below.
            Trace.WriteLine($"Export artifact cleanup failed: {error.GetType().Name}");
            CleanupErrorKey = _strings.ErrorKey(error);
            return false;
        }
        finally
        {
            _cleaning = false;
            IsCleaning = false;
            OnPropertyChanged(nameof(IsCleaning));
            OnPropertyChanged(nameof(CleanupErrorKey));
            OnPropertyChanged(nameof(CleanupResultKey));
            OnPropertyChanged(nameof(CleanedItems));
            if (IsReady) await _refreshes.Resume();
            else _refreshes.Stop();
        }
    }

    /// <summary>
    /// Hands a directory to the shell. Never creates it; a missing or unopenable
    /// path becomes <see cref="DirectoryErrorKey"/> instead of a silent no-op.
    /// </summary>
    public bool OpenDirectory(string directory)
    {
        if (!IsReady) return false;
        var result = _openDirectory(directory);
        DirectoryErrorKey = result switch
        {
            DirectoryOpenResult.Missing => "desktop_directory_missing",
            DirectoryOpenResult.Failed => "desktop_directory_open_failed",
            _ => null
        };
        OnPropertyChanged(nameof(DirectoryErrorKey));
        return result == DirectoryOpenResult.Opened;
    }

    public void RequestStop()
    {
        if (_stopping) return;
        _stopping = true;
        _lifetime.Cancel();
        _refreshes.Stop();
        OnPropertyChanged(nameof(IsReady));
    }

    public async ValueTask DisposeAsync()
    {
        if (_disposed) return;
        RequestStop();
        try
        {
            await Task.WhenAll(_cleaningTask, _refreshes.Completion);
        }
        catch (OperationCanceledException)
        {
        }
        _disposed = true;
        _lifetime.Dispose();
        GC.SuppressFinalize(this);
    }

    private async Task<bool> RefreshOnceAsync()
    {
        IsLoading = true;
        OnPropertyChanged(nameof(IsLoading));
        try
        {
            var statistics = await _workspace.StatisticsAsync(_lifetime.Token);
            if (!IsReady) return false;
            if (!StorageSnapshot.TryCreate(statistics, out var snapshot))
            {
                Snapshot = null;
                StatisticsErrorKey = "failure_unknown";
                return false;
            }
            Snapshot = snapshot;
            StatisticsErrorKey = null;
            return true;
        }
        catch (OperationCanceledException)
        {
            return false;
        }
        catch (Exception error)
        {
            Trace.WriteLine($"Storage statistics failed: {error.GetType().Name}");
            Snapshot = null;
            StatisticsErrorKey = _strings.ErrorKey(error);
            return false;
        }
        finally
        {
            IsLoading = false;
            OnPropertyChanged(nameof(IsLoading));
            OnPropertyChanged(nameof(Snapshot));
            OnPropertyChanged(nameof(StatisticsErrorKey));
        }
    }
}
