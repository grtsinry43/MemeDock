using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Services;

namespace MemeDock.Windows.ViewModels;

/// <summary>
/// Writes one collection, or every collection, into a single archive of originals.
/// The core owns the writing and the report: an export that skipped damaged or
/// missing originals still produced a usable file, so the counts and reasons are
/// shown instead of being treated as a failure.
/// </summary>
public sealed class CollectionExportViewModel : ObservableObject, IAsyncDisposable
{
    private readonly LibrarySession _session;
    private readonly ICatalogStrings _strings;
    private readonly IBackupDestinationPicker _picker;
    private readonly CancellationTokenSource _lifetime = new();
    private CancellationTokenSource? _runCancellation;
    private Task<bool> _pending = Task.FromResult(false);
    private CollectionExportSummary? _last;
    private bool _empty;
    private bool _stopping;

    public bool IsRunning { get; private set; }
    public string RunningLabel { get; private set; } = "";
    public string RunningText => RunningLabel.Length == 0 ? ""
        : _strings.Format("desktop_export_collection_running", RunningLabel);
    public string? ErrorKey { get; private set; }
    public string ErrorText => ErrorKey is null ? "" : _strings[ErrorKey];

    /// <summary>Nothing was written because the collection had no stickers.</summary>
    public bool IsEmptyResult => _empty;

    /// <summary>A partial export that wrote nothing reads as a failure, like macOS.</summary>
    public bool IsFailureMessage => _last is { Exported: 0, Skipped: > 0 };

    public string MessageText
    {
        get
        {
            if (ErrorKey is not null) return "";
            if (_empty) return _strings["collection_empty"];
            if (_last is not { } summary) return "";
            if (summary.Skipped == 0)
                return _strings.Format("desktop_export_collection_done", summary.Exported);
            var reasons = new List<string>();
            if (summary.MissingOriginal > 0)
                reasons.Add(_strings.Format("desktop_export_issue_missing", summary.MissingOriginal));
            if (summary.Corrupt > 0)
                reasons.Add(_strings.Format("desktop_export_issue_corrupt", summary.Corrupt));
            if (summary.TooLarge > 0)
                reasons.Add(_strings.Format("desktop_export_issue_too_large", summary.TooLarge));
            if (summary.Unreadable > 0)
                reasons.Add(_strings.Format("desktop_export_issue_unreadable", summary.Unreadable));
            return _strings.Format("desktop_export_collection_partial", summary.Exported, summary.Skipped,
                string.Join(_strings["desktop_list_separator"], reasons));
        }
    }

    internal CollectionExportViewModel(LibrarySession session, ICatalogStrings strings, IBackupDestinationPicker picker)
        => (_session, _strings, _picker) = (session, strings, picker);

    public Task<bool> ExportCollectionAsync(CollectionMetadata collection)
        => BeginAsync(collection.Id, collection.Name + ".zip", collection.Name);

    public Task<bool> ExportAllAsync()
        => BeginAsync(null, _strings["desktop_export_all_file_name"], _strings["desktop_all_collections"]);

    private Task<bool> BeginAsync(string? collectionId, string fileName, string label)
    {
        if (_stopping || IsRunning) return Task.FromResult(false);
        var work = RunCoreAsync(collectionId, fileName, label);
        _pending = work;
        return work;
    }

    private async Task<bool> RunCoreAsync(string? collectionId, string fileName, string label)
    {
        IsRunning = true;
        RunningLabel = label;
        ErrorKey = null;
        _last = null;
        _empty = false;
        Notify();
        var cancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _runCancellation = cancellation;
        try
        {
            var destination = await _picker.PickSavePathAsync(fileName, cancellation.Token,
                "desktop_collection_export_file_type");
            if (destination is null || _stopping) return false;
            // Zip entries carry no zone; the core writes the local wall-clock time
            // the person exporting recognises.
            var offset = (int)TimeZoneInfo.Local.GetUtcOffset(DateTimeOffset.Now).TotalMinutes;
            var summary = await _session.ExportCollectionAsync(collectionId, destination.Value.Path, offset,
                cancellation.Token);
            if (_stopping) return false;
            _last = summary;
            return true;
        }
        catch (OperationCanceledException) { return false; }
        catch (BridgeException error) when (error is BridgeException.Failure { code: ErrorCode.InvalidInput })
        {
            // Nothing to write is a normal outcome of asking, not a failure worth
            // the generic error wording; macOS reports the same empty state.
            if (!_stopping) _empty = true;
            return false;
        }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            if (!_stopping) ErrorKey = _strings.ErrorKey(error);
            return false;
        }
        finally
        {
            IsRunning = false;
            RunningLabel = "";
            _runCancellation = null;
            cancellation.Dispose();
            Notify();
        }
    }

    public void Cancel() => _runCancellation?.Cancel();

    public void RequestStop()
    {
        if (_stopping) return;
        _stopping = true;
        _lifetime.Cancel();
        _runCancellation?.Cancel();
    }

    public void Relocalize() => Notify();
    private void Notify() => OnPropertyChanged(string.Empty);

    public async ValueTask DisposeAsync()
    {
        RequestStop();
        try { await _pending; }
        catch (OperationCanceledException) { }
        _lifetime.Dispose();
    }
}