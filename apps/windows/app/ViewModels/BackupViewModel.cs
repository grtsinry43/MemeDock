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
/// Creates backups and checkpoints. A backup is staged by the core first and only
/// becomes the user's file once they pick a location and the copy completes, so a
/// cancelled or failed run never leaves a partial file under the backup's name.
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

    private readonly IBackupWorkspace? _workspace;
    private readonly IBackupDestinationPicker _picker;
    private readonly ICatalogStrings _strings;
    private readonly CancellationTokenSource _lifetime = new();
    private Task<bool> _backupRun = Task.FromResult(false);
    private Task<bool> _checkpointRun = Task.FromResult(false);
    private bool _stopping;
    private bool _disposed;
    private bool _reportingStages;

    public BackupViewModel(IBackupWorkspace? workspace, IBackupDestinationPicker picker, ICatalogStrings strings)
    {
        (_workspace, _picker, _strings) = (workspace, picker, strings);
    }

    public bool IsReady => _workspace is not null && !_stopping;

    public bool IsCreatingBackup { get; private set; }

    public bool IsCreatingCheckpoint { get; private set; }

    public bool IsBusy => IsCreatingBackup || IsCreatingCheckpoint;

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
        await _backupRun.ConfigureAwait(false);
        await _checkpointRun.ConfigureAwait(false);
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
