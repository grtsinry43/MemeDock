using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

/// <summary>
/// Creates backups and checkpoints, and restores a backup the core validated. A backup is
/// staged by the core first and only becomes the user's file once they pick a location and
/// the copy completes, so a cancelled or failed run never leaves a partial file under the
/// backup's name. A restore is only offered after the core inspected the chosen file; the
/// counts shown come from that inspection, never from the file name.
/// </summary>
internal sealed class BackupViewModel : ObservableObject, IAsyncDisposable
{
    // The core names the stages it is working through. Unknown tokens are not shown:
    // a raw identifier must never leak into the page as if it were copy.
    private static readonly Dictionary<string, string> StageKeys = new(StringComparer.Ordinal)
    {
        ["creating_backup_snapshot"] = "backup_creating",
        ["copying_backup_originals"] = "backup_creating",
    };

    // Bound to the current session. A replace closes the library, so the backup view
    // model is the one consumer that survives a rebuild and has its handles swapped.
    private IBackupWorkspace? _workspace;
    private IArchiveWorkspace? _archives;
    private readonly IBackupDestinationPicker _picker;
    private readonly IArchiveSourcePicker _sources;
    private readonly IRestoreLifecycle _lifecycle;
    private readonly ICatalogStrings _strings;
    private readonly CancellationTokenSource _lifetime = new();
    private Task<bool> _backupRun = Task.FromResult(false);
    private Task<bool> _checkpointRun = Task.FromResult(false);
    private Task<bool> _inspectRun = Task.FromResult(false);
    private Task<bool> _restoreRun = Task.FromResult(false);
    private IArchivePreview? _preview;
    private bool _stopping;
    private bool _disposed;
    private bool _reportingStages;

    public BackupViewModel(
        IBackupWorkspace? workspace,
        IArchiveWorkspace? archives,
        IBackupDestinationPicker picker,
        IArchiveSourcePicker sources,
        IRestoreLifecycle lifecycle,
        ICatalogStrings strings)
    {
        (_workspace, _archives, _picker, _sources, _lifecycle, _strings) =
            (workspace, archives, picker, sources, lifecycle, strings);
    }

    /// <summary>
    /// Rebinds this section to the session that replaced the closed one. The section is
    /// not rebuilt because it is the caller of the restore: its in-flight run must keep
    /// its own state, and only the library handles behind it change. The preview is not
    /// touched either; a restore in progress already owns it.
    /// </summary>
    public void Adopt(IBackupWorkspace workspace, IArchiveWorkspace archives)
    {
        _workspace = workspace;
        _archives = archives;
        Raise(nameof(IsReady));
    }

    public bool IsReady => _workspace is not null && !_stopping;

    public bool IsCreatingBackup { get; private set; }

    public bool IsCreatingCheckpoint { get; private set; }

    public bool IsInspecting { get; private set; }

    public bool IsRestoring { get; private set; }

    public bool IsBusy => IsCreatingBackup || IsCreatingCheckpoint || IsInspecting || IsRestoring;

    /// <summary>Localized key for the stage the core reported, or null when unknown.</summary>
    public string? StageKey { get; private set; }

    public string? BackupErrorKey { get; private set; }

    /// <summary>
    /// Already displayable value for a key that takes one, such as the archive limit.
    /// </summary>
    public string? BackupErrorPlaceholder { get; private set; }

    public string? BackupSavedPath { get; private set; }

    public long? BackupSavedBytes { get; private set; }

    public string? CheckpointErrorKey { get; private set; }

    public string? CheckpointName { get; private set; }

    /// <summary>
    /// True once the core validated an archive and reported its contents. Until then the
    /// page has nothing truthful to show about a file the user only picked.
    /// </summary>
    public bool HasPreview => _preview is not null;

    /// <summary>The chosen file's own name, shown so the user can tell files apart.</summary>
    public string? PreviewSourceName { get; private set; }

    /// <summary>The chosen file's size on disk; the core's summary does not carry it.</summary>
    public long? PreviewArchiveBytes { get; private set; }

    public long PreviewStickers { get; private set; }

    public long PreviewDeletedStickers { get; private set; }

    public long PreviewCollections { get; private set; }

    public long PreviewTags { get; private set; }

    public long PreviewOriginalBytes { get; private set; }

    /// <summary>
    /// What a merge would add, keep and skip, as the core predicted it while inspecting
    /// against this library. A replace discards the current library, so these are hidden
    /// in that mode instead of being presented as a result.
    /// </summary>
    public long PreviewAdded { get; private set; }

    public long PreviewPreserved { get; private set; }

    public long PreviewSkippedRelations { get; private set; }

    /// <summary>Replace is destructive and is never the default choice.</summary>
    public bool ReplaceSelected { get; private set; }

    public string? PreviewErrorKey { get; private set; }

    public string? PreviewErrorPlaceholder { get; private set; }

    public string? RestoreErrorKey { get; private set; }

    public string? RestoreErrorPlaceholder { get; private set; }

    /// <summary>
    /// Secondary line next to a failed replace: whether the app reopened the library or
    /// has to be restarted, which is the part a user needs before trusting the contents.
    /// </summary>
    public string? RestoreStateKey { get; private set; }

    /// <summary>Success line, set only after the core reported a completed restore.</summary>
    public string? RestoreDoneKey { get; private set; }

    public long RestoredAdded { get; private set; }

    public long RestoredPreserved { get; private set; }

    public long RestoredSkippedRelations { get; private set; }

    public Task<bool> CreateBackupAsync()
    {
        if (!IsReady || IsBusy)
        {
            return Task.FromResult(false);
        }

        IsCreatingBackup = true;
        StageKey = null;
        BackupErrorKey = null;
        BackupErrorPlaceholder = null;
        BackupSavedPath = null;
        BackupSavedBytes = null;
        Raise(nameof(IsCreatingBackup), nameof(IsBusy), nameof(StageKey), nameof(BackupErrorKey),
            nameof(BackupErrorPlaceholder), nameof(BackupSavedPath), nameof(BackupSavedBytes));
        _backupRun = CreateBackupCoreAsync();
        return _backupRun;
    }

    public Task<bool> CreateCheckpointAsync()
    {
        if (!IsReady || IsBusy)
        {
            return Task.FromResult(false);
        }

        IsCreatingCheckpoint = true;
        CheckpointErrorKey = null;
        CheckpointName = null;
        Raise(nameof(IsCreatingCheckpoint), nameof(IsBusy), nameof(CheckpointErrorKey), nameof(CheckpointName));
        _checkpointRun = CreateCheckpointCoreAsync();
        return _checkpointRun;
    }

    /// <summary>
    /// Asks for a backup file and lets the core validate it. Dismissing the dialog is a
    /// cancel: no error, and no staged copy is kept.
    /// </summary>
    public Task<bool> ChooseArchiveAsync()
    {
        if (!IsReady || IsBusy || _archives is null)
        {
            return Task.FromResult(false);
        }

        IsInspecting = true;
        PreviewErrorKey = null;
        PreviewErrorPlaceholder = null;
        // A new choice starts a new attempt; the previous outcome must not stand next to
        // the new file's preview as if it described this one.
        RestoreErrorKey = null;
        RestoreErrorPlaceholder = null;
        RestoreStateKey = null;
        RestoreDoneKey = null;
        Raise(nameof(IsInspecting), nameof(IsBusy), nameof(PreviewErrorKey), nameof(PreviewErrorPlaceholder),
            nameof(RestoreErrorKey), nameof(RestoreErrorPlaceholder), nameof(RestoreStateKey),
            nameof(RestoreDoneKey));
        _inspectRun = ChooseArchiveCoreAsync();
        return _inspectRun;
    }

    /// <summary>Applies the previewed archive. Returns false when nothing was restored.</summary>
    public Task<bool> RestoreAsync()
    {
        if (!IsReady || IsBusy || _preview is null)
        {
            return Task.FromResult(false);
        }

        // Claim the section before asking the lifecycle to pause sync: that call awaits,
        // and a second click must not slip into the gap and start a second restore.
        IsRestoring = true;
        RestoreErrorKey = null;
        RestoreErrorPlaceholder = null;
        RestoreStateKey = null;
        RestoreDoneKey = null;
        Raise(nameof(IsRestoring), nameof(IsBusy), nameof(RestoreErrorKey), nameof(RestoreErrorPlaceholder),
            nameof(RestoreStateKey), nameof(RestoreDoneKey));

        _restoreRun = RestoreAttemptAsync();
        return _restoreRun;
    }

    /// <summary>
    /// Registers the attempt before the lifecycle is asked to accept it. DisposeAsync waits
    /// on this task, so an attempt that is still asking for permission to start is not
    /// missed while the window closes.
    /// </summary>
    private async Task<bool> RestoreAttemptAsync()
    {
        bool begun;
        try
        {
            begun = await _lifecycle.TryBeginRestoreAsync();
        }
        catch (Exception error)
        {
            IsRestoring = false;
            ReportRestoreFailure(error);
            Raise(nameof(IsRestoring), nameof(IsBusy), nameof(RestoreErrorKey));
            return false;
        }

        if (!begun)
        {
            // Something else is using the library. Nothing was touched, so this is a
            // refusal rather than a half-finished restore.
            IsRestoring = false;
            RestoreErrorKey = "desktop_restore_busy";
            RestoreErrorPlaceholder = null;
            Raise(nameof(IsRestoring), nameof(IsBusy), nameof(RestoreErrorKey),
                nameof(RestoreErrorPlaceholder));
            return false;
        }

        return await RestoreCoreAsync();
    }

    /// <summary>
    /// A merge commits before the page reloads. When that reload fails the data is already
    /// in the library, so the only remaining step is to reload again; this never applies
    /// the archive a second time.
    /// </summary>
    public bool CanRetryRefresh => !IsBusy && RestoreStateKey == "desktop_restore_state_applied";

    /// <summary>Reloads the page after a committed merge without repeating the merge.</summary>
    public async Task<bool> RetryRefreshAsync()
    {
        if (!CanRetryRefresh) return false;
        IsRestoring = true;
        Raise(nameof(IsRestoring), nameof(IsBusy), nameof(CanRetryRefresh));
        try
        {
            await _lifecycle.RefreshAsync();
            RestoreErrorKey = null;
            RestoreErrorPlaceholder = null;
            RestoreStateKey = null;
            RestoreDoneKey = "desktop_restore_merge_done";
            Raise(nameof(RestoreErrorKey), nameof(RestoreErrorPlaceholder), nameof(RestoreStateKey),
                nameof(RestoreDoneKey), nameof(CanRetryRefresh));
            return true;
        }
        catch (Exception error)
        {
            // Still committed, still only the reload failing: keep saying exactly that.
            Trace.WriteLine($"Reloading the library after a merge failed again: {error.GetType().Name}");
            RestoreErrorKey = _strings.ErrorKey(error);
            RestoreStateKey = "desktop_restore_state_applied";
            Raise(nameof(RestoreErrorKey), nameof(RestoreStateKey), nameof(CanRetryRefresh));
            return false;
        }
        finally
        {
            IsRestoring = false;
            Raise(nameof(IsRestoring), nameof(IsBusy), nameof(CanRetryRefresh));
        }
    }

    /// <summary>Selects merge or replace before a restore is confirmed.</summary>
    public void SelectMode(bool replace)
    {
        if (ReplaceSelected == replace) return;
        ReplaceSelected = replace;
        Raise(nameof(ReplaceSelected));
    }

    /// <summary>
    /// Leaves the preview without touching the library, releasing the core's staged copy.
    /// </summary>
    public async Task DiscardPreviewAsync()
    {
        var preview = _preview;
        _preview = null;
        ClearPreview();
        if (preview is null) return;
        try
        {
            await preview.DisposeAsync().ConfigureAwait(false);
        }
        catch (Exception error)
        {
            // The staged copy lives in the library's data directory and the core sweeps
            // abandoned staging entries when it next opens the library.
            Trace.WriteLine($"Discarding the inspected archive failed: {error.GetType().Name}");
        }
    }

    /// <summary>
    /// Stops starting new work and asks the core to stop what is running. Anything that
    /// is already running still finishes before <see cref="DisposeAsync"/> returns, so
    /// the session outlives the files it is working on.
    /// </summary>
    public void RequestStop()
    {
        if (_stopping) return;
        _stopping = true;
        _lifetime.Cancel();
        Raise(nameof(IsReady));
    }

    public async ValueTask DisposeAsync()
    {
        if (_disposed) return;
        _disposed = true;
        RequestStop();
        await _inspectRun.ConfigureAwait(false);
        await _restoreRun.ConfigureAwait(false);
        await _backupRun.ConfigureAwait(false);
        await _checkpointRun.ConfigureAwait(false);
        await DiscardPreviewAsync().ConfigureAwait(false);
        _lifetime.Dispose();
    }

    private async Task<bool> CreateBackupCoreAsync()
    {
        IStagedBackup? staged = null;
        try
        {
            // Progress is created on the calling thread, so reports reach the page
            // through its synchronization context instead of a worker.
            _reportingStages = true;
            try
            {
                staged = await _workspace!.StageBackupAsync(new Progress<string>(ReportStage), _lifetime.Token);
            }
            finally
            {
                // Reports posted while staging ran are queued ahead of this continuation,
                // so closing the window here cannot drop one that still applies.
                _reportingStages = false;
            }

            SetStage(null);

            var destination = await _picker.PickSavePathAsync(staged.FileName, _lifetime.Token);
            if (destination is not { } target)
            {
                // Dismissing the picker is a cancel: no file, and nothing to report.
                return false;
            }

            _lifetime.Token.ThrowIfCancellationRequested();
            SetStage("backup_saving");
            await BackupFileWriter.WriteAsync(
                staged.Path,
                target.Path,
                staged.ByteSize,
                target.ReplaceExisting,
                _lifetime.Token);
            BackupSavedPath = target.Path;
            BackupSavedBytes = staged.ByteSize;
            return true;
        }
        catch (OperationCanceledException)
        {
            return false;
        }
        catch (BackupTargetExistsException)
        {
            BackupErrorKey = "desktop_backup_exists";
            return false;
        }
        catch (Exception error)
        {
            ReportBackupFailure(error);
            return false;
        }
        finally
        {
            // The run is closed before the staging entry is released, so a stage report
            // that is still queued cannot revive the label after the page was told the
            // run had ended.
            IsCreatingBackup = false;
            StageKey = null;
            if (staged is not null)
            {
                // No ConfigureAwait(false) here: the notifications below must come back
                // on the page's thread.
                await DiscardStagedAsync(staged);
            }

            Raise(nameof(IsCreatingBackup), nameof(IsBusy), nameof(StageKey),
                nameof(BackupErrorKey), nameof(BackupErrorPlaceholder),
                nameof(BackupSavedPath), nameof(BackupSavedBytes));
        }
    }

    private async Task<bool> CreateCheckpointCoreAsync()
    {
        try
        {
            var path = await _workspace!.CreateCheckpointAsync(_lifetime.Token);
            CheckpointName = System.IO.Path.GetFileName(path);
            if (string.IsNullOrEmpty(CheckpointName))
            {
                CheckpointName = path;
            }

            return true;
        }
        catch (OperationCanceledException)
        {
            return false;
        }
        catch (Exception error)
        {
            CheckpointErrorKey = _strings.ErrorKey(error);
            return false;
        }
        finally
        {
            IsCreatingCheckpoint = false;
            Raise(nameof(IsCreatingCheckpoint), nameof(IsBusy), nameof(CheckpointErrorKey), nameof(CheckpointName));
        }
    }

    private async Task<bool> ChooseArchiveCoreAsync()
    {
        try
        {
            // A new choice replaces the previous preview; its staged copy is released
            // before another one is staged, so at most one copy is ever held.
            await DiscardPreviewAsync();

            var path = await _sources.PickArchivePathAsync(_lifetime.Token);
            if (path is null)
            {
                // Dismissing the dialog is a cancel: nothing was read, nothing to report.
                return false;
            }

            _lifetime.Token.ThrowIfCancellationRequested();
            var bytes = ReadFileSize(path);
            var preview = await _archives!.InspectArchiveAsync(path, _lifetime.Token);
            _preview = preview;
            ApplyPreview(preview.Summary, path, bytes);
            return true;
        }
        catch (OperationCanceledException)
        {
            return false;
        }
        catch (Exception error)
        {
            ReportPreviewFailure(error);
            return false;
        }
        finally
        {
            IsInspecting = false;
            Raise(nameof(IsInspecting), nameof(IsBusy), nameof(HasPreview), nameof(PreviewSourceName),
                nameof(PreviewArchiveBytes), nameof(PreviewErrorKey), nameof(PreviewErrorPlaceholder));
        }
    }

    private async Task<bool> RestoreCoreAsync()
    {
        // The restore owns the preview from here on; the core discards it whether the
        // restore succeeds, fails or is cancelled.
        var preview = _preview!;
        _preview = null;
        try
        {
            return await ApplyAsync(preview, ReplaceSelected, _lifetime.Token);
        }
        finally
        {
            ClearPreview();
            try
            {
                // Resuming is the last thing the section waits for, so the page never
                // shows a finished restore while the library is still held paused.
                await _lifecycle.EndRestoreAsync();
            }
            catch (Exception error)
            {
                Trace.WriteLine($"Ending the restore lifecycle failed: {error.GetType().Name}");
            }
            finally
            {
                IsRestoring = false;
                Raise(nameof(IsRestoring), nameof(IsBusy), nameof(HasPreview), nameof(PreviewSourceName),
                    nameof(PreviewArchiveBytes), nameof(PreviewStickers), nameof(PreviewDeletedStickers),
                    nameof(PreviewCollections), nameof(PreviewTags), nameof(PreviewOriginalBytes),
                    nameof(PreviewAdded), nameof(PreviewPreserved), nameof(PreviewSkippedRelations),
                    nameof(RestoreErrorKey), nameof(RestoreErrorPlaceholder), nameof(RestoreStateKey),
                    nameof(RestoreDoneKey), nameof(RestoredAdded), nameof(RestoredPreserved),
                    nameof(RestoredSkippedRelations));
            }
        }
    }

    private async Task<bool> ApplyAsync(IArchivePreview preview, bool replacing, CancellationToken cancellation)
    {
        var mode = replacing ? ArchiveRestoreMode.Replace : ArchiveRestoreMode.Merge;
        ArchiveSummary summary;
        try
        {
            summary = await preview.RestoreArchiveAsync(mode, cancellation);
        }
        catch (Exception error)
        {
            // Replacing closes the library before it swaps the database, so a failed or
            // cancelled replace can still leave this session dead. A merge rolls its
            // transaction back and leaves the session usable.
            if (replacing)
            {
                await ReopenAfterFailureAsync();
            }

            if (error is not OperationCanceledException)
            {
                ReportRestoreFailure(error);
            }

            return false;
        }

        if (replacing)
        {
            // The replace is committed, so what is left is which session this page can use.
            // ReopenAsync returns false when the open was refused because that session still
            // owns the library, and throws when the library could not be opened at all;
            // neither answer says whether the contents were swapped, and the exception type
            // says nothing about it either. IsReady is the only signal the rebuild itself
            // wrote, so the reported state is mapped from it alone.
            try
            {
                if (await _lifecycle.ReopenAsync(cancellation))
                {
                    RestoreDoneKey = "desktop_restore_replace_done";
                    return true;
                }
            }
            catch (Exception error)
            {
                // The restore finished; only the rebind failed. It must never escape: the
                // click handler is async void, so the state is reported instead.
                Trace.WriteLine($"Reopening the library after a replace failed: {error.GetType().Name}");
            }

            if (!_lifecycle.IsReady)
            {
                // The core closed the library and the new session was never bound, so every
                // later command would fail on a dead handle. Only this state asks to restart.
                RestoreErrorKey = "desktop_restore_reopen_failed";
                RestoreStateKey = "desktop_restore_state_stopped";
                return false;
            }

            // The library is still open and usable, so the page keeps working. What cannot
            // be checked from here is whether this session shows the replaced contents,
            // which is exactly what the state line says.
            RestoreDoneKey = "desktop_restore_replace_done";
            RestoreStateKey = "desktop_restore_state_unconfirmed";
            return true;
        }

        RestoredAdded = (long)Math.Min(summary.Added, long.MaxValue);
        RestoredPreserved = (long)Math.Min(summary.Preserved, long.MaxValue);
        RestoredSkippedRelations = (long)Math.Min(summary.SkippedRelations, long.MaxValue);
        try
        {
            await _lifecycle.RefreshAsync();
        }
        catch (Exception error)
        {
            // The merge is committed, so calling it a failed restore would be wrong. The
            // page cannot show the new numbers either, which the state line says.
            Trace.WriteLine($"Reloading the library after a merge failed: {error.GetType().Name}");
            RestoreErrorKey = _strings.ErrorKey(error);
            RestoreStateKey = "desktop_restore_state_applied";
            return false;
        }

        RestoreDoneKey = "desktop_restore_merge_done";
        return true;
    }

    private void ApplyPreview(ArchiveSummary summary, string path, long? bytes)
    {
        PreviewSourceName = System.IO.Path.GetFileName(path);
        if (string.IsNullOrEmpty(PreviewSourceName))
        {
            PreviewSourceName = path;
        }

        PreviewArchiveBytes = bytes;
        PreviewStickers = (long)Math.Min(summary.Stickers, long.MaxValue);
        PreviewDeletedStickers = (long)Math.Min(summary.DeletedStickers, long.MaxValue);
        PreviewCollections = (long)Math.Min(summary.Collections, long.MaxValue);
        PreviewTags = (long)Math.Min(summary.Tags, long.MaxValue);
        PreviewOriginalBytes = (long)Math.Min(summary.OriginalBytes, long.MaxValue);
        PreviewAdded = (long)Math.Min(summary.Added, long.MaxValue);
        PreviewPreserved = (long)Math.Min(summary.Preserved, long.MaxValue);
        PreviewSkippedRelations = (long)Math.Min(summary.SkippedRelations, long.MaxValue);
    }

    private void ClearPreview()
    {
        PreviewSourceName = null;
        PreviewArchiveBytes = null;
        PreviewStickers = 0;
        PreviewDeletedStickers = 0;
        PreviewCollections = 0;
        PreviewTags = 0;
        PreviewOriginalBytes = 0;
        PreviewAdded = 0;
        PreviewPreserved = 0;
        PreviewSkippedRelations = 0;
    }

    private static long? ReadFileSize(string path)
    {
        try
        {
            return new System.IO.FileInfo(path).Length;
        }
        catch (Exception error) when (error is System.IO.IOException or UnauthorizedAccessException
            or ArgumentException or NotSupportedException)
        {
            // The size is only display data. An unreadable file fails when the core
            // inspects it, which is the error the page reports.
            return null;
        }
    }

    private void ReportStage(string stage)
    {
        if (!IsCreatingBackup || !_reportingStages)
        {
            // A report that arrived after staging finished must not revive a stage label
            // next to a save dialog or a finished run.
            return;
        }

        SetStage(StageKeys.TryGetValue(stage, out var key) ? key : null);
    }

    private void SetStage(string? key)
    {
        if (StageKey == key) return;
        StageKey = key;
        Raise(nameof(StageKey));
    }

    private void ReportBackupFailure(Exception error)
    {
        // The core enforces the archive ceiling and reports a refusal from that check
        // as a corrupt-archive failure. That is a size limit, not damaged data, so it
        // is named with the limit the core was actually given.
        if (error is BridgeException.Failure { code: ErrorCode.CorruptData } failure
            && failure.detail.Contains("archive exceeds byte limit", StringComparison.Ordinal))
        {
            BackupErrorKey = "desktop_backup_too_large";
            BackupErrorPlaceholder = ByteSize.Format(_workspace!.MaxArchiveBytes);
            return;
        }

        BackupErrorKey = _strings.ErrorKey(error);
    }

    private void ReportPreviewFailure(Exception error)
    {
        // A check refusal is not "the file is broken": the ceiling, an unreadable file
        // and a corrupt archive each get their own explanation.
        if (error is BridgeException.Failure { code: ErrorCode.CorruptData } failure
            && failure.detail.Contains("archive exceeds byte limit", StringComparison.Ordinal))
        {
            PreviewErrorKey = "desktop_restore_too_large";
            PreviewErrorPlaceholder = ByteSize.Format(_archives!.MaxArchiveBytes);
            return;
        }

        PreviewErrorKey = MapArchiveFailure(error);
    }

    private void ReportRestoreFailure(Exception error)
    {
        if (error is BridgeException.Failure { code: ErrorCode.CorruptData } failure
            && failure.detail.Contains("archive exceeds byte limit", StringComparison.Ordinal))
        {
            RestoreErrorKey = "desktop_restore_too_large";
            RestoreErrorPlaceholder = ByteSize.Format(_archives!.MaxArchiveBytes);
            return;
        }

        RestoreErrorKey = MapArchiveFailure(error);
    }

    private string MapArchiveFailure(Exception error) => error switch
    {
        BridgeException.Failure { code: ErrorCode.CorruptData } => "desktop_restore_corrupt",
        BridgeException.Failure { code: ErrorCode.UnsupportedFormat or ErrorCode.UnsupportedSchema }
            => "desktop_restore_unsupported",
        BridgeException.Failure { code: ErrorCode.Conflict } => "desktop_restore_changed",
        BridgeException.Failure { code: ErrorCode.Busy or ErrorCode.AlreadyOpen } => "desktop_restore_busy",
        BridgeException.Failure { code: ErrorCode.ResourceLimit or ErrorCode.StorageFull }
            => "desktop_restore_too_large",
        System.IO.FileNotFoundException or System.IO.DirectoryNotFoundException => "desktop_restore_read_failed",
        UnauthorizedAccessException => "desktop_restore_read_failed",
        System.IO.IOException => "desktop_restore_read_failed",
        _ => _strings.ErrorKey(error),
    };

    private async Task ReopenAfterFailureAsync()
    {
        // A failed replace may still have closed the library. Reopen before reporting so
        // the app can keep working; the outcome is named in the state line, not guessed.
        try
        {
            if (await _lifecycle.ReopenAsync(CancellationToken.None))
            {
                RestoreStateKey = "desktop_restore_state_reopened";
            }
            else if (!_lifecycle.IsReady)
            {
                // The core had closed the library and the rebound page could not be built.
                // The data is intact, but this window cannot keep using it.
                RestoreStateKey = "desktop_restore_state_stopped";
            }
            else
            {
                // The open was refused because the previous handle still owns the
                // library, which is exactly the state to report: nothing was lost.
                RestoreStateKey = "desktop_restore_state_unchanged";
            }
        }
        catch (Exception error)
        {
            // The thrown type says nothing about the library; IsReady does. A session that
            // was never replaced is reported as untouched rather than as a closed library.
            Trace.WriteLine($"Reopening the library after a failed restore failed: {error.GetType().Name}");
            RestoreStateKey = _lifecycle.IsReady
                ? "desktop_restore_state_unchanged"
                : "desktop_restore_state_stopped";
        }
    }

    private static async Task DiscardStagedAsync(IStagedBackup staged)
    {
        try
        {
            await staged.DisposeAsync().ConfigureAwait(false);
        }
        catch (Exception error)
        {
            // The staged archive lives inside the library's data directory and the core
            // sweeps abandoned staging entries when it next opens the library, so a
            // failed discard must not turn an already saved backup into a failure.
            Trace.WriteLine($"Discarding the staged backup failed: {error.GetType().Name}");
        }
    }

    private void Raise(params string[] names)
    {
        foreach (var name in names)
        {
            OnPropertyChanged(name);
        }
    }
}
