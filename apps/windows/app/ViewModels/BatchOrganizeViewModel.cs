using System.Collections.ObjectModel;
using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;

namespace MemeDock.Windows.ViewModels;

public sealed record BatchRow(string Id, string Title, BatchOutcome Outcome, string Error);

public sealed class BatchOrganizeViewModel : ObservableObject, IAsyncDisposable
{
    // Matches memedock_core::batch::MAX_BATCH_ITEMS; requests are still validated by core.
    public const int Limit = 1000;
    private readonly LibrarySession _session;
    private readonly LocalizationService _strings;
    private readonly CancellationTokenSource _lifetime = new();
    private BatchTarget[] _targets = [];
    private Dictionary<string, string> _titles = [];
    private BatchTask? _native;
    private Task _running = Task.CompletedTask;
    private bool _stopping;
    public ObservableCollection<BatchRow> Results { get; } = [];
    public int Count => _targets.Length;
    public bool AllStarred { get; private set; }
    public bool IsRunning { get; private set; }
    public bool IsStopping { get; private set; }
    public bool CanRun => Count is > 0 and <= Limit && !IsRunning && !_stopping;
    public string? ErrorKey { get; private set; }
    public string ErrorText => Count > Limit ? _strings.Format("desktop_selection_over_limit", Count, Limit)
        : ErrorKey is null ? "" : _strings[ErrorKey];
    public string CountText => _strings.Format("selection_count", Count);
    public string ProgressText { get; private set; } = "";
    public BatchReport? Report { get; private set; }
    public event EventHandler? Completed;

    internal BatchOrganizeViewModel(LibrarySession session, LocalizationService strings) => (_session, _strings) = (session, strings);

    public void SetSelection(IEnumerable<StickerCard> cards)
    {
        if (IsRunning) return;
        var items = cards.DistinctBy(card => card.Id).ToArray();
        var targets = items.Select(card => new BatchTarget(new EntityReference(card.Id, card.Generation), null)).ToArray();
        AllStarred = items.Length > 0 && items.All(card => card.Starred);
        if (_targets.Select(target => target.Target).SequenceEqual(targets.Select(target => target.Target))) { Notify(); return; }
        _targets = targets;
        _titles = items.ToDictionary(card => card.Id, card => card.Title);
        Report = null;
        Results.Clear();
        ErrorKey = null;
        ProgressText = "";
        Notify();
    }

    public void Relocalize()
    {
        if (Report is not null) ShowReport(Report);
        Notify();
    }

    internal void RetainRemaining(IEnumerable<StickerCard> cards)
    {
        if (IsRunning) return;
        var items = cards.ToArray();
        AllStarred = items.Length > 0 && items.All(card => card.Starred);
        _targets = items.Select(card => new BatchTarget(new EntityReference(card.Id, card.Generation), null)).ToArray();
        Notify();
    }

    public Task RunAsync(BatchAction action)
    {
        if (!CanRun) return Task.CompletedTask;
        return _running = RunCoreAsync(action);
    }

    private async Task RunCoreAsync(BatchAction action)
    {
        // Capture references before any await; later browsing cannot retarget this batch.
        var targets = _targets.ToArray();
        IsRunning = true;
        IsStopping = false;
        ErrorKey = null;
        Results.Clear();
        Report = null;
        ProgressText = _strings.Format("batch_progress", 0, targets.Length);
        Notify();
        try
        {
            _native = await _session.StartBatchAsync(targets, action, _lifetime.Token);
            if (IsStopping) _native.Cancel();
            using var registration = _lifetime.Token.Register(() => _native?.Cancel());
            var result = _native.AwaitResult();
            while (!result.IsCompleted)
            {
                await Task.WhenAny(result, Task.Delay(150));
                if (_stopping) break;
                var snapshot = await Task.Run(() => _native.Snapshot());
                ProgressText = _strings.Format("batch_progress", snapshot.Items.Count(item => item.Outcome != BatchOutcome.Pending), targets.Length);
                Notify();
            }
            Report = await result;
            ShowReport(Report);
        }
        catch (OperationCanceledException) { }
        catch (Exception error) { Trace.WriteLine(error); if (!_stopping) ErrorKey = _strings.ErrorKey(error); }
        finally
        {
            _native?.Dispose();
            _native = null;
            IsRunning = false;
            IsStopping = false;
            Notify();
            if (!_stopping && Report is not null) Completed?.Invoke(this, EventArgs.Empty);
        }
    }

    private void ShowReport(BatchReport report)
    {
        Results.Clear();
        foreach (var item in report.Items.Where(item => item.Outcome is BatchOutcome.Failed or BatchOutcome.Pending))
        {
            var error = item.Error is { } code
                ? _strings[_strings.ErrorKey(new BridgeException.Failure(code, "Batch item failed."))]
                : _strings["import_cancelled"];
            Results.Add(new BatchRow(item.Id, _titles.GetValueOrDefault(item.Id, item.Id), item.Outcome, error));
        }
        ProgressText = _strings.Format("batch_result",
            report.Items.Count(item => item.Outcome is BatchOutcome.Applied or BatchOutcome.Unchanged),
            report.Items.Count(item => item.Outcome == BatchOutcome.Failed),
            report.Items.Count(item => item.Outcome == BatchOutcome.Pending));
    }

    public void Cancel()
    {
        if (!IsRunning || IsStopping) return;
        IsStopping = true;
        _native?.Cancel();
        Notify();
    }

    private void Notify() => OnPropertyChanged(string.Empty);
    public async ValueTask DisposeAsync()
    {
        _stopping = true;
        _lifetime.Cancel();
        await _running;
        _lifetime.Dispose();
    }
}
